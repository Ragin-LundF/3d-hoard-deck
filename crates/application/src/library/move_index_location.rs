use std::path::Path;
use std::sync::Arc;

use crate::AppError;
use crate::ports::{DerivedIndexes, IndexExport, IndexLocationStore, LibraryRepository, StorageInspector, StorageKind};
use crate::scan::LibraryLocks;

/// What to do when the chosen folder already holds an index.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExistingIndex {
    /// Stop with [`AppError::IndexExists`] so the user can decide.
    Ask,
    /// Switch to the index that is already there; the current one stays in its old folder.
    Use,
    /// Overwrite it with a copy of the current index.
    Replace,
}

/// Moves the index to a folder the user picked; the app must restart right after to switch.
/// No library may be scanned or regrouped meanwhile, so nothing written after the copy gets lost.
pub struct MoveIndexLocation {
    pub libraries: Arc<dyn LibraryRepository>,
    pub export: Arc<dyn IndexExport>,
    pub derived: Arc<dyn DerivedIndexes>,
    pub location: Arc<dyn IndexLocationStore>,
    pub storage: Arc<dyn StorageInspector>,
    pub locks: Arc<LibraryLocks>,
}

impl MoveIndexLocation {
    /// # Errors
    /// Fails for the current folder, a folder inside a library, a network or cloud-sync folder, a busy library,
    /// an existing index with [`ExistingIndex::Ask`], or when copying or saving the location fails.
    pub fn execute(&self, current: &Path, target: &Path, existing: ExistingIndex) -> Result<(), AppError> {
        if target == current {
            return Err(AppError::InvalidInput("the index is already in this folder".to_owned()));
        }
        let libraries = self.libraries.list()?;
        if let Some(library) = libraries.iter().find(|library| target.starts_with(&library.root)) {
            return Err(AppError::InvalidInput(format!("the index cannot live inside the library {}", library.name)));
        }
        match self.storage.kind(target) {
            StorageKind::Local => {}
            StorageKind::Network => return Err(AppError::UnsafeIndexLocation("a network share".to_owned())),
            StorageKind::CloudSync => return Err(AppError::UnsafeIndexLocation("a cloud-sync folder".to_owned())),
        }
        let _busy = libraries.iter().map(|library| self.locks.lock(library.id)).collect::<Result<Vec<_>, _>>()?;
        match (self.export.existing_index(target), existing) {
            (Some(modified), ExistingIndex::Ask) => return Err(AppError::IndexExists { modified }),
            (Some(_), ExistingIndex::Use) => {}
            _ => {
                self.export.export_to(target)?;
                // A replaced folder's search index belongs to the old data.
                self.derived.invalidate(target)?;
            }
        }
        self.location.save(target)?;
        Ok(())
    }
}
