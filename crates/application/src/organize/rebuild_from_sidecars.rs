use std::sync::Arc;

use hoarddeck_domain::LibraryId;

use super::sidecar_path;
use crate::AppError;
use crate::ports::{LibraryRepository, MetaStore, ModelRepository, SearchIndex, SidecarFiles};
use crate::scan::{FolderRule, LibraryLocks, RuleEditor};

/// Restores names, tags, marks, notes and folder rules from the `.3dhoarddeck` files in a library, e.g. after a lost index or on a
/// new computer. A file replaces the index values when it is newer than them. Only reads the library.
pub struct RebuildFromSidecars {
    pub libraries: Arc<dyn LibraryRepository>,
    pub models: Arc<dyn ModelRepository>,
    pub meta: Arc<dyn MetaStore>,
    pub sidecars: Arc<dyn SidecarFiles>,
    pub search: Arc<dyn SearchIndex>,
    pub locks: Arc<LibraryLocks>,
    /// Restores the folder rules from the library root's `.3dhoarddeck` and regroups.
    pub editor: Arc<RuleEditor>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RebuildReport {
    pub restored: usize,
    pub unreadable: usize,
    /// Folder rules added or changed from the library root's file.
    pub rules_restored: usize,
}

impl RebuildFromSidecars {
    /// # Errors
    /// Fails for an unknown library or when the index cannot be written. Broken files are counted, not fatal.
    pub fn execute(&self, id: LibraryId) -> Result<RebuildReport, AppError> {
        let library = self.libraries.get(id)?.ok_or(AppError::LibraryNotFound(id))?;
        let mut report = RebuildReport::default();
        match self.sidecars.read_folder_rules(&library.root) {
            Ok(Some(saved)) => report.rules_restored = self.restore_rules(id, saved)?,
            Ok(None) => {}
            Err(_) => report.unreadable += 1,
        }
        for document in self.models.search_documents(id)? {
            let Some(file) = sidecar_path(&document.path, document.kind) else { continue };
            match self.sidecars.read(&library.root, &file) {
                Ok(Some((meta, written))) => {
                    let Ok(meta) = meta.normalized() else {
                        report.unreadable += 1;
                        continue;
                    };
                    let index_time = self.meta.meta_updated_at(id, &document.path)?;
                    if index_time.is_none_or(|saved| written > saved) {
                        self.meta.save_meta(id, &document.path, &meta)?;
                        report.restored += 1;
                    }
                }
                Ok(None) => {}
                Err(_) => report.unreadable += 1,
            }
        }
        let _sync = self.locks.search_sync();
        self.search.replace_library(id, &self.models.search_documents(id)?)?;
        Ok(report)
    }

    /// Rules from the file win for their folder or pattern; rules only in the index stay.
    fn restore_rules(&self, id: LibraryId, saved: Vec<FolderRule>) -> Result<usize, AppError> {
        let current = self.editor.rules(id)?.folder_rules;
        let changes = saved.into_iter().filter(|rule| !current.contains(rule)).collect::<Vec<_>>();
        if changes.is_empty() {
            return Ok(0);
        }
        let count = changes.len();
        self.editor.edit(id, |rules| {
            for rule in changes {
                rules.folder_rules.retain(|existing| existing.pattern != rule.pattern);
                rules.folder_rules.push(rule);
            }
        })?;
        Ok(count)
    }
}
