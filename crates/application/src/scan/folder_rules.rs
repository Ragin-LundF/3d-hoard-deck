use std::sync::Arc;

use hoarddeck_domain::LibraryId;

use super::{FolderRule, RuleEditor};
use crate::AppError;

/// Lists and removes a library's folder rules (marks on single folders and accepted presets).
pub struct FolderRules {
    pub editor: Arc<RuleEditor>,
}

impl FolderRules {
    /// # Errors
    /// Fails when the index cannot be read.
    pub fn list(&self, library: LibraryId) -> Result<Vec<FolderRule>, AppError> {
        Ok(self.editor.rules(library)?.folder_rules)
    }

    /// # Errors
    /// Fails for an unknown or busy library, or when the index cannot be written.
    pub fn remove(&self, library: LibraryId, pattern: &str) -> Result<(), AppError> {
        self.editor.edit(library, |rules| rules.folder_rules.retain(|rule| rule.pattern != pattern))
    }

    /// Whether models without a picture borrow one from the folder above.
    /// # Errors
    /// Fails when the index cannot be read.
    pub fn parent_covers(&self, library: LibraryId) -> Result<bool, AppError> {
        Ok(self.editor.rules(library)?.parent_covers)
    }

    /// Switches borrowing pictures from the folder above on or off; the regroup follows in the background.
    /// # Errors
    /// Fails for an unknown library, or when the index cannot be written.
    pub fn set_parent_covers(&self, library: LibraryId, on: bool) -> Result<(), AppError> {
        self.editor.edit_soon(library, |rules| rules.parent_covers = on)
    }

    /// Like [`Self::remove`], but the regroup follows in the background.
    /// # Errors
    /// Fails for an unknown library, or when the index cannot be written.
    pub fn queue_remove(&self, library: LibraryId, pattern: &str) -> Result<(), AppError> {
        self.editor.edit_soon(library, |rules| rules.folder_rules.retain(|rule| rule.pattern != pattern))
    }
}
