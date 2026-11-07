use std::path::Path;
use std::sync::Arc;

use hoarddeck_application::AppError;
use hoarddeck_application::browse::{BrowseFolder, GetFolderChildren, GetModel};
use hoarddeck_application::library::AddLibrary;
use hoarddeck_application::ports::{EventSink, LibraryRepository};
use hoarddeck_application::scan::{LibraryLocks, ScanLibrary, SetFolderRole};
use hoarddeck_application::search::{RebuildSearchIndex, SearchModels};
use hoarddeck_infra_fs::FsLibraryScanner;
use hoarddeck_infra_sqlite::SqliteIndex;
use hoarddeck_infra_tantivy::TantivySearchIndex;

/// All adapters built once and injected into the use cases.
pub struct App {
    pub libraries: Arc<dyn LibraryRepository>,
    pub add_library: AddLibrary,
    pub scan_library: ScanLibrary,
    pub set_folder_role: SetFolderRole,
    pub browse_folder: BrowseFolder,
    pub get_folder_children: GetFolderChildren,
    pub get_model: GetModel,
    pub search_models: SearchModels,
}

impl App {
    /// Opens (or creates) the index in `index_dir`; rebuilds the search index from SQLite when it was reset.
    ///
    /// # Errors
    /// Fails when the index or search index cannot be opened.
    pub fn open(index_dir: &Path, scan_threads: usize, events: Arc<dyn EventSink>) -> Result<Self, AppError> {
        let index = Arc::new(SqliteIndex::open(index_dir)?);
        // Resolved after SqliteIndex created the folder, so symlinks and relative paths cannot dodge the
        // "index inside a library" guard.
        let index_dir = &std::fs::canonicalize(index_dir).unwrap_or_else(|_| index_dir.to_path_buf());
        let search = Arc::new(TantivySearchIndex::open(&index_dir.join("search"))?);
        if search.was_reset() {
            RebuildSearchIndex { libraries: index.clone(), models: index.clone(), search: search.clone() }.execute()?;
        }
        let locks = Arc::new(LibraryLocks::default());
        Ok(Self {
            libraries: index.clone(),
            add_library: AddLibrary { libraries: index.clone(), index_dir: index_dir.clone() },
            scan_library: ScanLibrary::new(
                index.clone(),
                index.clone(),
                index.clone(),
                search.clone(),
                Arc::new(FsLibraryScanner::new(scan_threads)),
                events,
                locks.clone(),
            ),
            set_folder_role: SetFolderRole {
                libraries: index.clone(),
                entries: index.clone(),
                models: index.clone(),
                search: search.clone(),
                locks,
            },
            browse_folder: BrowseFolder { models: index.clone() },
            get_folder_children: GetFolderChildren { models: index.clone() },
            get_model: GetModel { models: index.clone() },
            search_models: SearchModels { search, models: index },
        })
    }
}
