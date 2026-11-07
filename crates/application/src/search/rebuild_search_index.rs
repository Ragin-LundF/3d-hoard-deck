use std::sync::Arc;

use crate::AppError;
use crate::ports::{LibraryRepository, ModelRepository, SearchIndex};

/// Recreates the search documents of every library from the index (after a schema change or a lost search folder).
pub struct RebuildSearchIndex {
    pub libraries: Arc<dyn LibraryRepository>,
    pub models: Arc<dyn ModelRepository>,
    pub search: Arc<dyn SearchIndex>,
}

impl RebuildSearchIndex {
    /// Returns the number of indexed models.
    /// # Errors
    /// Fails when the index cannot be read or the search index cannot be written.
    pub fn execute(&self) -> Result<usize, AppError> {
        let mut indexed = 0;
        for library in self.libraries.list()? {
            let docs = self.models.search_documents(library.id)?;
            self.search.replace_library(library.id, &docs)?;
            indexed += docs.len();
        }
        Ok(indexed)
    }
}
