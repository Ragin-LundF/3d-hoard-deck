use std::sync::Arc;
use std::sync::mpsc::sync_channel;
use std::thread;

use hoarddeck_domain::{Library, LibraryId};

use super::{CompiledRules, KnownDirs, LibraryLocks, ScanItem, ScanPhase, ScanProgress, regroup};
use crate::ports::{EntryStore, EventSink, LibraryRepository, LibraryScanner, ModelRepository, SearchIndex};
use crate::{AppError, ScanError};

/// Entries per index transaction while walking.
const BATCH_ENTRIES: usize = 2_000;
/// Walk results buffered between the walker threads and the index writer; keeps memory flat.
const CHANNEL_CAPACITY: usize = 256;

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
        let _busy = self.locks.lock(id)?;
        let library = self.libraries.get(id)?.ok_or(AppError::LibraryNotFound(id))?;
        let rules = CompiledRules::compile(&self.libraries.scan_rules(id)?)?;
        let mut report = self.walk(&library, &rules, mode)?;
        self.progress(id, ScanPhase::Grouping, &report);
        (report.models, report.folders) =
            regroup(self.entries.as_ref(), self.models.as_ref(), self.search.as_ref(), &self.locks, id, &rules)?;
        self.progress(id, ScanPhase::Done, &report);
        Ok(report)
    }

    fn walk(&self, library: &Library, rules: &CompiledRules, mode: ScanMode) -> Result<ScanReport, AppError> {
        let (generation, mut known) = self.entries.begin_scan(library.id)?;
        if mode == ScanMode::Full {
            known = KnownDirs::new();
        }
        let (sender, receiver) = sync_channel::<ScanItem>(CHANNEL_CAPACITY);
        let mut report = ScanReport::default();
        thread::scope(|scope| -> Result<(), AppError> {
            let (scanner, known, root) = (&self.scanner, &known, library.root.as_str());
            let walker = scope.spawn(move || scanner.scan(root, known, rules, &sender));
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
        report.entries_removed = self.entries.finish_scan(library.id, generation)?;
        Ok(report)
    }

    fn progress(&self, library: LibraryId, phase: ScanPhase, report: &ScanReport) {
        let dirs = report.dirs_listed + report.dirs_unchanged;
        self.events.scan_progress(&ScanProgress { library, phase, dirs, files: report.entries_seen });
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
        ScanItem::Failed { .. } => {
            report.dirs_failed += 1;
            1
        }
    }
}
