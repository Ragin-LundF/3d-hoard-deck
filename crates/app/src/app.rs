use std::path::{Path, PathBuf};
use std::sync::Arc;

use hoarddeck_application::AppError;
use hoarddeck_application::browse::{BrowseFolder, GetFolderChildren, GetModel};
use hoarddeck_application::library::{AddLibrary, MoveIndexLocation, UpdateLibrarySettings};
use hoarddeck_application::organize::{
    FindDuplicates, ImportOrynt3d, ListTags, ManageCollections, RebuildFromSidecars, UpdateModelMeta,
};
use hoarddeck_application::ports::{EventSink, LibraryRepository};
use hoarddeck_application::scan::{
    CollectionPresets, FolderRules, LibraryLocks, RuleEditor, ScanLibrary, SetFolderRole,
};
use hoarddeck_application::search::{RebuildSearchIndex, SearchModels};
use hoarddeck_application::slicer::{ManageSlicers, OpenInSlicer};
use hoarddeck_infra_fs::{
    Blake3Hasher, BootstrapFile, FsLibraryScanner, Orynt3dFiles, SidecarFileStore, SystemStorage,
};
use hoarddeck_infra_platform::{InstalledSlicers, SystemLauncher};
use hoarddeck_infra_sqlite::SqliteIndex;
use hoarddeck_infra_tantivy::{SearchFolder, TantivySearchIndex};

/// All adapters built once and injected into the use cases.
pub struct App {
    /// The folder this index was opened from (resolved).
    pub index_dir: PathBuf,
    pub libraries: Arc<dyn LibraryRepository>,
    pub add_library: AddLibrary,
    pub update_library_settings: UpdateLibrarySettings,
    pub move_index_location: MoveIndexLocation,
    pub scan_library: ScanLibrary,
    pub set_folder_role: SetFolderRole,
    pub folder_rules: FolderRules,
    pub collection_presets: CollectionPresets,
    pub browse_folder: BrowseFolder,
    pub get_folder_children: GetFolderChildren,
    pub get_model: GetModel,
    pub search_models: Arc<SearchModels>,
    pub collections: ManageCollections,
    pub find_duplicates: FindDuplicates,
    pub slicers: Arc<ManageSlicers>,
    pub update_model_meta: Arc<UpdateModelMeta>,
    pub import_orynt3d: ImportOrynt3d,
    pub rebuild_from_sidecars: RebuildFromSidecars,
    pub list_tags: ListTags,
    pub open_in_slicer: OpenInSlicer,
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
        let editor = Arc::new(RuleEditor {
            libraries: index.clone(),
            entries: index.clone(),
            models: index.clone(),
            search: search.clone(),
            locks: locks.clone(),
        });
        let search_models = Arc::new(SearchModels { search: search.clone(), models: index.clone() });
        let update_model_meta = Arc::new(UpdateModelMeta {
            libraries: index.clone(),
            models: index.clone(),
            meta: index.clone(),
            sidecars: Arc::new(SidecarFileStore),
            search: search.clone(),
            locks: locks.clone(),
        });
        let slicers = Arc::new(ManageSlicers { store: index.clone(), detector: Arc::new(InstalledSlicers) });
        Ok(Self {
            index_dir: index_dir.clone(),
            libraries: index.clone(),
            update_library_settings: UpdateLibrarySettings {
                libraries: index.clone(),
                models: index.clone(),
                meta: index.clone(),
                sidecars: Arc::new(SidecarFileStore),
            },
            move_index_location: MoveIndexLocation {
                libraries: index.clone(),
                export: index.clone(),
                derived: Arc::new(SearchFolder),
                location: Arc::new(BootstrapFile),
                storage: Arc::new(SystemStorage),
                locks: locks.clone(),
            },
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
            set_folder_role: SetFolderRole { editor: editor.clone() },
            folder_rules: FolderRules { editor: editor.clone() },
            collection_presets: CollectionPresets { entries: index.clone(), editor },
            browse_folder: BrowseFolder { models: index.clone() },
            get_folder_children: GetFolderChildren { models: index.clone() },
            get_model: GetModel { models: index.clone() },
            open_in_slicer: OpenInSlicer {
                libraries: index.clone(),
                models: index.clone(),
                slicers: slicers.clone(),
                launcher: Arc::new(SystemLauncher::new(index_dir.join("temp"))),
            },
            slicers,
            import_orynt3d: ImportOrynt3d {
                libraries: index.clone(),
                models: index.clone(),
                meta: index.clone(),
                reader: Arc::new(Orynt3dFiles),
                update: update_model_meta.clone(),
            },
            update_model_meta,
            rebuild_from_sidecars: RebuildFromSidecars {
                libraries: index.clone(),
                models: index.clone(),
                meta: index.clone(),
                sidecars: Arc::new(SidecarFileStore),
                search: search.clone(),
                locks: locks.clone(),
            },
            list_tags: ListTags { meta: index.clone() },
            collections: ManageCollections {
                store: index.clone(),
                models: index.clone(),
                search: search_models.clone(),
            },
            find_duplicates: FindDuplicates {
                libraries: index.clone(),
                entries: index.clone(),
                models: index.clone(),
                hasher: Arc::new(Blake3Hasher),
            },
            search_models,
        })
    }
}
