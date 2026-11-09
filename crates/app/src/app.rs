use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex, OnceLock};

use crate::after_scan::AfterScan;

use hoarddeck_application::AppError;
use hoarddeck_application::ai::SuggestTags;
use hoarddeck_application::browse::{BrowseFolder, GetFolderChildren, GetModel, ListArchive};
use hoarddeck_application::files::{
    ExtractArchives, MoveModel, ReconcileJournal, RenderedPreviews, SaveScreenshot, UndoFileOperation,
};
use hoarddeck_application::library::{AddLibrary, MoveIndexLocation, UpdateLibrarySettings};
use hoarddeck_application::organize::{
    FindDuplicates, ImportOrynt3d, ManageCollections, ManageTags, RebuildFromSidecars, SetFolderCover, UpdateModelMeta,
};
use hoarddeck_application::ports::{EventSink, FileJournal, LibraryRepository};
use hoarddeck_application::scan::{
    AutoRescan, CollectionPresets, FolderRules, LibraryLocks, RuleEditor, ScanLibrary, SetFolderRole,
};
use hoarddeck_application::search::{RebuildSearchIndex, SearchModels};
use hoarddeck_application::slicer::{ManageSlicers, OpenInSlicer};
use hoarddeck_application::tagging::{ManageTagRules, TagRuleBook};
use hoarddeck_infra_ai::{HttpTagSuggester, Keychain};
use hoarddeck_infra_archive::Archives;
use hoarddeck_infra_fs::{
    Blake3Hasher, BootstrapFile, FsFileOperations, FsLibraryScanner, IoPriority, NotifyWatcher, Orynt3dFiles,
    SidecarFileStore, SystemStorage,
};
use hoarddeck_infra_mesh::ZipArchives;
use hoarddeck_infra_platform::{InstalledSlicers, SystemLauncher};
use hoarddeck_infra_sqlite::SqliteIndex;
use hoarddeck_infra_tantivy::{SearchFolder, TantivySearchIndex};
use hoarddeck_infra_thumbs::ThumbnailCache;

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
    pub extract_archives: Arc<ExtractArchives>,
    pub save_screenshot: Arc<SaveScreenshot>,
    /// Previews rendered in the app for models without any picture.
    pub rendered: RenderedPreviews,
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
    pub tags: Arc<ManageTags>,
    pub tag_rules: ManageTagRules,
    /// Optional tag suggestions from an AI; everything is off until the user picks a provider.
    pub suggest_tags: Arc<SuggestTags>,
    /// Small JPEGs of images, for the UI and for the AI.
    pub thumbnails: Arc<ThumbnailCache>,
    pub open_in_slicer: OpenInSlicer,
}

impl App {
    /// Opens (or creates) the index in `index_dir`; rebuilds the search index from SQLite when it was reset.
    ///
    /// # Errors
    /// Fails when the index or search index cannot be opened.
    pub fn open(index_dir: &Path, scan_threads: usize, events: Arc<dyn EventSink>) -> Result<Self, AppError> {
        let after_scan = Arc::new(AfterScan { inner: events, suggest: OnceLock::new() });
        let events: Arc<dyn EventSink> = after_scan.clone();
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
        let tag_rules = Arc::new(TagRuleBook::new(index.clone()));
        let scan_library = Arc::new(ScanLibrary::new(
            index.clone(),
            index.clone(),
            index.clone(),
            search.clone(),
            Arc::new(FsLibraryScanner::new(scan_threads, io_priority.clone())),
            events.clone(),
            locks.clone(),
            tag_rules.clone(),
        ));
        let editor = rule_editor(&index, &search, &locks, &tag_rules, events.clone());
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
        let tags = Arc::new(manage_tags(&index, &search, &search_models, &locks));
        let thumbnails = Arc::new(ThumbnailCache::new(index_dir.join("thumbnails")));
        let suggest_tags = suggest_tags(&index, &tags, &thumbnails, &update_model_meta, events.clone());
        let rendered = rendered_previews(&index, &thumbnails, &save_screenshot);
        let _ = after_scan.suggest.set(Arc::downgrade(&suggest_tags));
        Ok(Self {
            io_priority: io_priority.clone(),
            index_dir: index_dir.clone(),
            libraries: index.clone(),
            update_library_settings: UpdateLibrarySettings {
                libraries: index.clone(),
                models: index.clone(),
                meta: index.clone(),
                sidecars: Arc::new(SidecarFileStore),
            },
            move_index_location: move_index_location(&index, &locks),
            add_library: AddLibrary { libraries: index.clone(), index_dir: index_dir.clone() },
            move_model,
            undo_file_operation,
            extract_archives: extract_archives(&index, &locks, &scan_library, &search, events.clone()),
            save_screenshot,
            rendered,
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
            import_orynt3d: import_orynt3d(&index, &update_model_meta),
            update_model_meta,
            rebuild_from_sidecars: rebuild_from_sidecars(&index, &search, &locks, &editor),
            set_folder_cover,
            tags,
            suggest_tags,
            thumbnails,
            tag_rules: ManageTagRules {
                book: tag_rules,
                libraries: index.clone(),
                entries: index.clone(),
                editor: editor.clone(),
            },
            collections: ManageCollections {
                store: index.clone(),
                models: index.clone(),
                search: search_models.clone(),
            },
            find_duplicates: find_duplicates(&index, &io_priority, events),
            search_models,
        })
    }
}

/// Rename, move, undo, screenshots and copied folder covers write into libraries; they share locks and rescans. Settles changes a crash interrupted.
fn file_operations(
    index: &Arc<SqliteIndex>,
    locks: &Arc<LibraryLocks>,
    scan: &Arc<ScanLibrary>,
) -> (MoveModel, UndoFileOperation, Arc<SaveScreenshot>, SetFolderCover) {
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
    let screenshot = Arc::new(SaveScreenshot {
        libraries: index.clone(),
        models: index.clone(),
        files: Arc::new(FsFileOperations),
        locks: locks.clone(),
        scan: scan.clone(),
    });
    let folder_cover = SetFolderCover {
        libraries: index.clone(),
        covers: index.clone(),
        files: Arc::new(FsFileOperations),
        scan: scan.clone(),
    };
    (move_model, undo, screenshot, folder_cover)
}

fn find_duplicates(
    index: &Arc<SqliteIndex>,
    io_priority: &Arc<IoPriority>,
    events: Arc<dyn EventSink>,
) -> FindDuplicates {
    FindDuplicates {
        libraries: index.clone(),
        entries: index.clone(),
        models: index.clone(),
        hasher: Arc::new(Blake3Hasher(io_priority.clone())),
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
    tag_rules: &Arc<TagRuleBook>,
    events: Arc<dyn EventSink>,
) -> Arc<RuleEditor> {
    Arc::new(RuleEditor {
        libraries: index.clone(),
        entries: index.clone(),
        models: index.clone(),
        search: search.clone(),
        locks: locks.clone(),
        sidecars: Arc::new(SidecarFileStore),
        tag_rules: tag_rules.clone(),
        events,
        queue: Mutex::default(),
    })
}

fn manage_tags(
    index: &Arc<SqliteIndex>,
    search: &Arc<TantivySearchIndex>,
    search_models: &Arc<SearchModels>,
    locks: &Arc<LibraryLocks>,
) -> ManageTags {
    ManageTags {
        libraries: index.clone(),
        models: index.clone(),
        meta: index.clone(),
        tags: index.clone(),
        sidecars: Arc::new(SidecarFileStore),
        search: search.clone(),
        search_models: search_models.clone(),
        locks: locks.clone(),
        undo: Mutex::default(),
    }
}

fn move_index_location(index: &Arc<SqliteIndex>, locks: &Arc<LibraryLocks>) -> MoveIndexLocation {
    MoveIndexLocation {
        libraries: index.clone(),
        export: index.clone(),
        derived: Arc::new(SearchFolder),
        location: Arc::new(BootstrapFile),
        storage: Arc::new(SystemStorage),
        locks: locks.clone(),
    }
}

fn extract_archives(
    index: &Arc<SqliteIndex>,
    locks: &Arc<LibraryLocks>,
    scan: &Arc<ScanLibrary>,
    search: &Arc<TantivySearchIndex>,
    events: Arc<dyn EventSink>,
) -> Arc<ExtractArchives> {
    Arc::new(ExtractArchives {
        libraries: index.clone(),
        models: index.clone(),
        files: Arc::new(FsFileOperations),
        extractor: Arc::new(Archives),
        journal: index.clone(),
        locks: locks.clone(),
        scan: scan.clone(),
        storage: Arc::new(SystemStorage),
        search: search.clone(),
        events,
        cancel: AtomicBool::new(false),
    })
}

fn rendered_previews(
    index: &Arc<SqliteIndex>,
    thumbnails: &Arc<ThumbnailCache>,
    screenshot: &Arc<SaveScreenshot>,
) -> RenderedPreviews {
    RenderedPreviews {
        libraries: index.clone(),
        models: index.clone(),
        store: index.clone(),
        cache: thumbnails.clone(),
        screenshot: screenshot.clone(),
    }
}

fn suggest_tags(
    index: &Arc<SqliteIndex>,
    tags: &Arc<ManageTags>,
    thumbnails: &Arc<ThumbnailCache>,
    update: &Arc<UpdateModelMeta>,
    events: Arc<dyn EventSink>,
) -> Arc<SuggestTags> {
    Arc::new(SuggestTags {
        libraries: index.clone(),
        models: index.clone(),
        meta: index.clone(),
        store: index.clone(),
        secrets: Arc::new(Keychain),
        suggester: Arc::new(HttpTagSuggester),
        previews: thumbnails.clone(),
        tags: tags.clone(),
        update: update.clone(),
        events,
        running: AtomicBool::default(),
        cancelled: AtomicBool::default(),
    })
}

fn rebuild_from_sidecars(
    index: &Arc<SqliteIndex>,
    search: &Arc<TantivySearchIndex>,
    locks: &Arc<LibraryLocks>,
    editor: &Arc<RuleEditor>,
) -> RebuildFromSidecars {
    RebuildFromSidecars {
        libraries: index.clone(),
        models: index.clone(),
        meta: index.clone(),
        sidecars: Arc::new(SidecarFileStore),
        search: search.clone(),
        locks: locks.clone(),
        editor: editor.clone(),
    }
}

fn import_orynt3d(index: &Arc<SqliteIndex>, update: &Arc<UpdateModelMeta>) -> ImportOrynt3d {
    ImportOrynt3d {
        libraries: index.clone(),
        models: index.clone(),
        meta: index.clone(),
        reader: Arc::new(Orynt3dFiles),
        update: update.clone(),
    }
}
