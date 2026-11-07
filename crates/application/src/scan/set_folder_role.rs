use std::sync::Arc;

use hoarddeck_domain::{LibraryId, RelPath};

use super::{FolderRole, FolderRule, RuleEditor};
use crate::AppError;

/// The user decides what a folder is: one model, a collection, or back to automatic detection (`None`).
/// Regroups at once from the stored entries, so it takes seconds even for a network library.
pub struct SetFolderRole {
    pub editor: Arc<RuleEditor>,
}

impl SetFolderRole {
    /// # Errors
    /// Fails for the library root, an unknown or busy library, or when the index cannot be written.
    pub fn execute(&self, library: LibraryId, folder: &RelPath, role: Option<FolderRole>) -> Result<(), AppError> {
        if folder.is_root() {
            return Err(AppError::InvalidInput("the library root cannot be a model or collection".to_owned()));
        }
        self.editor.edit(library, |rules| {
            let own_pattern = FolderRule::for_folder(folder, FolderRole::Model).pattern;
            rules.folder_rules.retain(|rule| rule.pattern != own_pattern);
            if let Some(role) = role {
                rules.folder_rules.push(FolderRule::for_folder(folder, role));
            }
        })
    }
}
