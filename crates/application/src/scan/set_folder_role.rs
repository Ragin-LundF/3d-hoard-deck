use std::sync::Arc;

use hoarddeck_domain::{LibraryId, RelPath};

use super::{FolderRole, FolderRule, RuleEditor, ScanRules};
use crate::AppError;

/// The user decides what a folder is: one model, a collection, or back to automatic detection (`None`).
/// Regroups from the stored entries without reading the library: [`Self::execute`] waits for it, [`Self::queue`] does
/// it in the background.
pub struct SetFolderRole {
    pub editor: Arc<RuleEditor>,
}

impl SetFolderRole {
    /// # Errors
    /// Fails for the library root, an unknown or busy library, or when the index cannot be written.
    pub fn execute(&self, library: LibraryId, folder: &RelPath, role: Option<FolderRole>) -> Result<(), AppError> {
        self.editor.edit(library, change(folder, role)?)
    }

    /// Like [`Self::execute`], but returns once the rule is saved; the regroup follows in the background.
    /// # Errors
    /// Fails for the library root, an unknown library, or when the index cannot be written.
    pub fn queue(&self, library: LibraryId, folder: &RelPath, role: Option<FolderRole>) -> Result<(), AppError> {
        self.editor.edit_soon(library, change(folder, role)?)
    }
}

fn change(folder: &RelPath, role: Option<FolderRole>) -> Result<impl FnOnce(&mut ScanRules) + '_, AppError> {
    if folder.is_root() {
        return Err(AppError::InvalidInput("the library root cannot be a model or collection".to_owned()));
    }
    Ok(move |rules: &mut ScanRules| {
        let own_pattern = FolderRule::for_folder(folder, FolderRole::Model).pattern;
        rules.folder_rules.retain(|rule| rule.pattern != own_pattern);
        if let Some(role) = role {
            rules.folder_rules.push(FolderRule::for_folder(folder, role));
        }
    })
}
