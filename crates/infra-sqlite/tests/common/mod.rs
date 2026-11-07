//! Fixtures shared by the index tests.
#![allow(dead_code)] // each test file uses a different subset

use hoarddeck_application::ports::LibraryRepository;
use hoarddeck_application::scan::ScanRules;
use hoarddeck_domain::{Entry, EntryKind, FileFormat, FileKind, LibraryId, Model, ModelFile, ModelKind, RelPath};
use hoarddeck_infra_sqlite::SqliteIndex;

pub fn path(value: &str) -> RelPath {
    RelPath::new(value).expect("valid path")
}

pub fn open() -> (tempfile::TempDir, SqliteIndex, LibraryId) {
    let dir = tempfile::tempdir().expect("temp dir");
    let index = SqliteIndex::open(dir.path()).expect("open index");
    let library = index.add("3D Print", "/Volumes/volume/3D Print", &ScanRules::default()).expect("add library");
    (dir, index, library.id)
}

pub fn stl(value: &str) -> Entry {
    Entry { path: path(value), kind: EntryKind::File(FileKind::Mesh(FileFormat::Stl)), size: 10, mtime: 1 }
}

pub fn dir(value: &str) -> Entry {
    Entry { path: path(value), kind: EntryKind::Dir, size: 0, mtime: 1 }
}

pub fn model(model_path: &str, title: &str) -> Model {
    let model_path = path(model_path);
    Model {
        folder: model_path.parent().expect("not root"),
        files: vec![ModelFile {
            path: model_path.join("a.stl").expect("valid"),
            kind: FileKind::Mesh(FileFormat::Stl),
            size: 5,
            variant: None,
        }],
        path: model_path,
        kind: ModelKind::Folder,
        title: title.to_owned(),
        released: None,
        cover: None,
        collection: None,
        marked_by_user: false,
    }
}
