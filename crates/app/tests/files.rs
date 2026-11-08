//! Renaming and moving on disk: only in Managed libraries, user data follows, undo restores.

use std::fs;
use std::sync::Arc;

use hoarddeck_app::App;
use hoarddeck_application::files::Target;
use hoarddeck_application::ports::EventSink;
use hoarddeck_application::ports::JournalStatus;
use hoarddeck_application::scan::{FolderRole, ScanMode, ScanProgress};
use hoarddeck_application::search::SearchRequest;
use hoarddeck_domain::{AccessMode, MetadataStorage, ModelMeta, RelPath, TagName};

struct Silent;

impl EventSink for Silent {
    fn scan_progress(&self, _: &ScanProgress) {}
}

#[test]
fn rename_and_move_carry_user_data_and_undo_puts_everything_back() {
    let library_dir = tempfile::tempdir().expect("library");
    let index_dir = tempfile::tempdir().expect("index");
    let root = library_dir.path();
    fs::create_dir_all(root.join("Minis/Dragon")).expect("dirs");
    fs::write(root.join("Minis/Dragon/dragon.stl"), vec![0_u8; 134]).expect("stl");
    let app = App::open(index_dir.path(), 2, Arc::new(Silent)).expect("open");
    let library = app.add_library.execute("Minis", root.to_str().expect("utf-8")).expect("add");
    app.scan_library.execute(library.id, ScanMode::Incremental).expect("scan");
    let find = |text: &str| {
        let request =
            SearchRequest { text: text.to_owned(), library: Some(library.id), scope: None, offset: 0, limit: 5 };
        app.search_models.execute(&request).expect("search").models
    };
    let dragon = find("dragon")[0].id;
    let tagged =
        ModelMeta { tags: [TagName::new("Resin").expect("valid")].into_iter().collect(), ..ModelMeta::default() };
    app.update_model_meta.execute(dragon, tagged).expect("tag");

    assert!(
        app.move_model.execute(dragon, &Target::Rename("Red Dragon".into())).is_err(),
        "read-only libraries refuse"
    );
    assert!(root.join("Minis/Dragon").exists());

    app.update_library_settings.execute(library.id, AccessMode::Managed, MetadataStorage::IndexOnly).expect("managed");
    let dragon_folder = RelPath::new("Minis/Dragon").expect("valid");
    app.set_folder_role.execute(library.id, &dragon_folder, Some(FolderRole::Model)).expect("mark");
    let dragon = find("dragon")[0].id;
    let change = app.move_model.execute(dragon, &Target::Rename("Red Dragon".into())).expect("rename");
    assert_eq!(change.warning, None);
    let journal = change.journal;
    let rules = app.libraries.scan_rules(library.id).expect("rules");
    assert_eq!(rules.folder_rules[0].pattern, "Minis/Red Dragon", "the folder mark followed the rename");
    assert!(root.join("Minis/Red Dragon/dragon.stl").exists() && !root.join("Minis/Dragon").exists());
    let renamed = &find("tag:resin")[0];
    assert_eq!(renamed.path, RelPath::new("Minis/Red Dragon").expect("valid"), "the tag moved with the folder");

    let moved_journal = app
        .move_model
        .execute(renamed.id, &Target::MoveTo(RelPath::new("Archive").expect("valid")))
        .expect("move")
        .journal;
    assert!(root.join("Archive/Red Dragon/dragon.stl").exists());
    let ignored = Target::MoveTo(RelPath::new("x/@eaDir").expect("valid"));
    assert!(app.move_model.execute(find("tag:resin")[0].id, &ignored).is_err(), "ignored folders are refused");
    fs::create_dir_all(root.join("Archive/Taken")).expect("taken");
    let current = find("tag:resin")[0].id;
    assert!(app.move_model.execute(current, &Target::Rename("Taken".into())).is_err(), "never overwrites");

    app.undo_file_operation.execute(moved_journal).expect("undo move");
    app.undo_file_operation.execute(journal).expect("undo rename");
    assert!(root.join("Minis/Dragon/dragon.stl").exists(), "back where it started");
    assert_eq!(find("tag:resin")[0].path, RelPath::new("Minis/Dragon").expect("valid"));
    assert!(app.undo_file_operation.execute(journal).is_err(), "undo only once");
    assert_eq!(app.libraries.scan_rules(library.id).expect("rules").folder_rules[0].pattern, "Minis/Dragon");
}

#[test]
fn a_move_interrupted_by_a_crash_is_settled_on_the_next_start() {
    let library_dir = tempfile::tempdir().expect("library");
    let index_dir = tempfile::tempdir().expect("index");
    let root = library_dir.path();
    fs::create_dir_all(root.join("Dragon")).expect("dirs");
    fs::write(root.join("Dragon/dragon.stl"), vec![0_u8; 134]).expect("stl");
    let (library, entry) = {
        let app = App::open(index_dir.path(), 2, Arc::new(Silent)).expect("open");
        let library = app.add_library.execute("Minis", root.to_str().expect("utf-8")).expect("add");
        let (from, to) = (RelPath::new("Dragon").expect("valid"), RelPath::new("Red Dragon").expect("valid"));
        let entry = app.file_journal.begin(library.id, &from, &to, hoarddeck_domain::ModelKind::Folder).expect("begin");
        // The files moved, then the app died before the index was updated.
        fs::rename(root.join("Dragon"), root.join("Red Dragon")).expect("rename");
        (library, entry)
    };
    let app = App::open(index_dir.path(), 2, Arc::new(Silent)).expect("reopen");
    assert_eq!(app.file_journal.entry(entry).expect("read").expect("exists").status, JournalStatus::Done);
    let _ = library;
}

#[test]
fn a_screenshot_lands_next_to_its_mesh_and_becomes_the_cover_without_a_scan() {
    let library_dir = tempfile::tempdir().expect("library");
    let index_dir = tempfile::tempdir().expect("index");
    let root = library_dir.path();
    fs::create_dir_all(root.join("Dragon")).expect("dirs");
    fs::write(root.join("Dragon/dragon.stl"), vec![0_u8; 134]).expect("stl");
    let app = App::open(index_dir.path(), 2, Arc::new(Silent)).expect("open");
    let library = app.add_library.execute("Minis", root.to_str().expect("utf-8")).expect("add");
    app.scan_library.execute(library.id, ScanMode::Incremental).expect("scan");
    let find = || {
        let request = SearchRequest { text: "dragon".to_owned(), library: None, scope: None, offset: 0, limit: 5 };
        app.search_models.execute(&request).expect("search").models.remove(0)
    };
    let stl = RelPath::new("Dragon/dragon.stl").expect("valid");
    let png = [&[0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1a, b'\n'][..], b"pixels"].concat();

    assert!(app.save_screenshot.execute(find().id, &stl, &png).is_err(), "read-only libraries refuse");
    assert!(!root.join("Dragon/preview_dragon.png").exists());

    app.update_library_settings
        .execute(library.id, AccessMode::MetadataWrite, MetadataStorage::IndexOnly)
        .expect("write");
    assert!(app.save_screenshot.execute(find().id, &stl, b"not a png").is_err(), "only PNG images");
    let other = RelPath::new("Dragon/other.stl").expect("valid");
    assert!(app.save_screenshot.execute(find().id, &other, &png).is_err(), "only files of the model");
    // Written by hand: an image the screenshot must not pull into the index, because nothing walks the library.
    fs::write(root.join("Dragon/cover.png"), b"by hand").expect("hand-made image");

    let shot = app.save_screenshot.execute(find().id, &stl, &png).expect("screenshot");
    assert_eq!(shot.path, RelPath::new("Dragon/preview_dragon.png").expect("valid"));
    assert_eq!(fs::read(root.join("Dragon/preview_dragon.png")).expect("read"), png);
    assert_eq!(find().cover, Some(shot.path), "the new image is the cover right away");
    let second = app.save_screenshot.execute(find().id, &stl, &png).expect("second screenshot");
    assert_eq!(second.path, RelPath::new("Dragon/preview_dragon 2.png").expect("valid"), "never overwrites");
    let detail = app.get_model.execute(find().id).expect("model").expect("exists");
    assert!(!detail.files.iter().any(|file| file.path.name() == "cover.png"), "only the screenshot was added");
}
