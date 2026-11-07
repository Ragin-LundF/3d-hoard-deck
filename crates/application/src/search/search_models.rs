use std::sync::Arc;

use hoarddeck_domain::{LibraryId, RelPath};

use super::{SearchQuery, parse_query};
use crate::ports::{ModelRepository, SearchIndex};
use crate::{AppError, ModelSummary};

const MAX_PAGE: usize = 500;
/// Deeper pages make the search engine rank `offset + limit` hits; nobody scrolls that far.
const MAX_OFFSET: usize = 10_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchRequest {
    pub text: String,
    pub library: Option<LibraryId>,
    /// Folder scope from the UI; an `in:` filter in the text overrides it.
    pub scope: Option<RelPath>,
    pub offset: usize,
    pub limit: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchResult {
    pub models: Vec<ModelSummary>,
    pub total: usize,
}

/// Ranked, typo-tolerant search over model titles and paths, optionally inside a folder.
pub struct SearchModels {
    pub search: Arc<dyn SearchIndex>,
    pub models: Arc<dyn ModelRepository>,
}

impl SearchModels {
    /// # Errors
    /// Fails for a folder scope without a library, or when the search index or the index cannot be read.
    pub fn execute(&self, request: &SearchRequest) -> Result<SearchResult, AppError> {
        let parsed = parse_query(&request.text);
        let scope = parsed.scope.or_else(|| request.scope.clone()).filter(|scope| !scope.is_root());
        if scope.is_some() && request.library.is_none() {
            return Err(AppError::InvalidInput("a folder scope needs a library".to_owned()));
        }
        if request.offset > MAX_OFFSET {
            // End of the reachable results: an empty page stops infinite scrolling instead of repeating one.
            return Ok(SearchResult { models: Vec::new(), total: request.offset });
        }
        let hits = self.search.search(&SearchQuery {
            words: parsed.words,
            formats: parsed.formats,
            library: request.library,
            scope,
            offset: request.offset,
            limit: request.limit.clamp(1, MAX_PAGE),
        })?;
        Ok(SearchResult { models: self.models.summaries(&hits.ids)?, total: hits.total })
    }
}
