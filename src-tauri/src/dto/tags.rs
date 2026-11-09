use hoarddeck_application::ports::{TagGroup, TagInfo};
use serde::{Deserialize, Serialize};
use specta::Type;

/// A tag in use, with its group and pin.
#[derive(Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TagInfoDto {
    pub name: String,
    pub count: u32,
    pub group: Option<f64>,
    pub pinned: bool,
    /// When a model carrying it last changed, seconds since 1970.
    pub recent: f64,
}

#[allow(clippy::cast_precision_loss)] // ids and seconds stay far below 2^53
impl From<TagInfo> for TagInfoDto {
    fn from(tag: TagInfo) -> Self {
        Self {
            name: tag.name.as_str().to_owned(),
            count: tag.count,
            group: tag.group.map(|id| id as f64),
            pinned: tag.pinned,
            recent: tag.recent as f64,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TagGroupDto {
    pub id: f64,
    pub name: String,
    pub color: String,
}

#[allow(clippy::cast_precision_loss)] // ids stay far below 2^53
impl From<TagGroup> for TagGroupDto {
    fn from(group: TagGroup) -> Self {
        Self { id: group.id as f64, name: group.name, color: group.color }
    }
}
