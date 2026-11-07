use std::sync::Arc;

use hoarddeck_domain::{Folder, LibraryId, RelPath};

use crate::AppError;
use crate::ports::ModelRepository;

/// One level of the folder tree, loaded when a folder is expanded.
pub struct GetFolderChildren {
    pub models: Arc<dyn ModelRepository>,
}

impl GetFolderChildren {
    /// # Errors
    /// Fails when the index cannot be read.
    pub fn execute(&self, library: LibraryId, folder: &RelPath) -> Result<Vec<Folder>, AppError> {
        Ok(self.models.child_folders(library, folder)?)
    }
}
