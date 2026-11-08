use std::sync::Arc;

use hoarddeck_domain::{Folder, RelPath};

use super::{BrowsePage, BrowseRequest};
use crate::AppError;
use crate::ports::ModelRepository;

const MAX_PAGE: usize = 500;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FolderPage {
    /// Child folders; only on the first page of the direct view.
    pub folders: Vec<Folder>,
    /// Pictures lying in the folder that belong to no model; only on the first page of the direct view.
    pub images: Vec<RelPath>,
    pub page: BrowsePage,
}

/// One page of a folder: its subfolders (with counts and covers) and its models.
pub struct BrowseFolder {
    pub models: Arc<dyn ModelRepository>,
}

impl BrowseFolder {
    /// # Errors
    /// Fails when the index cannot be read.
    pub fn execute(&self, request: &BrowseRequest) -> Result<FolderPage, AppError> {
        let request = BrowseRequest { limit: request.limit.clamp(1, MAX_PAGE), ..request.clone() };
        let first_direct_page = !request.recursive && request.cursor.is_none();
        let (folders, images) = if first_direct_page {
            (
                self.models.child_folders(request.library, &request.folder)?,
                self.models.loose_images(request.library, &request.folder)?,
            )
        } else {
            (Vec::new(), Vec::new())
        };
        Ok(FolderPage { folders, images, page: self.models.browse(&request)? })
    }
}
