use std::sync::Arc;

use hoarddeck_domain::{Library, LibraryId};

use super::{CompiledRules, LibraryLocks, ScanRules, regroup};
use crate::AppError;
use crate::library::writes_sidecars;
use crate::ports::{EntryStore, LibraryRepository, ModelRepository, SearchIndex, SidecarFiles};

/// Changes a library's scan rules and regroups from the stored entries, holding the library lock meanwhile.
/// Shared by the use cases that edit folder rules.
pub struct RuleEditor {
    pub libraries: Arc<dyn LibraryRepository>,
    pub entries: Arc<dyn EntryStore>,
    pub models: Arc<dyn ModelRepository>,
    pub search: Arc<dyn SearchIndex>,
    pub locks: Arc<LibraryLocks>,
    pub sidecars: Arc<dyn SidecarFiles>,
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
