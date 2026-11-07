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
}
