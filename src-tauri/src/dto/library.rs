//! Libraries and folders.

use hoarddeck_application::library::ExistingIndex;
use hoarddeck_domain::{AccessMode, Folder, Library, MetadataStorage};
use serde::{Deserialize, Serialize};
use specta::Type;

use super::id32;

#[derive(Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct LibraryDto {
    pub id: u32,
    pub name: String,
    pub root: String,
    pub access: AccessModeDto,
    pub metadata: MetadataStorageDto,
}

/// What to do when the chosen index folder already holds an index.
#[derive(Debug, Clone, Copy, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum ExistingIndexDto {
    Ask,
    Use,
    Replace,
}

impl From<ExistingIndexDto> for ExistingIndex {
    fn from(existing: ExistingIndexDto) -> Self {
        match existing {
            ExistingIndexDto::Ask => Self::Ask,
            ExistingIndexDto::Use => Self::Use,
            ExistingIndexDto::Replace => Self::Replace,
        }
    }
}

/// What the app may write into the library.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum AccessModeDto {
    ReadOnly,
    MetadataWrite,
    Managed,
}

/// Where tags, names and notes are kept.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum MetadataStorageDto {
    IndexOnly,
    IndexAndSidecar,
}

impl From<AccessMode> for AccessModeDto {
    fn from(access: AccessMode) -> Self {
        match access {
            AccessMode::ReadOnly => Self::ReadOnly,
            AccessMode::MetadataWrite => Self::MetadataWrite,
            AccessMode::Managed => Self::Managed,
        }
    }
}

impl From<AccessModeDto> for AccessMode {
    fn from(access: AccessModeDto) -> Self {
        match access {
            AccessModeDto::ReadOnly => Self::ReadOnly,
            AccessModeDto::MetadataWrite => Self::MetadataWrite,
            AccessModeDto::Managed => Self::Managed,
        }
    }
}

impl From<MetadataStorage> for MetadataStorageDto {
    fn from(storage: MetadataStorage) -> Self {
        match storage {
            MetadataStorage::IndexOnly => Self::IndexOnly,
            MetadataStorage::IndexAndSidecar => Self::IndexAndSidecar,
        }
    }
}

impl From<MetadataStorageDto> for MetadataStorage {
    fn from(storage: MetadataStorageDto) -> Self {
        match storage {
            MetadataStorageDto::IndexOnly => Self::IndexOnly,
            MetadataStorageDto::IndexAndSidecar => Self::IndexAndSidecar,
        }
    }
}

impl From<Library> for LibraryDto {
    fn from(library: Library) -> Self {
        Self {
            id: id32(library.id.0),
            access: library.settings.access().into(),
            metadata: library.settings.metadata().into(),
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
