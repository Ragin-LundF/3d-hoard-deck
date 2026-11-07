//! `MoveIndexLocation` with fakes: guards, existing indexes, busy libraries, unsafe folders.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use hoarddeck_application::library::{ExistingIndex, MoveIndexLocation};
use hoarddeck_application::ports::{
    DerivedIndexes, IndexExport, IndexLocationStore, LibraryRepository, StorageInspector, StorageKind,
};
use hoarddeck_application::scan::{LibraryLocks, ScanRules};
use hoarddeck_application::{AppError, IndexError};
use hoarddeck_domain::{Library, LibraryId, LibrarySettings};

const CURRENT: &str = "/Users/j/.3dhoarddeck";
const TARGET: &str = "/Users/j/Index";

#[derive(Default)]
struct Fakes {
    existing: Option<i64>,
    storage: Option<StorageKind>,
    exported: Mutex<Option<PathBuf>>,
    saved: Mutex<Option<PathBuf>>,
    invalidated: Mutex<Option<PathBuf>>,
}

impl LibraryRepository for Fakes {
    fn add(&self, _: &str, _: &str, _: &ScanRules) -> Result<Library, IndexError> {
        unreachable!()
    }
    fn list(&self) -> Result<Vec<Library>, IndexError> {
        let settings = LibrarySettings::default();
        Ok(vec![Library { id: LibraryId(1), name: "NAS".to_owned(), root: "/Volumes/nas".to_owned(), settings }])
    }
    fn get(&self, _: LibraryId) -> Result<Option<Library>, IndexError> {
        unreachable!()
    }
    fn scan_rules(&self, _: LibraryId) -> Result<ScanRules, IndexError> {
        unreachable!()
    }
    fn save_settings(&self, _: LibraryId, _: LibrarySettings) -> Result<(), IndexError> {
        unreachable!()
    }
    fn save_scan_rules(&self, _: LibraryId, _: &ScanRules) -> Result<(), IndexError> {
        unreachable!()
    }
}

impl IndexExport for Fakes {
    fn existing_index(&self, _: &Path) -> Option<i64> {
        self.existing
    }
    fn export_to(&self, dir: &Path) -> Result<(), IndexError> {
        *self.exported.lock().expect("lock") = Some(dir.to_path_buf());
        Ok(())
    }
}

impl DerivedIndexes for Fakes {
    fn invalidate(&self, dir: &Path) -> Result<(), IndexError> {
        *self.invalidated.lock().expect("lock") = Some(dir.to_path_buf());
        Ok(())
    }
}

impl IndexLocationStore for Fakes {
    fn save(&self, dir: &Path) -> Result<(), IndexError> {
        *self.saved.lock().expect("lock") = Some(dir.to_path_buf());
        Ok(())
    }
}

impl StorageInspector for Fakes {
    fn kind(&self, _: &Path) -> StorageKind {
        self.storage.unwrap_or(StorageKind::Local)
    }
}

fn run(fakes: &Arc<Fakes>, locks: &Arc<LibraryLocks>, target: &str, existing: ExistingIndex) -> Result<(), AppError> {
    let use_case = MoveIndexLocation {
        libraries: fakes.clone(),
        export: fakes.clone(),
        derived: fakes.clone(),
        location: fakes.clone(),
        storage: fakes.clone(),
        locks: locks.clone(),
    };
    use_case.execute(Path::new(CURRENT), Path::new(target), existing)
}

fn exported(fakes: &Fakes) -> Option<PathBuf> {
    fakes.exported.lock().expect("lock").clone()
}

fn saved(fakes: &Fakes) -> Option<PathBuf> {
    fakes.saved.lock().expect("lock").clone()
}

#[test]
fn the_index_is_copied_and_the_new_location_remembered() {
    let fakes = Arc::new(Fakes::default());
    run(&fakes, &Arc::default(), TARGET, ExistingIndex::Ask).expect("move");
    assert_eq!((exported(&fakes), saved(&fakes)), (Some(PathBuf::from(TARGET)), Some(PathBuf::from(TARGET))));
}

#[test]
fn an_existing_index_is_only_used_or_replaced_after_the_user_decides() {
    let fakes = Arc::new(Fakes { existing: Some(1_700_000_000), ..Fakes::default() });
    let locks = Arc::default();
    assert!(matches!(
        run(&fakes, &locks, TARGET, ExistingIndex::Ask),
        Err(AppError::IndexExists { modified: 1_700_000_000 })
    ));
    assert_eq!(saved(&fakes), None);
    run(&fakes, &locks, TARGET, ExistingIndex::Use).expect("use existing");
    assert_eq!((exported(&fakes), saved(&fakes)), (None, Some(PathBuf::from(TARGET))), "used as is, not overwritten");
    assert_eq!(*fakes.invalidated.lock().expect("lock"), None, "a used index keeps its search index");
    run(&fakes, &locks, TARGET, ExistingIndex::Replace).expect("replace");
    assert_eq!(exported(&fakes), Some(PathBuf::from(TARGET)));
    assert_eq!(*fakes.invalidated.lock().expect("lock"), Some(PathBuf::from(TARGET)), "stale search index discarded");
}

#[test]
fn unsafe_folders_libraries_and_busy_libraries_are_refused() {
    let locks: Arc<LibraryLocks> = Arc::default();
    let network = Arc::new(Fakes { storage: Some(StorageKind::Network), ..Fakes::default() });
    assert!(matches!(run(&network, &locks, TARGET, ExistingIndex::Ask), Err(AppError::UnsafeIndexLocation(_))));
    let cloud = Arc::new(Fakes { storage: Some(StorageKind::CloudSync), ..Fakes::default() });
    assert!(matches!(run(&cloud, &locks, TARGET, ExistingIndex::Ask), Err(AppError::UnsafeIndexLocation(_))));

    let fakes = Arc::new(Fakes::default());
    assert!(matches!(run(&fakes, &locks, "/Volumes/nas/index", ExistingIndex::Ask), Err(AppError::InvalidInput(_))));
    assert!(matches!(run(&fakes, &locks, CURRENT, ExistingIndex::Ask), Err(AppError::InvalidInput(_))));
    let scanning = locks.lock(LibraryId(1)).expect("a scan holds the library");
    assert!(matches!(run(&fakes, &locks, TARGET, ExistingIndex::Ask), Err(AppError::LibraryBusy(_))));
    drop(scanning);
    assert_eq!(saved(&fakes), None, "nothing saved after a refusal");
}
