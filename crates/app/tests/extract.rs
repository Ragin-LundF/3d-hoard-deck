//! Extracting archives with the real adapters: staging folder, review, delete or discard, user data carried over.

use std::fs;
use std::io::Write;
use std::path::Path;
use std::sync::Arc;

use hoarddeck_app::App;
use hoarddeck_application::AppError;
use hoarddeck_application::files::ArchiveChoice;
use hoarddeck_application::ports::{Deleted, EventSink};
use hoarddeck_application::scan::{ScanMode, ScanProgress};
use hoarddeck_application::search::SearchRequest;
use hoarddeck_domain::{AccessMode, LibraryId, MetadataStorage, ModelId, RelPath, TagName};

fn rel(path: &str) -> RelPath {
    RelPath::new(path).expect("path")
}

struct Silent;

impl EventSink for Silent {
    fn scan_progress(&self, _: &ScanProgress) {}
}

fn stl() -> Vec<u8> {
    let mut stl = vec![0_u8; 134];
    stl[80] = 1;
    stl
}

fn write_zip(path: &Path, files: &[(&str, &[u8])]) {
    fs::create_dir_all(path.parent().expect("parent")).expect("dirs");
    let mut zip = zip::ZipWriter::new(fs::File::create(path).expect("create"));
    let options = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    for (name, bytes) in files {
        zip.start_file(*name, options).expect("start");
        zip.write_all(bytes).expect("write");
    }
    zip.finish().expect("finish");
}

fn open(library_dir: &Path, index_dir: &Path) -> (App, LibraryId) {
    let app = App::open(index_dir, 2, Arc::new(Silent)).expect("open");
    let library = app.add_library.execute("Downloads", library_dir.to_str().expect("utf-8")).expect("add");
    app.scan_library.execute(library.id, ScanMode::Incremental).expect("scan");
    (app, library.id)
}

fn find(app: &App, library: LibraryId, text: &str) -> Vec<(ModelId, String)> {
    let request = SearchRequest { text: text.to_owned(), library: Some(library), scope: None, offset: 0, limit: 50 };
    app.search_models
        .execute(&request)
        .expect("search")
        .models
        .into_iter()
        .map(|m| (m.id, m.path.to_string()))
        .collect()
}

#[test]
fn an_archive_is_extracted_reviewed_and_deleted_with_its_tags_moving_along() {
    let library_dir = tempfile::tempdir().expect("library");
    let index_dir = tempfile::tempdir().expect("index");
    let stl = stl();
    let pack = library_dir.path().join("Loot/Dragon Pack.zip");
    write_zip(
        &pack,
        &[
            ("Dragon Pack/dragon.stl", &stl),
            ("Dragon Pack/Supported/wing.stl", &stl),
            ("Dragon Pack/Renders/front.png", b"png"),
        ],
    );
    write_zip(&library_dir.path().join("Loot/Odd.zip"), &[("Read me?.txt", b"hi"), ("odd.stl", &stl)]);
    // A model folder beside the archives: they become models of their own, and the name "Odd" is taken.
    fs::create_dir_all(library_dir.path().join("Loot/Odd")).expect("dir");
    fs::write(library_dir.path().join("Loot/Odd/old.stl"), &stl).expect("taken name");
    let (app, library) = open(library_dir.path(), index_dir.path());
    let archive = find(&app, library, "dragon pack")[0].0;

    let refused = app.extract_archives.extract(archive, &rel("Loot/Dragon Pack.zip"), "Dragon Pack", None);
    assert!(matches!(refused, Err(AppError::Refused(_))), "only Managed libraries: {refused:?}");
    app.update_library_settings.execute(library, AccessMode::Managed, MetadataStorage::IndexOnly).expect("managed");

    let plan = app.extract_archives.plan(archive, &rel("Loot/Dragon Pack.zip"), None).expect("plan");
    assert_eq!(
        (plan.folder.as_str(), plan.files, plan.unwrapped.as_deref(), plan.problem),
        ("Dragon Pack", 3, Some("Dragon Pack"), None)
    );
    let mut meta = app.get_model.execute(archive).expect("read").expect("model").meta;
    meta.tags.insert(TagName::new("dragon").expect("tag"));
    app.update_model_meta.execute(archive, meta).expect("tag the archive");

    let done = app.extract_archives.extract(archive, &plan.archive, &plan.folder, None).expect("extract");
    assert_eq!(done.pictures, [rel("Loot/Dragon Pack/Renders/front.png")], "pictures for a quick look");
    let folder = library_dir.path().join("Loot/Dragon Pack");
    assert!(
        folder.join("dragon.stl").is_file() && folder.join("Supported/wing.stl").is_file(),
        "unwrapped into the new folder"
    );
    let leftovers: Vec<_> = fs::read_dir(library_dir.path().join("Loot"))
        .expect("list")
        .filter_map(Result::ok)
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.starts_with('.'))
        .collect();
    assert!(leftovers.is_empty(), "no staging folder left: {leftovers:?}");
    assert_eq!(
        app.extract_archives.pending(archive).expect("pending"),
        [(done.journal, plan.archive.clone(), done.folder.clone())]
    );
    assert!(pack.exists(), "the archive waits for the user's decision");

    let deleted = app.extract_archives.finish(done.journal, ArchiveChoice::Delete { permanent: true }).expect("delete");
    assert_eq!((deleted.deleted, deleted.warning), (Deleted::Removed, None));
    assert!(!pack.exists() && folder.is_dir());
    let tagged = find(&app, library, "tag:dragon");
    assert_eq!(tagged.iter().map(|(_, path)| path.as_str()).collect::<Vec<_>>(), ["Loot/Dragon Pack"], "the tag moved");
    assert!(
        app.extract_archives.finish(done.journal, ArchiveChoice::Discard { permanent: true }).is_err(),
        "settled once"
    );

    // A taken name gets "(2)", odd names are made safe, and discarding removes the folder again.
    let odd = find(&app, library, "odd").into_iter().find(|(_, path)| path == "Loot/Odd.zip").expect("odd.zip").0;
    let plan = app.extract_archives.plan(odd, &rel("Loot/Odd.zip"), None).expect("plan");
    assert!(plan.clash && plan.folder == "Odd (2)");
    let done = app.extract_archives.extract(odd, &plan.archive, &plan.folder, None).expect("extract");
    assert_eq!(done.renamed, [("Read me?.txt".to_owned(), "Read me_.txt".to_owned())]);
    assert!(library_dir.path().join("Loot/Odd (2)/Read me_.txt").is_file());
    app.extract_archives.finish(done.journal, ArchiveChoice::Discard { permanent: true }).expect("discard");
    assert!(!library_dir.path().join("Loot/Odd (2)").exists() && library_dir.path().join("Loot/Odd.zip").exists());
}

#[test]
fn a_damaged_or_changed_archive_leaves_nothing_behind_and_is_never_deleted() {
    let library_dir = tempfile::tempdir().expect("library");
    let index_dir = tempfile::tempdir().expect("index");
    let stl = stl();
    let broken = library_dir.path().join("Broken.zip");
    write_zip(&broken, &[("a.stl", &stl), ("b.stl", &stl)]);
    // Flip a byte inside the stored data of the first file: its checksum no longer matches.
    let mut bytes = fs::read(&broken).expect("read");
    let at =
        bytes.windows(4).position(|window| window == [0x50, 0x4b, 0x03, 0x04]).expect("local header") + 30 + 5 + 90;
    bytes[at] ^= 0xff;
    fs::write(&broken, bytes).expect("damage");
    write_zip(&library_dir.path().join("Good.zip"), &[("c.stl", &stl)]);
    let (app, library) = open(library_dir.path(), index_dir.path());
    app.update_library_settings.execute(library, AccessMode::Managed, MetadataStorage::IndexOnly).expect("managed");
    let id = |name: &str| find(&app, library, name).into_iter().find(|(_, path)| path == name).expect(name).0;

    assert!(
        app.extract_archives.extract(id("Broken.zip"), &rel("Broken.zip"), "Broken", None).is_err(),
        "checksum mismatch"
    );
    let names: Vec<String> = fs::read_dir(library_dir.path())
        .expect("list")
        .filter_map(Result::ok)
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    assert!(!names.iter().any(|name| name == "Broken" || name.starts_with(".hoarddeck-extract-")), "{names:?}");

    let done = app.extract_archives.extract(id("Good.zip"), &rel("Good.zip"), "Good", None).expect("extract");
    write_zip(&library_dir.path().join("Good.zip"), &[("c.stl", &stl), ("d.stl", &stl)]);
    let refused = app.extract_archives.finish(done.journal, ArchiveChoice::Delete { permanent: true });
    assert!(refused.is_err(), "the archive changed since it was extracted");
    assert!(library_dir.path().join("Good.zip").exists());
}

#[test]
fn archives_alone_in_a_folder_are_extracted_from_the_folder_model() {
    let library_dir = tempfile::tempdir().expect("library");
    let index_dir = tempfile::tempdir().expect("index");
    let stl = stl();
    write_zip(&library_dir.path().join("Release/Knight.zip"), &[("knight.stl", &stl)]);
    write_zip(&library_dir.path().join("Release/Orc.zip"), &[("orc.stl", &stl)]);
    let (app, library) = open(library_dir.path(), index_dir.path());
    app.update_library_settings.execute(library, AccessMode::Managed, MetadataStorage::IndexOnly).expect("managed");
    let release =
        find(&app, library, "release").into_iter().find(|(_, path)| path == "Release").expect("folder model").0;

    assert!(app.extract_archives.plan(release, &rel("Release/Nope.zip"), None).is_err(), "not a file of the model");
    let done = app.extract_archives.extract(release, &rel("Release/Knight.zip"), "Knight", None).expect("extract");
    assert!(library_dir.path().join("Release/Knight/knight.stl").is_file());
    app.extract_archives.finish(done.journal, ArchiveChoice::Delete { permanent: true }).expect("delete");
    assert!(!library_dir.path().join("Release/Knight.zip").exists());
    let knight = find(&app, library, "knight");
    assert!(knight.iter().any(|(_, path)| path == "Release/Knight"), "the new folder is a model: {knight:?}");
}
