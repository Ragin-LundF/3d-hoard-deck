use hoarddeck_domain::LibraryId;

use super::{CompiledRules, LibraryLocks, detect};
use crate::AppError;
use crate::ports::{EntryStore, ModelRepository, SearchIndex};

/// Changed models beyond this (or a quarter of the library) rebuild the search index instead of updating it.
const FULL_REBUILD_ABOVE: usize = 2000;

/// Re-detects folders and models from the stored entries (no disk access) and refreshes the search index.
/// Returns the number of models and folders.
pub(crate) fn regroup(
    entries: &dyn EntryStore,
    models: &dyn ModelRepository,
    search: &dyn SearchIndex,
    locks: &LibraryLocks,
    library: LibraryId,
    rules: &CompiledRules,
) -> Result<(usize, usize), AppError> {
    let catalog = detect(&entries.files(library)?, rules);
    let change = models.replace_catalog(library, &catalog)?;
    let _sync = locks.search_sync();
    // Few changes: update just those documents. Many (the first regroup after an update, a big rescan): one rebuild
    // is cheaper than thousands of single lookups.
    if change.changed.len() > FULL_REBUILD_ABOVE.max(catalog.models.len() / 4) {
        search.replace_library(library, &models.search_documents(library)?)?;
    } else if !change.changed.is_empty() || !change.removed.is_empty() {
        let documents = change
            .changed
            .iter()
            .map(|id| models.search_document(*id))
            .filter_map(Result::transpose)
            .collect::<Result<Vec<_>, _>>()?;
        search.apply_changes(&documents, &change.removed)?;
    }
    Ok((catalog.models.len(), catalog.folders.len()))
}
