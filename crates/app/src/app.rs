use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use hoarddeck_application::AppError;
use hoarddeck_application::browse::{BrowseFolder, GetFolderChildren, GetModel, ListArchive};
use hoarddeck_application::files::{MoveModel, ReconcileJournal, SaveScreenshot, UndoFileOperation};
use hoarddeck_application::library::{AddLibrary, MoveIndexLocation, UpdateLibrarySettings};
use hoarddeck_application::organize::{
    FindDuplicates, ImportOrynt3d, ListTags, ManageCollections, RebuildFromSidecars, SetFolderCover, UpdateModelMeta,
};
use hoarddeck_application::ports::{EventSink, FileJournal, LibraryRepository};
use hoarddeck_application::scan::{
    AutoRescan, CollectionPresets, FolderRules, LibraryLocks, RuleEditor, ScanLibrary, SetFolderRole,
};
use hoarddeck_application::search::{RebuildSearchIndex, SearchModels};
use hoarddeck_application::slicer::{ManageSlicers, OpenInSlicer};
use hoarddeck_infra_fs::{
    Blake3Hasher, BootstrapFile, FsFileOperations, FsLibraryScanner, IoPriority, NotifyWatcher, Orynt3dFiles,
    SidecarFileStore, SystemStorage,
};
use hoarddeck_infra_mesh::ZipArchives;
use hoarddeck_infra_platform::{InstalledSlicers, SystemLauncher};
use hoarddeck_infra_sqlite::SqliteIndex;
use hoarddeck_infra_tantivy::{SearchFolder, TantivySearchIndex};

/// All adapters built once and injected into the use cases.
pub struct App {
    /// The folder this index was opened from (resolved).
    pub index_dir: PathBuf,
    /// Interactive reads (pictures, 3D files) take this while they run, so a scan steps back meanwhile.
    pub io_priority: Arc<IoPriority>,
    pub libraries: Arc<dyn LibraryRepository>,
    pub add_library: AddLibrary,
    pub update_library_settings: UpdateLibrarySettings,
    pub move_index_location: MoveIndexLocation,
    pub scan_library: Arc<ScanLibrary>,
    /// Rescans changed or due libraries; the shell calls `tick` from a background thread.
    pub auto_rescan: Arc<AutoRescan>,
    pub move_model: MoveModel,
    pub undo_file_operation: UndoFileOperation,
    pub save_screenshot: SaveScreenshot,
    /// Recent renames and moves, for the undo list.
    pub file_journal: Arc<dyn FileJournal>,
    pub set_folder_role: SetFolderRole,
    pub folder_rules: FolderRules,
    pub collection_presets: CollectionPresets,
    pub browse_folder: BrowseFolder,
    pub get_folder_children: GetFolderChildren,
    pub get_model: GetModel,
    pub list_archive: ListArchive,
    pub search_models: Arc<SearchModels>,
    pub collections: ManageCollections,
    pub find_duplicates: FindDuplicates,
    pub slicers: Arc<ManageSlicers>,
    pub update_model_meta: Arc<UpdateModelMeta>,
    pub import_orynt3d: ImportOrynt3d,
    pub rebuild_from_sidecars: RebuildFromSidecars,
    pub set_folder_cover: SetFolderCover,
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
        let io_priority = Arc::new(IoPriority::default());
        let scan_library = Arc::new(ScanLibrary::new(
            index.clone(),
            index.clone(),
            index.clone(),
            search.clone(),
            Arc::new(FsLibraryScanner::new(scan_threads, io_priority.clone())),
            events.clone(),
            locks.clone(),
        ));
        let editor = rule_editor(&index, &search, &locks, events.clone());
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
        let (move_model, undo_file_operation, save_screenshot, set_folder_cover) =
            file_operations(&index, &locks, &scan_library);
        Ok(Self {
            io_priority,
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
            move_model,
            undo_file_operation,
            save_screenshot,
            file_journal: index.clone(),
            auto_rescan: auto_rescan(&index, &scan_library),
            scan_library,
            set_folder_role: SetFolderRole { editor: editor.clone() },
            folder_rules: FolderRules { editor: editor.clone() },
            collection_presets: CollectionPresets { entries: index.clone(), editor: editor.clone() },
            browse_folder: BrowseFolder { models: index.clone() },
            get_folder_children: GetFolderChildren { models: index.clone() },
            list_archive: ListArchive { libraries: index.clone(), archives: Arc::new(ZipArchives) },
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
                editor: editor.clone(),
            },
            set_folder_cover,
            list_tags: ListTags { meta: index.clone() },
            collections: ManageCollections {
                store: index.clone(),
                models: index.clone(),
                search: search_models.clone(),
            },
            find_duplicates: find_duplicates(&index, events),
            search_models,
        })
    }
}

/// Rename, move, undo, screenshots and copied folder covers write into libraries; they share locks and rescans. Settles changes a crash interrupted.
fn file_operations(
    index: &Arc<SqliteIndex>,
    locks: &Arc<LibraryLocks>,
    scan: &Arc<ScanLibrary>,
) -> (MoveModel, UndoFileOperation, SaveScreenshot, SetFolderCover) {
    // An interrupted rename must not block opening; it is tried again on the next start.
    let reconcile =
        ReconcileJournal { libraries: index.clone(), files: Arc::new(FsFileOperations), journal: index.clone() };
    if let Err(error) = reconcile.execute() {
        eprintln!("unfinished file changes could not be settled: {error}");
    }
    let move_model = MoveModel {
        libraries: index.clone(),
        models: index.clone(),
        files: Arc::new(FsFileOperations),
        journal: index.clone(),
        locks: locks.clone(),
        scan: scan.clone(),
        sidecars: Arc::new(SidecarFileStore),
    };
    let undo = UndoFileOperation {
        libraries: index.clone(),
        files: Arc::new(FsFileOperations),
        journal: index.clone(),
        locks: locks.clone(),
        scan: scan.clone(),
        sidecars: Arc::new(SidecarFileStore),
    };
    let screenshot = SaveScreenshot {
        libraries: index.clone(),
        models: index.clone(),
        files: Arc::new(FsFileOperations),
        locks: locks.clone(),
        scan: scan.clone(),
    };
    let folder_cover = SetFolderCover {
        libraries: index.clone(),
        covers: index.clone(),
        files: Arc::new(FsFileOperations),
        scan: scan.clone(),
    };
    (move_model, undo, screenshot, folder_cover)
}

fn find_duplicates(index: &Arc<SqliteIndex>, events: Arc<dyn EventSink>) -> FindDuplicates {
    FindDuplicates {
        libraries: index.clone(),
        entries: index.clone(),
        models: index.clone(),
        hasher: Arc::new(Blake3Hasher),
        cache: index.clone(),
        events,
        cancelled: Arc::default(),
    }
}

fn auto_rescan(index: &Arc<SqliteIndex>, scan: &Arc<ScanLibrary>) -> Arc<AutoRescan> {
    Arc::new(AutoRescan::new(index.clone(), Arc::new(SystemStorage), Arc::new(NotifyWatcher::new()), scan.clone()))
}

fn rule_editor(
    index: &Arc<SqliteIndex>,
    search: &Arc<TantivySearchIndex>,
    locks: &Arc<LibraryLocks>,
    events: Arc<dyn EventSink>,
) -> Arc<RuleEditor> {
    Arc::new(RuleEditor {
        libraries: index.clone(),
        entries: index.clone(),
        models: index.clone(),
        search: search.clone(),
        locks: locks.clone(),
        sidecars: Arc::new(SidecarFileStore),
        events,
        queue: Mutex::default(),
    })
}
