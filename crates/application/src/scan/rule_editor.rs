use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

use hoarddeck_domain::{Library, LibraryId};

use super::{CompiledRules, LibraryLocks, ScanPhase, ScanProgress, ScanRules, regroup};
use crate::AppError;
use crate::library::writes_sidecars;
use crate::ports::{EntryStore, EventSink, LibraryRepository, ModelRepository, SearchIndex, SidecarFiles};
use crate::tagging::TagRuleBook;

/// Changes arriving within this time are grouped together, so marking several folders in a row regroups once.
const QUIET: Duration = Duration::from_millis(400);

/// Changes a library's scan rules and regroups from the stored entries. Shared by the use cases that edit folder
/// rules: [`Self::edit`] waits for the regroup, [`Self::edit_soon`] returns at once and regroups in the background.
pub struct RuleEditor {
    pub libraries: Arc<dyn LibraryRepository>,
    pub entries: Arc<dyn EntryStore>,
    pub models: Arc<dyn ModelRepository>,
    pub search: Arc<dyn SearchIndex>,
    pub locks: Arc<LibraryLocks>,
    pub sidecars: Arc<dyn SidecarFiles>,
    pub tag_rules: Arc<TagRuleBook>,
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
        let rules = self.change_rules(library, change)?;
        let compiled = CompiledRules::compile(&rules)?;
        save_rules_file(self.sidecars.as_ref(), &found, &rules);
        regroup(
            self.entries.as_ref(),
            self.models.as_ref(),
            self.search.as_ref(),
            &self.locks,
            library,
            &compiled,
            &self.tag_rules.current(),
        )?;
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
            let rules = self.change_rules(library, change)?;
            save_rules_file(self.sidecars.as_ref(), &found, &rules);
            Self::next_ticket(&mut queue, library)
        };
        let editor = Arc::clone(self);
        thread::spawn(move || editor.regroup_when_quiet(library, ticket));
        Ok(())
    }

    /// Regroups in the background without changing the folder rules, e.g. after the tag rules changed.
    pub(crate) fn regroup_soon(self: &Arc<Self>, library: LibraryId) {
        let ticket = Self::next_ticket(&mut self.queue(), library);
        let editor = Arc::clone(self);
        thread::spawn(move || editor.regroup_when_quiet(library, ticket));
    }

    /// Applies `change` and saves, in one index transaction; rules that do not compile are not saved.
    fn change_rules(&self, library: LibraryId, change: impl FnOnce(&mut ScanRules)) -> Result<ScanRules, AppError> {
        let mut change = Some(change);
        let mut refused = None;
        let saved = self.libraries.update_scan_rules(library, &mut |rules| {
            if let Some(change) = change.take() {
                change(rules);
            }
            match CompiledRules::compile(rules) {
                Ok(_) => true,
                Err(error) => {
                    refused = Some(error);
                    false
                }
            }
        })?;
        match (saved, refused) {
            (_, Some(error)) => Err(error),
            (Some(rules), None) => Ok(rules),
            (None, None) => Err(AppError::LibraryNotFound(library)),
        }
    }

    fn next_ticket(queue: &mut HashMap<LibraryId, u64>, library: LibraryId) -> u64 {
        let ticket = queue.entry(library).or_default();
        *ticket += 1;
        *ticket
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
        // A newer edit has its own thread, which will regroup with all changes.
        if self.queue().get(&library) == Some(&ticket) {
            // A scan holds the library lock for minutes on a network share. The regroup does not wait for it: the
            // regroup lock keeps it apart from the scan's own regroup, which reads the rules again. While a scan runs
            // the UI only hears "regrouped", so its scan status stays.
            let busy = self.locks.lock(library);
            let scanning = matches!(busy, Err(AppError::LibraryBusy(_)));
            if busy.is_ok() || scanning {
                if !scanning {
                    self.progress(library, ScanPhase::Grouping);
                }
                let result =
                    CompiledRules::compile(&self.libraries.scan_rules(library).unwrap_or_default()).and_then(|rules| {
                        regroup(
                            self.entries.as_ref(),
                            self.models.as_ref(),
                            self.search.as_ref(),
                            &self.locks,
                            library,
                            &rules,
                            &self.tag_rules.current(),
                        )
                    });
                if let Err(error) = result {
                    // shortcut: the UI is not told about a failed background regroup; the next scan regroups.
                    eprintln!("regrouping library {} failed: {error}", library.0);
                }
                self.progress(library, if scanning { ScanPhase::Regrouped } else { ScanPhase::Done });
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
