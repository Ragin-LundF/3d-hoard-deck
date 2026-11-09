use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

use hoarddeck_domain::{Library, LibraryId};

use super::{CompiledRules, LibraryLocks, ScanPhase, ScanProgress, ScanRules, regroup};
use crate::AppError;
use crate::library::writes_sidecars;
use crate::ports::{EntryStore, EventSink, LibraryRepository, ModelRepository, SearchIndex, SidecarFiles};

/// Changes arriving within this time are grouped together, so marking several folders in a row regroups once.
const QUIET: Duration = Duration::from_millis(400);
/// How often a queued regroup looks again while a scan holds the library.
const BUSY_RETRY: Duration = Duration::from_millis(500);

/// Changes a library's scan rules and regroups from the stored entries. Shared by the use cases that edit folder
/// rules: [`Self::edit`] waits for the regroup, [`Self::edit_soon`] returns at once and regroups in the background.
pub struct RuleEditor {
    pub libraries: Arc<dyn LibraryRepository>,
    pub entries: Arc<dyn EntryStore>,
    pub models: Arc<dyn ModelRepository>,
    pub search: Arc<dyn SearchIndex>,
    pub locks: Arc<LibraryLocks>,
    pub sidecars: Arc<dyn SidecarFiles>,
    /// Tells the UI that a background regroup started and finished.
    pub events: Arc<dyn EventSink>,
    /// Serializes rule edits (read, change, save) and counts requested regroups per library.
    pub queue: Mutex<HashMap<LibraryId, u64>>,
}

impl RuleEditor {
    /// # Errors
    /// Fails for an unknown or busy library, invalid rules, or when the index cannot be written.
    pub(crate) fn edit(&self, library: LibraryId, change: impl FnOnce(&mut ScanRules)) -> Result<(), AppError> {
        let _busy = self.locks.lock(library)?;
        let found = self.libraries.get(library)?.ok_or(AppError::LibraryNotFound(library))?;
        let mut rules = self.libraries.scan_rules(library)?;
        change(&mut rules);
        let compiled = CompiledRules::compile(&rules)?;
        self.libraries.save_scan_rules(library, &rules)?;
        save_rules_file(self.sidecars.as_ref(), &found, &rules);
        regroup(self.entries.as_ref(), self.models.as_ref(), self.search.as_ref(), &self.locks, library, &compiled)?;
        Ok(())
    }

    /// Saves the changed rules now and regroups in the background. A scan in progress is waited for, and several
    /// edits in a row are regrouped once.
    /// # Errors
    /// Fails for an unknown library, invalid rules, or when the index cannot be written.
    pub(crate) fn edit_soon(
        self: &Arc<Self>,
        library: LibraryId,
        change: impl FnOnce(&mut ScanRules),
    ) -> Result<(), AppError> {
        let ticket = {
            let mut queue = self.queue();
            let found = self.libraries.get(library)?.ok_or(AppError::LibraryNotFound(library))?;
            let mut rules = self.libraries.scan_rules(library)?;
            change(&mut rules);
            CompiledRules::compile(&rules)?;
            self.libraries.save_scan_rules(library, &rules)?;
            save_rules_file(self.sidecars.as_ref(), &found, &rules);
            let ticket = queue.entry(library).or_default();
            *ticket += 1;
            *ticket
        };
        let editor = Arc::clone(self);
        thread::spawn(move || editor.regroup_when_quiet(library, ticket));
        Ok(())
    }

    /// Waits until no background regroup is pending, or `timeout` passed. Returns whether all are done.
    pub fn wait_for_regroups(&self, timeout: Duration) -> bool {
        let deadline = Instant::now() + timeout;
        while !self.queue().is_empty() {
            if Instant::now() >= deadline {
                return false;
            }
            thread::sleep(Duration::from_millis(20));
        }
        true
    }

    fn regroup_when_quiet(&self, library: LibraryId, ticket: u64) {
        thread::sleep(QUIET);
        loop {
            // A newer edit has its own thread, which will regroup with all changes.
            if self.queue().get(&library) != Some(&ticket) {
                return;
            }
            match self.locks.lock(library) {
                Err(AppError::LibraryBusy(_)) => thread::sleep(BUSY_RETRY),
                Err(_) => break,
                Ok(_busy) => {
                    self.progress(library, ScanPhase::Grouping);
                    let result = CompiledRules::compile(&self.libraries.scan_rules(library).unwrap_or_default())
                        .and_then(|rules| {
                            regroup(
                                self.entries.as_ref(),
                                self.models.as_ref(),
                                self.search.as_ref(),
                                &self.locks,
                                library,
                                &rules,
                            )
                        });
                    if let Err(error) = result {
                        // shortcut: the UI is not told about a failed background regroup; the next scan regroups.
                        eprintln!("regrouping library {} failed: {error}", library.0);
                    }
                    self.progress(library, ScanPhase::Done);
                    break;
                }
            }
        }
        let mut queue = self.queue();
        if queue.get(&library) == Some(&ticket) {
            queue.remove(&library);
        }
    }

    fn progress(&self, library: LibraryId, phase: ScanPhase) {
        // `partial`: this is no scan, so the UI keeps the last scan's report of unreadable folders.
        let progress =
            ScanProgress { library, phase, dirs: 0, files: 0, failed: 0, failures: Vec::new(), partial: true };
        self.events.scan_progress(&progress);
    }

    fn queue(&self) -> MutexGuard<'_, HashMap<LibraryId, u64>> {
        self.queue.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// # Errors
    /// Fails when the index cannot be read.
    pub(crate) fn rules(&self, library: LibraryId) -> Result<ScanRules, AppError> {
        Ok(self.libraries.scan_rules(library)?)
    }
}

/// Copies the folder rules into the `.3dhoarddeck` at the library root when the library keeps metadata files, so a
/// rebuild restores them. The index stays the source of truth: a failed write is caught up by the next rule change.
pub(crate) fn save_rules_file(sidecars: &dyn SidecarFiles, library: &Library, rules: &ScanRules) {
    if writes_sidecars(library.settings) {
        let _ = sidecars.write_folder_rules(&library.root, &rules.folder_rules);
    }
}
