//! `.3dhoarddeck` files hold everything the user decided: export, lose the index, restore, and nothing is missing.

use std::fs;
use std::path::Path;
use std::sync::Arc;

use hoarddeck_app::App;
use hoarddeck_application::ports::{EventSink, SavedEntry, SavedLibraryData};
use hoarddeck_application::scan::{FolderRole, FolderRule, ScanMode, ScanProgress};
use hoarddeck_application::search::SearchRequest;
use hoarddeck_application::sidecar::{RestoreMode, RestoreSource};
use hoarddeck_application::tagging::{TagRule, tag_rule_presets};
use hoarddeck_domain::{AccessMode, LibraryId, MetadataStorage, ModelId, ModelMeta, RelPath, SmartSearch, TagName};

struct Silent;

impl EventSink for Silent {
    fn scan_progress(&self, _: &ScanProgress) {}
}

fn write(root: &Path, relative: &str) {
    let path = root.join(relative);
    fs::create_dir_all(path.parent().expect("parent")).expect("dirs");
    let mut stl = vec![0_u8; 134];
    stl[80] = 1;
    fs::write(path, stl).expect("write");
}

fn rel(path: &str) -> RelPath {
    RelPath::new(path).expect("path")
}

fn tag(name: &str) -> TagName {
    TagName::new(name).expect("tag")
}

/// Everything a user can decide about a library, in a comparable form.
#[derive(Debug, PartialEq)]
struct Snapshot {
    folder_rules: Vec<(String, FolderRole)>,
    ignore: Vec<String>,
    variants: Vec<String>,
    parent_covers: bool,
    entries: Vec<SavedEntry>,
    library: SavedLibraryData,
    tag_rules: Vec<TagRule>,
}

fn snapshot(app: &App, library: LibraryId) -> Snapshot {
    let rules = app.libraries.scan_rules(library).expect("rules");
    let mut folder_rules: Vec<(String, FolderRole)> =
        rules.folder_rules.into_iter().map(|rule| (rule.pattern, rule.role)).collect();
    folder_rules.sort_by(|a, b| a.0.cmp(&b.0));
    let store = &app.restore_from_sidecars.store;
    let mut entries: Vec<SavedEntry> = store
        .saved_entries(library, None)
        .expect("entries")
        .into_iter()
        .filter(|entry| {
            !entry.meta.is_empty() || entry.folder_cover.is_some() || entry.rendered.as_deref() == Some("skipped")
        })
        .collect();
    entries.sort_by(|a, b| a.path.cmp(&b.path));
    let mut data = store.saved_library(library).expect("library data");
    data.collections.sort_by(|a, b| a.name.cmp(&b.name));
    data.tag_groups.sort_by(|a, b| a.name.cmp(&b.name));
    let mut ignore = rules.ignore;
    ignore.sort();
    let mut variants = rules.variant_folders;
    variants.sort();
    Snapshot {
        folder_rules,
        ignore,
        variants,
        parent_covers: rules.parent_covers,
        entries,
        library: data,
        tag_rules: app.tag_rules.rules().expect("tag rules"),
    }
}

fn find(app: &App, library: LibraryId, path: &str) -> ModelId {
    let request = SearchRequest { text: String::new(), library: Some(library), scope: None, offset: 0, limit: 0 };
    app.search_models
        .all(&request, 1000)
        .expect("search")
        .models
        .into_iter()
        .find(|model| model.path.as_str() == path)
        .expect(path)
        .id
}

fn open(index: &Path, root: &Path) -> (App, LibraryId) {
    let app = App::open(index, 2, Arc::new(Silent)).expect("open");
    let library = app.add_library.execute("Print", root.to_str().expect("utf-8")).expect("add").id;
    app.scan_library.execute(library, ScanMode::Incremental).expect("scan");
    (app, library)
}

/// A library with every kind of user data the plan lists.
fn library_with_everything(root: &Path, index: &Path, writes_files: bool) -> (App, LibraryId) {
    write(root, "Minis/Dragon/dragon.stl");
    fs::write(root.join("Minis/Dragon/front.jpg"), b"jpg").expect("picture");
    write(root, "Minis/Orc/orc.stl");
    write(root, "Makerworld/Box/box.stl");
    write(root, "Makerworld/gear.stl");
    write(root, "Packs/Pack A/part1/a.stl");
    write(root, "Packs/Pack A/part2/b.stl");
    write(root, "Loot/2024-01/Knight/knight.stl");
    let (app, library) = open(index, root);
    if writes_files {
        app.update_library_settings
            .execute(library, AccessMode::MetadataWrite, MetadataStorage::IndexAndSidecar)
            .expect("files on");
    }
    let rules = vec![TagRule { tag: "32mm".to_owned(), ..tag_rule_presets().remove(0) }];
    app.tag_rules.save(&rules).expect("tag rules");
    app.libraries
        .update_scan_rules(library, &mut |rules| {
            rules.ignore.push("**/*.bak".to_owned());
            rules.variant_folders.push("Prints".to_owned());
            rules.folder_rules.push(FolderRule { pattern: "**/Loot/*".to_owned(), role: FolderRole::Collection });
            true
        })
        .expect("scan rules");
    app.folder_rules.set_parent_covers(library, false).expect("parent covers");
    app.set_folder_role.execute(library, &rel("Packs/Pack A"), Some(FolderRole::Model)).expect("mark");
    let dragon = find(&app, library, "Minis/Dragon");
    let meta = ModelMeta {
        display_name: Some("Red Dragon".to_owned()),
        tags: [tag("Resin")].into(),
        favorite: true,
        notes: "Print at 0.05 mm".to_owned(),
        cover: Some(rel("Minis/Dragon/front.jpg")),
        hidden_tags: [tag("32mm")].into(),
        printed: false,
    };
    app.update_model_meta.execute(dragon, meta).expect("dragon");
    let gear = find(&app, library, "Makerworld/gear.stl");
    app.update_model_meta
        .execute(gear, ModelMeta { tags: [tag("Functional")].into(), ..ModelMeta::default() })
        .expect("gear");
    let orc = find(&app, library, "Minis/Orc");
    app.update_model_meta.execute(orc, ModelMeta { tags: [tag("32mm")].into(), ..ModelMeta::default() }).expect("orc");
    app.rendered.skip(&[orc]).expect("skip preview");
    app.set_folder_cover
        .execute(library, &rel("Minis/Orc"), Some(&rel("Minis/Dragon/front.jpg")))
        .expect("folder cover");
    let group = app.tags.save_group(library, None, "Scale", "#e8833a").expect("group");
    app.tags.set_group(library, &[tag("32mm")], Some(group)).expect("in group");
    app.tags.set_pinned(library, &[tag("Resin")], true).expect("pin");
    let next = app.collections.create("Print next", None).expect("collection");
    app.collections.add(next.id, &[gear, dragon]).expect("members");
    app.collections
        .create("Resin here", Some(SmartSearch { text: "tag:resin".to_owned(), library: Some(library), scope: None }))
        .expect("smart");
    (app, library)
}

#[test]
fn everything_comes_back_from_the_files_after_the_index_is_lost() {
    let library_dir = tempfile::tempdir().expect("library");
    let root = library_dir.path();
    let before = {
        let index = tempfile::tempdir().expect("index");
        let (app, library) = library_with_everything(root, index.path(), true);
        let report = app.sidecar_sync.export(library).expect("export");
        assert!(report.failed.is_empty(), "{report:?}");
        let mtime = fs::metadata(root.join("Minis/Dragon")).expect("folder").modified().expect("mtime");
        let again = app.sidecar_sync.export(library).expect("export again");
        assert_eq!(again.written, 0, "nothing changed: no file is written, {again:?}");
        assert_eq!(fs::metadata(root.join("Minis/Dragon")).expect("folder").modified().expect("mtime"), mtime);
        snapshot(&app, library)
    };
    assert!(before.entries.len() == 3 && before.library.collections.len() == 2, "{before:#?}");

    // A new computer: a fresh index over the same folders, the files are all there is.
    let index = tempfile::tempdir().expect("new index");
    let (app, library) = open(index.path(), root);
    let report =
        app.restore_from_sidecars.execute(library, &RestoreSource::Files, RestoreMode::NewerWins).expect("restore");
    assert!(report.unreadable.is_empty() && report.dropped_members == 0, "{report:?}");
    assert_eq!(report.tag_rules_restored, 1, "the app had no tag rules yet");
    assert_eq!(snapshot(&app, library), before);
}

#[test]
fn a_backup_file_brings_back_a_read_only_library() {
    let library_dir = tempfile::tempdir().expect("library");
    let root = library_dir.path();
    let backup_dir = tempfile::tempdir().expect("backup");
    let backup = backup_dir.path().join("Print.3dhoarddeck-backup.json");
    let before = {
        let index = tempfile::tempdir().expect("index");
        let (app, library) = library_with_everything(root, index.path(), false);
        assert!(!root.join("Minis/Dragon/.3dhoarddeck").exists(), "a read-only library gets no files");
        assert!(app.sidecar_sync.export(library).is_err(), "no metadata files without write access");
        assert!(app.sidecar_sync.backup(library, &root.join("Minis/backup.json")).is_err(), "never into a library");
        assert!(!root.join("Minis/backup.json").exists());
        // The three models with data and the folder marked as one model.
        assert_eq!(app.sidecar_sync.backup(library, &backup).expect("backup"), 4);
        snapshot(&app, library)
    };
    let index = tempfile::tempdir().expect("new index");
    let (app, library) = open(index.path(), root);
    app.restore_from_sidecars
        .execute(library, &RestoreSource::Backup(backup), RestoreMode::NewerWins)
        .expect("restore");
    assert_eq!(snapshot(&app, library), before);
}

#[test]
fn a_folder_renamed_outside_the_app_keeps_its_data_mark_and_cover() {
    let library_dir = tempfile::tempdir().expect("library");
    let root = library_dir.path();
    {
        let index = tempfile::tempdir().expect("index");
        let (app, library) = library_with_everything(root, index.path(), true);
        app.sidecar_sync.export(library).expect("export");
    }
    fs::rename(root.join("Minis/Dragon"), root.join("Minis/Red Dragon")).expect("rename dragon");
    fs::rename(root.join("Packs/Pack A"), root.join("Packs/Pack B")).expect("rename pack");
    let index = tempfile::tempdir().expect("new index");
    let (app, library) = open(index.path(), root);
    app.update_library_settings
        .execute(library, AccessMode::MetadataWrite, MetadataStorage::IndexOnly)
        .expect("write access");
    app.restore_from_sidecars.execute(library, &RestoreSource::Files, RestoreMode::NewerWins).expect("restore");
    let settings = app.libraries.get(library).expect("read").expect("library").settings;
    assert_eq!(settings.metadata(), MetadataStorage::IndexAndSidecar, "where tags are kept comes back");
    let dragon = app.get_model.execute(find(&app, library, "Minis/Red Dragon")).expect("read").expect("model");
    assert_eq!(dragon.meta.display_name.as_deref(), Some("Red Dragon"));
    assert_eq!(dragon.meta.cover, Some(rel("Minis/Red Dragon/front.jpg")), "the cover follows the folder");
    find(&app, library, "Packs/Pack B");
    let rules = app.folder_rules.list(library).expect("rules");
    assert!(rules.contains(&FolderRule::for_folder(&rel("Packs/Pack B"), FolderRole::Model)), "{rules:?}");
}

#[test]
fn newer_wins_keeps_a_newer_index_value_and_files_win_overwrites_it() {
    let library_dir = tempfile::tempdir().expect("library");
    let root = library_dir.path();
    let index = tempfile::tempdir().expect("index");
    let (app, library) = library_with_everything(root, index.path(), true);
    // A file written long ago; the index value is newer.
    let file = root.join("Makerworld/gear.stl.3dhoarddeck");
    let mut json: serde_json::Value = serde_json::from_slice(&fs::read(&file).expect("read")).expect("json");
    json["tags"] = serde_json::json!(["Old"]);
    json["updatedAt"] = serde_json::json!(1);
    fs::write(&file, serde_json::to_vec(&json).expect("json")).expect("write");
    app.scan_library.execute(library, ScanMode::Incremental).expect("scan");
    let gear = find(&app, library, "Makerworld/gear.stl");
    let tags = |app: &App| app.get_model.execute(gear).expect("read").expect("model").meta.tags;
    app.restore_from_sidecars.execute(library, &RestoreSource::Files, RestoreMode::NewerWins).expect("newer wins");
    assert_eq!(tags(&app), [tag("Functional")].into());
    app.restore_from_sidecars.execute(library, &RestoreSource::Files, RestoreMode::FilesWin).expect("files win");
    assert_eq!(tags(&app), [tag("Old")].into());

    // Tags removed on another computer: a newer file without them clears them here too.
    json["tags"] = serde_json::json!([]);
    json["updatedAt"] = serde_json::json!(4_000_000_000_i64);
    fs::write(&file, serde_json::to_vec(&json).expect("json")).expect("write");
    app.restore_from_sidecars.execute(library, &RestoreSource::Files, RestoreMode::NewerWins).expect("newer file");
    assert!(tags(&app).is_empty(), "the cleared tags stay cleared");
}

#[cfg(unix)]
#[test]
fn a_file_that_cannot_be_written_waits_without_blocking_the_change() {
    use std::os::unix::fs::PermissionsExt;

    let library_dir = tempfile::tempdir().expect("library");
    let root = library_dir.path();
    write(root, "Minis/Orc/orc.stl");
    write(root, "Minis/Elf/elf.stl");
    let index = tempfile::tempdir().expect("index");
    let (app, library) = open(index.path(), root);
    app.update_library_settings
        .execute(library, AccessMode::MetadataWrite, MetadataStorage::IndexAndSidecar)
        .expect("files on");
    let orc = find(&app, library, "Minis/Orc");
    fs::set_permissions(root.join("Minis/Orc"), fs::Permissions::from_mode(0o555)).expect("lock folder");
    app.update_model_meta
        .execute(orc, ModelMeta { favorite: true, ..ModelMeta::default() })
        .expect("the change stands");
    assert!(app.get_model.execute(orc).expect("read").expect("model").meta.favorite);
    assert_eq!(app.sidecar_sync.status(library).expect("status").pending, 1);
    fs::set_permissions(root.join("Minis/Orc"), fs::Permissions::from_mode(0o755)).expect("unlock folder");
    let elf = find(&app, library, "Minis/Elf");
    app.update_model_meta.execute(elf, ModelMeta { printed: true, ..ModelMeta::default() }).expect("next change");
    assert_eq!(app.sidecar_sync.status(library).expect("status").pending, 0, "the next change writes the waiting file");
    assert!(root.join("Minis/Orc/.3dhoarddeck").exists());
}

#[test]
fn a_collection_across_two_libraries_merges_back_into_one() {
    let first_dir = tempfile::tempdir().expect("first");
    let second_dir = tempfile::tempdir().expect("second");
    write(first_dir.path(), "Minis/Orc/orc.stl");
    write(second_dir.path(), "Terrain/Wall/wall.stl");
    let open_both = |index: &Path| {
        let app = App::open(index, 2, Arc::new(Silent)).expect("open");
        let first = app.add_library.execute("First", first_dir.path().to_str().expect("utf-8")).expect("add").id;
        let second = app.add_library.execute("Second", second_dir.path().to_str().expect("utf-8")).expect("add").id;
        for library in [first, second] {
            app.scan_library.execute(library, ScanMode::Incremental).expect("scan");
        }
        (app, first, second)
    };
    {
        let index = tempfile::tempdir().expect("index");
        let (app, first, second) = open_both(index.path());
        for library in [first, second] {
            app.update_library_settings
                .execute(library, AccessMode::MetadataWrite, MetadataStorage::IndexAndSidecar)
                .expect("files on");
        }
        let both = app.collections.create("Weekend", None).expect("collection");
        app.collections
            .add(both.id, &[find(&app, first, "Minis/Orc"), find(&app, second, "Terrain/Wall")])
            .expect("members");
    }
    let index = tempfile::tempdir().expect("new index");
    let (app, first, second) = open_both(index.path());
    for library in [first, second] {
        app.restore_from_sidecars.execute(library, &RestoreSource::Files, RestoreMode::NewerWins).expect("restore");
    }
    let collections = app.collections.list().expect("list");
    assert_eq!(collections.len(), 1, "{collections:?}");
    assert_eq!(collections[0].1, Some(2), "both members, one collection");
}

#[test]
fn a_file_that_is_no_longer_its_own_model_gets_its_file_beside_it() {
    let library_dir = tempfile::tempdir().expect("library");
    let root = library_dir.path();
    write(root, "Loose/sword.stl");
    write(root, "Loose/Hands/hand.stl");
    write(root, "Gone/axe.stl");
    write(root, "Gone/Other/other.stl");
    let index = tempfile::tempdir().expect("index");
    let (app, library) = open(index.path(), root);
    for model in ["Loose/sword.stl", "Gone/axe.stl"] {
        let id = find(&app, library, model);
        app.update_model_meta
            .execute(id, ModelMeta { tags: [tag("Weapon")].into(), ..ModelMeta::default() })
            .expect("tag");
    }
    // The folder becomes one model: the sword is no model of its own anymore, but its tags stay in the index.
    app.set_folder_role.execute(library, &rel("Loose"), Some(FolderRole::Model)).expect("mark");
    // The axe is deleted outside the app.
    fs::remove_file(root.join("Gone/axe.stl")).expect("delete");
    app.scan_library.execute(library, ScanMode::Incremental).expect("scan");
    app.update_library_settings
        .execute(library, AccessMode::MetadataWrite, MetadataStorage::IndexAndSidecar)
        .expect("files on");
    let report = app.sidecar_sync.export(library).expect("export");
    assert!(report.failed.is_empty(), "no folder is made of a file: {report:?}");
    assert!(root.join("Loose/sword.stl.3dhoarddeck").is_file());
    assert!(root.join("Loose/sword.stl").is_file(), "the model file is untouched");
    assert!(!root.join("Gone/axe.stl.3dhoarddeck").exists(), "no file for a path that is gone");
}
