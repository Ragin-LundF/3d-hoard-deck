use std::sync::Arc;
use std::sync::mpsc::sync_channel;
use std::thread;

use hoarddeck_domain::{Entry, Library, LibraryId, RelPath};

use super::{CompiledRules, KnownDirs, LibraryLocks, ScanItem, ScanPhase, ScanProgress, regroup};
use crate::ports::{EntryStore, EventSink, LibraryRepository, LibraryScanner, ModelRepository, SearchIndex};
use crate::tagging::TagRuleBook;
use crate::{AppError, ScanError};

/// Entries per index transaction while walking.
const BATCH_ENTRIES: usize = 2_000;
/// Walk results buffered between the walker threads and the index writer; keeps memory flat.
const CHANNEL_CAPACITY: usize = 256;
/// A library root that suddenly lists nothing while the index knew more folders than this is treated as offline.
const MIN_DIRS_FOR_EMPTY_GUARD: usize = 20;
/// Wait before asking again whether a folder that seemed gone really is.
const RECHECK_DELAY: std::time::Duration = std::time::Duration::from_millis(500);
/// Unreadable entries named in the report; the rest are only counted.
const MAX_FAILURES_LISTED: usize = 50;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ScanMode {
    /// Lists only folders whose mtime changed since the last scan.
    #[default]
    Incremental,
    /// Lists every folder; catches files overwritten in place, which do not change the folder mtime.
    Full,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ScanReport {
    pub dirs_listed: u64,
    pub dirs_unchanged: u64,
    pub dirs_failed: u64,
    /// `path: reason` of the first unreadable folders and files.
    pub failures: Vec<String>,
    /// Only one folder and what is below it was scanned.
    pub partial: bool,
    pub entries_seen: u64,
    pub entries_removed: u64,
    pub models: usize,
    pub folders: usize,
}

/// Walks a library, stores what changed, regroups models and refreshes the search index.
/// Reads the library only; all writes go to the index.
/// At most one scan runs per library.
pub struct ScanLibrary {
    libraries: Arc<dyn LibraryRepository>,
    entries: Arc<dyn EntryStore>,
    models: Arc<dyn ModelRepository>,
    search: Arc<dyn SearchIndex>,
    scanner: Arc<dyn LibraryScanner>,
    events: Arc<dyn EventSink>,
    locks: Arc<LibraryLocks>,
    tag_rules: Arc<TagRuleBook>,
}

impl ScanLibrary {
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        libraries: Arc<dyn LibraryRepository>,
        entries: Arc<dyn EntryStore>,
        models: Arc<dyn ModelRepository>,
        search: Arc<dyn SearchIndex>,
        scanner: Arc<dyn LibraryScanner>,
        events: Arc<dyn EventSink>,
        locks: Arc<LibraryLocks>,
        tag_rules: Arc<TagRuleBook>,
    ) -> Self {
        Self { libraries, entries, models, search, scanner, events, locks, tag_rules }
    }

    /// # Errors
    /// Fails when the library is unknown, offline or busy, or the index cannot be written.
    /// An offline library keeps its index.
    pub fn execute(&self, id: LibraryId, mode: ScanMode) -> Result<ScanReport, AppError> {
        self.execute_with(id, mode, &[])
    }

    /// Like [`Self::execute`], and also lists `changed` folders, where a file may have changed in place without
    /// touching the folder's mtime.
    /// # Errors
    /// As [`Self::execute`].
    pub fn execute_with(&self, id: LibraryId, mode: ScanMode, changed: &[RelPath]) -> Result<ScanReport, AppError> {
        self.run(id, &RelPath::root(), mode, changed)
    }

    /// Scans only `folder` and what is below it, much faster than the whole library on a big share. Entries elsewhere
    /// stay as they are.
    /// # Errors
    /// As [`Self::execute`].
    pub fn scan_folder(&self, id: LibraryId, folder: &RelPath, mode: ScanMode) -> Result<ScanReport, AppError> {
        self.run(id, folder, mode, &[])
    }

    fn run(&self, id: LibraryId, from: &RelPath, mode: ScanMode, changed: &[RelPath]) -> Result<ScanReport, AppError> {
        let _busy = self.locks.lock(id)?;
        let library = self.libraries.get(id)?.ok_or(AppError::LibraryNotFound(id))?;
        let rules = CompiledRules::compile(&self.libraries.scan_rules(id)?)?;
        let mut report = self.walk(&library, &rules, from, mode, changed)?;
        self.progress(id, ScanPhase::Grouping, &report);
        // Read again: the user may have marked a folder while the scan was walking.
        let rules = CompiledRules::compile(&self.libraries.scan_rules(id)?)?;
        (report.models, report.folders) = regroup(
            self.entries.as_ref(),
            self.models.as_ref(),
            self.search.as_ref(),
            &self.locks,
            id,
            &rules,
            &self.tag_rules.current(),
        )?;
        self.progress(id, ScanPhase::Done, &report);
        Ok(report)
    }

    /// Puts files the app itself just wrote into the index and regroups, without walking the library (a network share
    /// takes seconds for that). The folder is marked so the next scan lists it properly.
    /// # Errors
    /// Fails when the library is busy or the index cannot be written.
    pub fn add_files(&self, id: LibraryId, dir: &RelPath, files: Vec<Entry>) -> Result<(), AppError> {
        self.add_files_in(id, vec![(dir.clone(), files)])
    }

    /// Like [`Self::add_files`] for files in several folders, with one regroup for all of them.
    /// # Errors
    /// Fails when the rules or the index cannot be read or written.
    pub fn add_files_in(&self, id: LibraryId, folders: Vec<(RelPath, Vec<Entry>)>) -> Result<(), AppError> {
        let _busy = self.locks.lock(id)?;
        let rules = CompiledRules::compile(&self.libraries.scan_rules(id)?)?;
        // A fresh generation without `finish_scan` deletes nothing; the next full scan settles everything again.
        let (generation, _) = self.entries.begin_scan(id)?;
        let listed: Vec<ScanItem> =
            folders.into_iter().map(|(dir, children)| ScanItem::Listed { dir, mtime: i64::MIN, children }).collect();
        self.entries.apply(id, generation, &listed)?;
        // No progress events: a "done" would replace the last real scan's report (unreadable folders) in the UI.
        regroup(
            self.entries.as_ref(),
            self.models.as_ref(),
            self.search.as_ref(),
            &self.locks,
            id,
            &rules,
            &self.tag_rules.current(),
        )?;
        Ok(())
    }

    fn walk(
        &self,
        library: &Library,
        rules: &CompiledRules,
        from: &RelPath,
        mode: ScanMode,
        changed: &[RelPath],
    ) -> Result<ScanReport, AppError> {
        let (generation, mut known) = self.entries.begin_scan(library.id)?;
        let indexed_dirs = known.len();
        // Folders an ignore rule hides now (e.g. `#recycle`) are never listed again, so they are removed after the walk.
        let ignored: Vec<RelPath> =
            known.keys().filter(|dir| rules.is_ignored_dir(dir) && dir.is_within(from)).cloned().collect();
        if mode == ScanMode::Full {
            known = KnownDirs::new();
        }
        // A folder the scan does not know is always listed.
        for dir in changed {
            known.remove(dir);
        }
        let (sender, receiver) = sync_channel::<ScanItem>(CHANNEL_CAPACITY);
        let mut report = ScanReport { partial: !from.is_root(), ..ScanReport::default() };
        // The scanner reports nothing for a start folder that no longer exists.
        let mut visited = false;
        let mut root_empty = false;
        thread::scope(|scope| -> Result<(), AppError> {
            let (scanner, known, root) = (&self.scanner, &known, library.root.as_str());
            let walker = scope.spawn(move || scanner.scan(root, from, known, rules, &sender));
            let mut batch = Vec::new();
            let mut batch_entries = 0;
            for item in receiver {
                visited |= item_dir(&item) == from;
                if let ScanItem::Listed { dir, children, .. } = &item
                    && dir.is_root()
                {
                    root_empty = children.is_empty();
                }
                batch_entries += count(&item, &mut report);
                batch.push(item);
                if batch_entries >= BATCH_ENTRIES {
                    self.entries.apply(library.id, generation, &batch)?;
                    self.progress(library.id, ScanPhase::Walking, &report);
                    batch.clear();
                    batch_entries = 0;
                }
            }
            self.entries.apply(library.id, generation, &batch)?;
            walker.join().map_err(|_| ScanError::Aborted("walker thread panicked".to_owned()))??;
            Ok(())
        })?;
        if from.is_root() && root_empty && indexed_dirs > MIN_DIRS_FOR_EMPTY_GUARD {
            // A stale mount point or a share that answers with nothing must not clear a big library's index.
            return Err(ScanError::Offline(format!("{} looks empty; the index is kept", library.root)).into());
        }
        if !visited && !from.is_root() {
            // An SMB hiccup can report a folder as missing for a moment; only a second "not found" removes it.
            thread::sleep(RECHECK_DELAY);
            visited = self.scanner.exists(&library.root, from) != Ok(false);
        }
        report.entries_removed = self.entries.finish_folder_scan(library.id, generation, from, visited)?;
        if !ignored.is_empty() {
            report.entries_removed += self.entries.remove_folders(library.id, &ignored)?;
        }
        Ok(report)
    }

    fn progress(&self, library: LibraryId, phase: ScanPhase, report: &ScanReport) {
        let dirs = report.dirs_listed + report.dirs_unchanged;
        self.events.scan_progress(&ScanProgress {
            library,
            phase,
            dirs,
            files: report.entries_seen,
            failed: report.dirs_failed,
            failures: report.failures.clone(),
            partial: report.partial,
        });
    }
}

fn item_dir(item: &ScanItem) -> &RelPath {
    match item {
        ScanItem::Listed { dir, .. } | ScanItem::Unchanged { dir } | ScanItem::Failed { dir, .. } => dir,
    }
}

/// Updates the report and returns how many entries the item carries.
fn count(item: &ScanItem, report: &mut ScanReport) -> usize {
    match item {
        ScanItem::Listed { children, .. } => {
            report.dirs_listed += 1;
            report.entries_seen += children.len() as u64;
            children.len().max(1)
        }
        ScanItem::Unchanged { .. } => {
            report.dirs_unchanged += 1;
            1
        }
        ScanItem::Failed { dir, message } => {
            report.dirs_failed += 1;
            if report.failures.len() < MAX_FAILURES_LISTED {
                report.failures.push(format!("{dir}: {message}"));
            }
            1
        }
    }
}
