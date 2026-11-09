//! Search: query parsing and the search use case. Ranking and fuzzy matching live in the search adapter.

mod query;
mod rebuild_search_index;
mod search_models;

use hoarddeck_domain::{FileFormat, LibraryId, ModelId, ModelKind, RelPath};

pub use query::{ParsedQuery, parse_query};
pub use rebuild_search_index::RebuildSearchIndex;
pub use search_models::{SearchModels, SearchRequest, SearchResult};

/// One model as the search index sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchDocument {
    pub id: ModelId,
    pub library: LibraryId,
    pub kind: ModelKind,
    /// Display name and original name, so both find the model.
    pub title: String,
    pub path: RelPath,
    pub folder: RelPath,
    pub formats: Vec<FileFormat>,
    pub tags: Vec<String>,
    pub notes: String,
    pub favorite: bool,
    pub printed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchQuery {
    pub words: Vec<String>,
    pub formats: Vec<FileFormat>,
    /// Lower-case tags; every one must be present.
    pub tags: Vec<String>,
    /// Lower-case tags; at least one must be present (when any are given).
    pub any_tags: Vec<String>,
    /// Lower-case tags; none may be present.
    pub excluded_tags: Vec<String>,
    pub favorite: bool,
    pub printed: bool,
    /// Restricts results to one library, optionally to a folder inside it (recursive).
    pub library: Option<LibraryId>,
    pub scope: Option<RelPath>,
    pub offset: usize,
    pub limit: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SearchHits {
    pub ids: Vec<ModelId>,
    pub total: usize,
}
