//! Collections.

use hoarddeck_domain::Collection;
use serde::Serialize;
use specta::Type;

use super::id32;

#[derive(Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CollectionDto {
    pub id: u32,
    pub name: String,
    /// Search text of a smart collection; `None` for a hand-made one.
    pub query: Option<String>,
    /// Members of a hand-made collection; `None` for smart ones.
    pub count: Option<u32>,
}

impl From<(Collection, Option<u32>)> for CollectionDto {
    fn from((collection, count): (Collection, Option<u32>)) -> Self {
        Self { id: id32(collection.id.0), name: collection.name, query: collection.query, count }
    }
}
