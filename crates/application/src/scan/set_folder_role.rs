use std::sync::Arc;

use hoarddeck_domain::{LibraryId, RelPath};

use super::{CompiledRules, FolderRole, FolderRule, LibraryLocks, regroup};
use crate::AppError;
use crate::ports::{EntryStore, LibraryRepository, ModelRepository, SearchIndex};

/// The user decides what a folder is: one model, a collection, or back to automatic detection (`None`).
/// Regroups at once from the stored entries, so it takes seconds even for a network library.
pub struct SetFolderRole {
    pub libraries: Arc<dyn LibraryRepository>,
    pub entries: Arc<dyn EntryStore>,
    pub models: Arc<dyn ModelRepository>,
    pub search: Arc<dyn SearchIndex>,
    pub locks: Arc<LibraryLocks>,
}

impl SetFolderRole {
    /// # Errors
    /// Fails for the library root, an unknown or busy library, or when the index cannot be written.
    pub fn execute(&self, library: LibraryId, folder: &RelPath, role: Option<FolderRole>) -> Result<(), AppError> {
        if folder.is_root() {
            return Err(AppError::InvalidInput("the library root cannot be a model or collection".to_owned()));
        }
        let _busy = self.locks.lock(library)?;
        self.libraries.get(library)?.ok_or(AppError::LibraryNotFound(library))?;
        let mut rules = self.libraries.scan_rules(library)?;
        let own_pattern = FolderRule::for_folder(folder, FolderRole::Model).pattern;
        rules.folder_rules.retain(|rule| rule.pattern != own_pattern);
        if let Some(role) = role {
            rules.folder_rules.push(FolderRule::for_folder(folder, role));
        }
        let compiled = CompiledRules::compile(&rules)?;
        self.libraries.save_scan_rules(library, &rules)?;
        regroup(self.entries.as_ref(), self.models.as_ref(), self.search.as_ref(), library, &compiled)?;
        Ok(())
    }
}
