use std::sync::Arc;
use std::sync::mpsc::sync_channel;
use std::thread;

use hoarddeck_domain::{Entry, Library, LibraryId, RelPath};

use super::{CompiledRules, KnownDirs, LibraryLocks, ScanItem, ScanPhase, ScanProgress, regroup};
use crate::ports::{EntryStore, EventSink, LibraryRepository, LibraryScanner, ModelRepository, SearchIndex};
use crate::{AppError, ScanError};

/// Entries per index transaction while walking.
const BATCH_ENTRIES: usize = 2_000;
/// Walk results buffered between the walker threads and the index writer; keeps memory flat.
const CHANNEL_CAPACITY: usize = 256;
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
}

impl ScanLibrary {
    #[must_use]
    pub fn new(
        libraries: Arc<dyn LibraryRepository>,
        entries: Arc<dyn EntryStore>,
        models: Arc<dyn ModelRepository>,
        search: Arc<dyn SearchIndex>,
        scanner: Arc<dyn LibraryScanner>,
        events: Arc<dyn EventSink>,
        locks: Arc<LibraryLocks>,
    ) -> Self {
        Self { libraries, entries, models, search, scanner, events, locks }
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
        (report.models, report.folders) =
            regroup(self.entries.as_ref(), self.models.as_ref(), self.search.as_ref(), &self.locks, id, &rules)?;
        self.progress(id, ScanPhase::Done, &report);
        Ok(report)
    }

    /// Puts files the app itself just wrote into the index and regroups, without walking the library (a network share
    /// takes seconds for that). The folder is marked so the next scan lists it properly.
    /// # Errors
    /// Fails when the library is busy or the index cannot be written.
    pub fn add_files(&self, id: LibraryId, dir: &RelPath, files: Vec<Entry>) -> Result<(), AppError> {
        let _busy = self.locks.lock(id)?;
        let rules = CompiledRules::compile(&self.libraries.scan_rules(id)?)?;
        // A fresh generation without `finish_scan` deletes nothing; the next full scan settles everything again.
        let (generation, _) = self.entries.begin_scan(id)?;
        let folder = ScanItem::Listed { dir: dir.clone(), mtime: i64::MIN, children: files };
        self.entries.apply(id, generation, &[folder])?;
        // No progress events: a "done" would replace the last real scan's report (unreadable folders) in the UI.
        regroup(self.entries.as_ref(), self.models.as_ref(), self.search.as_ref(), &self.locks, id, &rules)?;
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
        if mode == ScanMode::Full {
            known = KnownDirs::new();
        }
        // A folder the scan does not know is always listed.
        for dir in changed {
            known.remove(dir);
        }
        let (sender, receiver) = sync_channel::<ScanItem>(CHANNEL_CAPACITY);
        let mut report = ScanReport { partial: !from.is_root(), ..ScanReport::default() };
        thread::scope(|scope| -> Result<(), AppError> {
            let (scanner, known, root) = (&self.scanner, &known, library.root.as_str());
            let walker = scope.spawn(move || scanner.scan(root, from, known, rules, &sender));
            let mut batch = Vec::new();
            let mut batch_entries = 0;
            for item in receiver {
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
        report.entries_removed = self.entries.finish_folder_scan(library.id, generation, from)?;
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
