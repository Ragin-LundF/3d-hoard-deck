use std::sync::Arc;

use hoarddeck_domain::LibraryId;

use super::sidecar_path;
use crate::AppError;
use crate::ports::{LibraryRepository, MetaStore, ModelRepository, SearchIndex, SidecarFiles};
use crate::scan::LibraryLocks;

/// Restores names, tags, marks and notes from the `.3dhoarddeck` files in a library, e.g. after a lost index or on a
/// new computer. A file replaces the index values when it is newer than them. Only reads the library.
pub struct RebuildFromSidecars {
    pub libraries: Arc<dyn LibraryRepository>,
    pub models: Arc<dyn ModelRepository>,
    pub meta: Arc<dyn MetaStore>,
    pub sidecars: Arc<dyn SidecarFiles>,
    pub search: Arc<dyn SearchIndex>,
    pub locks: Arc<LibraryLocks>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RebuildReport {
    pub restored: usize,
    pub unreadable: usize,
}

impl RebuildFromSidecars {
    /// # Errors
    /// Fails for an unknown library or when the index cannot be written. Broken files are counted, not fatal.
    pub fn execute(&self, id: LibraryId) -> Result<RebuildReport, AppError> {
        let library = self.libraries.get(id)?.ok_or(AppError::LibraryNotFound(id))?;
        let mut report = RebuildReport::default();
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
}
