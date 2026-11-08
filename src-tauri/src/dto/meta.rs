//! What the user sets on a model, and tag counts.

use hoarddeck_application::AppError;
use hoarddeck_domain::{ModelMeta, TagName};
use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ModelMetaDto {
    pub display_name: Option<String>,
    pub tags: Vec<String>,
    pub favorite: bool,
    pub printed: bool,
    pub notes: String,
    /// An image path of the model picked as cover, or `None` for the detected one.
    pub cover: Option<String>,
}

impl From<ModelMeta> for ModelMetaDto {
    fn from(meta: ModelMeta) -> Self {
        Self {
            display_name: meta.display_name,
            tags: meta.tags.into_iter().map(String::from).collect(),
            favorite: meta.favorite,
            printed: meta.printed,
            notes: meta.notes,
            cover: meta.cover.map(String::from),
        }
    }
}

impl TryFrom<ModelMetaDto> for ModelMeta {
    type Error = AppError;

    fn try_from(dto: ModelMetaDto) -> Result<Self, Self::Error> {
        Ok(Self {
            display_name: dto.display_name,
            tags: dto.tags.iter().map(|tag| TagName::new(tag)).collect::<Result<_, _>>()?,
            favorite: dto.favorite,
            printed: dto.printed,
            notes: dto.notes,
            cover: dto
                .cover
                .as_deref()
                .map(super::rel_path)
                .transpose()
                .map_err(|_| AppError::InvalidInput("invalid cover path".to_owned()))?,
        })
    }
}

#[derive(Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TagCountDto {
    pub name: String,
    pub count: u32,
}

#[derive(Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ImportReportDto {
    pub updated: u32,
    pub unreadable: u32,
}

#[derive(Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RebuildReportDto {
    pub restored: u32,
    pub unreadable: u32,
    /// Folder rules (one model, collection) taken from the library root's `.3dhoarddeck`.
    pub rules_restored: u32,
}
