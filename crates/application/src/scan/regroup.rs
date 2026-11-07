use hoarddeck_domain::LibraryId;

use super::{CompiledRules, detect};
use crate::AppError;
use crate::ports::{EntryStore, ModelRepository, SearchIndex};

/// Re-detects folders and models from the stored entries (no disk access) and refreshes the search index.
/// Returns the number of models and folders.
pub(crate) fn regroup(
    entries: &dyn EntryStore,
    models: &dyn ModelRepository,
    search: &dyn SearchIndex,
    library: LibraryId,
    rules: &CompiledRules,
) -> Result<(usize, usize), AppError> {
    let catalog = detect(&entries.files(library)?, rules);
    models.replace_catalog(library, &catalog)?;
    search.replace_library(library, &models.search_documents(library)?)?;
    Ok((catalog.models.len(), catalog.folders.len()))
}
