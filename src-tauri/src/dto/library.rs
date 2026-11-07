//! Libraries and folders.

use hoarddeck_domain::{Folder, Library};
use serde::Serialize;
use specta::Type;

use super::id32;

#[derive(Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct LibraryDto {
    pub id: u32,
    pub name: String,
    pub root: String,
    pub read_only: bool,
}

impl From<Library> for LibraryDto {
    fn from(library: Library) -> Self {
        Self {
            id: id32(library.id.0),
            read_only: library.access == hoarddeck_domain::AccessMode::ReadOnly,
            name: library.name,
            root: library.root,
        }
    }
}

#[derive(Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct FolderDto {
    pub path: String,
    pub name: String,
    pub released: Option<String>,
    pub direct_models: u32,
    pub total_models: u32,
    pub cover: Option<String>,
    pub is_collection: bool,
}

impl From<Folder> for FolderDto {
    fn from(folder: Folder) -> Self {
        Self {
            path: folder.path.into(),
            name: folder.display_name,
            released: folder.released.map(|r| r.to_string()),
            direct_models: folder.direct_models,
            total_models: folder.total_models,
            cover: folder.cover.map(String::from),
            is_collection: folder.is_collection,
        }
    }
}
