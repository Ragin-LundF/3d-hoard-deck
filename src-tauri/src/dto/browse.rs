//! Folder browsing requests and pages, folder roles.

use hoarddeck_application::browse::{Cursor, FolderPage, Sort};
use hoarddeck_application::scan::FolderRole;
use hoarddeck_domain::ModelId;
use serde::{Deserialize, Serialize};
use specta::Type;

use super::id32;
use super::library::FolderDto;
use super::model::ModelSummaryDto;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum FolderRoleDto {
    Model,
    Collection,
}

impl From<FolderRoleDto> for FolderRole {
    fn from(role: FolderRoleDto) -> Self {
        match role {
            FolderRoleDto::Model => Self::Model,
            FolderRoleDto::Collection => Self::Collection,
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum SortDto {
    Name,
    Added,
    Released,
}

impl From<SortDto> for Sort {
    fn from(sort: SortDto) -> Self {
        match sort {
            SortDto::Name => Self::Name,
            SortDto::Added => Self::Added,
            SortDto::Released => Self::Released,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CursorDto {
    pub key: String,
    pub id: u32,
}

impl From<Cursor> for CursorDto {
    fn from(cursor: Cursor) -> Self {
        Self { key: cursor.key, id: id32(cursor.id.0) }
    }
}

impl From<CursorDto> for Cursor {
    fn from(cursor: CursorDto) -> Self {
        Self { key: cursor.key, id: ModelId(i64::from(cursor.id)) }
    }
}

#[derive(Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct BrowseRequestDto {
    pub library: u32,
    pub folder: String,
    pub recursive: bool,
    pub sort: SortDto,
    pub cursor: Option<CursorDto>,
    pub limit: u32,
}

#[derive(Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct FolderPageDto {
    pub folders: Vec<FolderDto>,
    pub models: Vec<ModelSummaryDto>,
    pub next: Option<CursorDto>,
}

impl From<FolderPage> for FolderPageDto {
    fn from(page: FolderPage) -> Self {
        Self {
            folders: page.folders.into_iter().map(Into::into).collect(),
            models: page.page.models.into_iter().map(Into::into).collect(),
            next: page.page.next.map(Into::into),
        }
    }
}
