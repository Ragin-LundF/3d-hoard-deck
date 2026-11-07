//! Duplicate file groups.

use hoarddeck_application::organize::DuplicateGroup;
use serde::Serialize;
use specta::Type;

use super::id32;

#[derive(Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateFileDto {
    pub library: u32,
    pub path: String,
    pub model: Option<u32>,
}

#[derive(Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateGroupDto {
    pub size: f64,
    pub files: Vec<DuplicateFileDto>,
}

impl From<DuplicateGroup> for DuplicateGroupDto {
    #[allow(clippy::cast_precision_loss)] // sizes beyond 2^53 bytes are not a concern
    fn from(group: DuplicateGroup) -> Self {
        Self {
            size: group.size as f64,
            files: group
                .files
                .into_iter()
                .map(|file| DuplicateFileDto {
                    library: id32(file.library.0),
                    path: file.path.into(),
                    model: file.model.map(|model| id32(model.0)),
                })
                .collect(),
        }
    }
}
