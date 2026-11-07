//! Browsing: folder pages with child folders and models, and the model page.

mod browse_folder;
mod get_folder_children;
mod get_model;

use hoarddeck_domain::{LibraryId, ModelId, RelPath};

pub use browse_folder::{BrowseFolder, FolderPage};
pub use get_folder_children::GetFolderChildren;
pub use get_model::GetModel;

use crate::ModelSummary;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Sort {
    #[default]
    Name,
    /// Newest first.
    Added,
    /// Newest release first; models without a release date last.
    Released,
}

/// Keyset position after the last model of a page. Opaque to the UI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cursor {
    pub key: String,
    pub id: ModelId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowseRequest {
    pub library: LibraryId,
    pub folder: RelPath,
    /// `true` shows everything inside the folder, `false` only its direct models.
    pub recursive: bool,
    pub sort: Sort,
    pub cursor: Option<Cursor>,
    pub limit: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowsePage {
    pub models: Vec<ModelSummary>,
    pub next: Option<Cursor>,
}
