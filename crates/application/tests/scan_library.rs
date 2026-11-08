//! `ScanLibrary` with in-memory fakes: what happens when the walk cannot finish.

use std::sync::mpsc::SyncSender;
use std::sync::{Arc, Mutex};

use hoarddeck_application::browse::{BrowsePage, BrowseRequest};
use hoarddeck_application::ports::{
    EntryStore, EventSink, LibraryRepository, LibraryScanner, ModelRepository, SearchIndex, SizedFile,
};
use hoarddeck_application::scan::{
    CompiledRules, Generation, KnownDirs, LibraryLocks, ScanItem, ScanLibrary, ScanMode, ScanProgress, ScanRules,
};
use hoarddeck_application::search::{SearchDocument, SearchHits, SearchQuery};
use hoarddeck_application::{AppError, IndexError, ModelDetail, ModelSummary, ScanError, SearchError};
use hoarddeck_domain::{Catalog, Entry, Folder, Library, LibraryId, LibrarySettings, ModelId, RelPath};

#[derive(Default)]
struct Store {
    finished: Mutex<bool>,
    replaced: Mutex<bool>,
}

impl LibraryRepository for Store {
    fn add(&self, _: &str, _: &str, _: &ScanRules) -> Result<Library, IndexError> {
        unreachable!()
    }
    fn list(&self) -> Result<Vec<Library>, IndexError> {
        Ok(Vec::new())
    }
    fn get(&self, id: LibraryId) -> Result<Option<Library>, IndexError> {
        Ok(Some(Library {
            id,
            name: "NAS".to_owned(),
            root: "/Volumes/gone".to_owned(),
            settings: LibrarySettings::default(),
        }))
    }
    fn scan_rules(&self, _: LibraryId) -> Result<ScanRules, IndexError> {
        Ok(ScanRules::default())
    }
    fn save_settings(&self, _: LibraryId, _: LibrarySettings) -> Result<(), IndexError> {
        Ok(())
    }
    fn save_scan_rules(&self, _: LibraryId, _: &ScanRules) -> Result<(), IndexError> {
        Ok(())
    }
    fn last_scan_at(&self, _: LibraryId) -> Result<Option<i64>, IndexError> {
        Ok(None)
    }
}

impl EntryStore for Store {
    fn begin_scan(&self, _: LibraryId) -> Result<(Generation, KnownDirs), IndexError> {
        Ok((Generation(2), KnownDirs::new()))
    }
    fn apply(&self, _: LibraryId, _: Generation, _: &[ScanItem]) -> Result<(), IndexError> {
        Ok(())
    }
    fn finish_scan(&self, _: LibraryId, _: Generation) -> Result<u64, IndexError> {
        *self.finished.lock().expect("lock") = true;
        Ok(0)
    }
    fn finish_folder_scan(&self, library: LibraryId, generation: Generation, _: &RelPath) -> Result<u64, IndexError> {
        self.finish_scan(library, generation)
    }
    fn folders(&self, _: LibraryId) -> Result<Vec<RelPath>, IndexError> {
        Ok(Vec::new())
    }
    fn same_size_files(&self, _: u64) -> Result<Vec<Vec<SizedFile>>, IndexError> {
        Ok(Vec::new())
    }
    fn files(&self, _: LibraryId) -> Result<Vec<Entry>, IndexError> {
        Ok(Vec::new())
    }
}

impl ModelRepository for Store {
    fn replace_catalog(&self, _: LibraryId, _: &Catalog) -> Result<(), IndexError> {
        *self.replaced.lock().expect("lock") = true;
        Ok(())
    }
    fn search_documents(&self, _: LibraryId) -> Result<Vec<SearchDocument>, IndexError> {
        Ok(Vec::new())
    }
    fn search_document(&self, _: ModelId) -> Result<Option<SearchDocument>, IndexError> {
        Ok(None)
    }
    fn summaries(&self, _: &[ModelId]) -> Result<Vec<ModelSummary>, IndexError> {
        Ok(Vec::new())
    }
    fn get(&self, _: ModelId) -> Result<Option<ModelDetail>, IndexError> {
        Ok(None)
    }
    fn browse(&self, _: &BrowseRequest) -> Result<BrowsePage, IndexError> {
        Ok(BrowsePage { models: Vec::new(), next: None })
    }
    fn model_of_file(&self, _: LibraryId, _: &RelPath) -> Result<Option<ModelId>, IndexError> {
        Ok(None)
    }
    fn child_folders(&self, _: LibraryId, _: &RelPath) -> Result<Vec<Folder>, IndexError> {
        Ok(Vec::new())
    }
    fn loose_images(&self, _: LibraryId, _: &RelPath) -> Result<Vec<RelPath>, IndexError> {
        Ok(Vec::new())
    }
}

struct NoSearch;

impl SearchIndex for NoSearch {
    fn replace_library(&self, _: LibraryId, _: &[SearchDocument]) -> Result<(), SearchError> {
        Ok(())
    }
    fn update_documents(&self, _: &[SearchDocument]) -> Result<(), SearchError> {
        Ok(())
    }
    fn search(&self, _: &SearchQuery) -> Result<SearchHits, SearchError> {
        Ok(SearchHits::default())
    }
}

struct Silent;

impl EventSink for Silent {
    fn scan_progress(&self, _: &ScanProgress) {}
}

/// Sends `items`, then fails with `error` (if any).
struct ScriptedScanner {
    items: Vec<ScanItem>,
    error: fn() -> Option<ScanError>,
}

impl LibraryScanner for ScriptedScanner {
    fn scan(
        &self,
        _: &str,
        _: &RelPath,
        _: &KnownDirs,
        _: &CompiledRules,
        out: &SyncSender<ScanItem>,
    ) -> Result<(), ScanError> {
        for item in &self.items {
            out.send(item.clone()).map_err(|_| ScanError::Aborted("closed".to_owned()))?;
        }
        (self.error)().map_or(Ok(()), Err)
    }
}

fn scan_with(scanner: ScriptedScanner) -> (Arc<Store>, Result<(), AppError>) {
    let store = Arc::new(Store::default());
    let use_case = ScanLibrary::new(
        store.clone(),
        store.clone(),
        store.clone(),
        Arc::new(NoSearch),
        Arc::new(scanner),
        Arc::new(Silent),
        Arc::new(LibraryLocks::default()),
    );
    let result = use_case.execute(LibraryId(1), ScanMode::Incremental).map(|_| ());
    (store, result)
}

#[test]
fn an_offline_library_keeps_its_index() {
    let (store, result) = scan_with(ScriptedScanner {
        items: Vec::new(),
        error: || Some(ScanError::Offline("/Volumes/gone: not mounted".to_owned())),
    });
    assert!(matches!(result, Err(AppError::Scan(ScanError::Offline(_)))));
    assert!(!*store.finished.lock().expect("lock"), "no stale-entry cleanup after a failed walk");
    assert!(!*store.replaced.lock().expect("lock"), "models are not regrouped from a partial walk");
}

#[test]
fn an_aborted_walk_deletes_nothing_even_after_partial_results() {
    let partial = ScanItem::Listed { dir: RelPath::root(), mtime: 1, children: Vec::new() };
    let (store, result) = scan_with(ScriptedScanner {
        items: vec![partial],
        error: || Some(ScanError::Aborted("network dropped".to_owned())),
    });
    assert!(result.is_err());
    assert!(!*store.finished.lock().expect("lock"));
}

#[test]
fn a_complete_walk_cleans_up_and_regroups() {
    let (store, result) = scan_with(ScriptedScanner { items: Vec::new(), error: || None });
    assert!(result.is_ok());
    assert!(*store.finished.lock().expect("lock") && *store.replaced.lock().expect("lock"));
}
