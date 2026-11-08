//! Names, tags and marks with the real adapters: index only by default, `.3dhoarddeck` files only when allowed.

use std::fs;
use std::path::Path;
use std::sync::Arc;

use hoarddeck_app::App;
use hoarddeck_application::ports::EventSink;
use hoarddeck_application::scan::{ScanMode, ScanProgress};
use hoarddeck_application::search::SearchRequest;
use hoarddeck_domain::{AccessMode, MetadataStorage, ModelMeta, SmartSearch, TagName};

struct Silent;

impl EventSink for Silent {
    fn scan_progress(&self, _: &ScanProgress) {}
}

fn write_stl(root: &Path, relative: &str) {
    let path = root.join(relative);
    fs::create_dir_all(path.parent().expect("parent")).expect("dirs");
    let mut stl = vec![0_u8; 134];
    stl[80] = 1;
    fs::write(path, stl).expect("write");
}

#[test]
fn user_metadata_is_searchable_and_reaches_sidecars_only_when_allowed() {
    let library_dir = tempfile::tempdir().expect("library");
    let index_dir = tempfile::tempdir().expect("index");
    write_stl(library_dir.path(), "Minis/Dragon/dragon.stl");
    let app = App::open(index_dir.path(), 2, Arc::new(Silent)).expect("open");
    let library = app.add_library.execute("Minis", library_dir.path().to_str().expect("utf-8")).expect("add");
    app.scan_library.execute(library.id, ScanMode::Incremental).expect("scan");
    let search = |text: &str| {
        let request =
            SearchRequest { text: text.to_owned(), library: Some(library.id), scope: None, offset: 0, limit: 10 };
        app.search_models.execute(&request).expect("search").models
    };
    let dragon = search("dragon")[0].id;
    let meta = ModelMeta {
        display_name: Some("Red Wyrm".to_owned()),
        tags: [TagName::new("Resin").expect("valid")].into_iter().collect(),
        favorite: true,
        ..ModelMeta::default()
    };

    let detail = app.update_model_meta.execute(dragon, meta.clone()).expect("save");
    assert_eq!((detail.summary.title.as_str(), detail.original_title.as_str()), ("Red Wyrm", "Dragon"));
    assert_eq!(search("wyrm")[0].id, dragon);
    assert_eq!(search("tag:resin is:favorite")[0].id, dragon);
    let sidecar = library_dir.path().join("Minis/Dragon/.3dhoarddeck");
    assert!(!sidecar.exists(), "read-only libraries get no files");

    let (_, written) = app
        .update_library_settings
        .execute(library.id, AccessMode::MetadataWrite, MetadataStorage::IndexAndSidecar)
        .expect("allow sidecars");
    assert_eq!(written, 1, "existing data is written when metadata files are switched on");
    assert!(sidecar.exists());
    app.update_model_meta.execute(dragon, meta.clone()).expect("save again");

    app.update_model_meta.execute(dragon, ModelMeta::default()).expect("clear in index");
    app.update_library_settings
        .execute(library.id, AccessMode::ReadOnly, MetadataStorage::IndexOnly)
        .expect("read-only");
    fs::write(&sidecar, serde_json::to_vec(&meta).expect("json")).expect("restore file");
    let report = app.rebuild_from_sidecars.execute(library.id).expect("rebuild");
    assert_eq!((report.restored, report.unreadable), (1, 0));
    assert_eq!(search("wyrm")[0].id, dragon, "restored from the file");
    assert_eq!(app.list_tags.execute(library.id).expect("tags")[0].0.as_str(), "Resin");
}

#[test]
fn hand_made_collections_keep_their_order_and_smart_ones_follow_their_search() {
    let library_dir = tempfile::tempdir().expect("library");
    let index_dir = tempfile::tempdir().expect("index");
    for name in ["Alpha", "Bravo", "Charlie"] {
        write_stl(library_dir.path(), &format!("Minis/{name}/model.stl"));
    }
    let app = App::open(index_dir.path(), 2, Arc::new(Silent)).expect("open");
    let library = app.add_library.execute("Minis", library_dir.path().to_str().expect("utf-8")).expect("add");
    app.scan_library.execute(library.id, ScanMode::Incremental).expect("scan");
    let id = |name: &str| {
        let request =
            SearchRequest { text: name.to_owned(), library: Some(library.id), scope: None, offset: 0, limit: 5 };
        app.search_models.execute(&request).expect("search").models[0].id
    };
    let (alpha, bravo, charlie) = (id("alpha"), id("bravo"), id("charlie"));

    let chess = app.collections.create("  Chess   set ", None).expect("create");
    assert_eq!(chess.name, "Chess set");
    app.collections.add(chess.id, &[charlie, alpha, charlie]).expect("add");
    app.collections.add(chess.id, &[bravo]).expect("add more");
    app.collections.move_to(chess.id, bravo, 0).expect("move");
    let titles = |collection| {
        app.collections.models(collection, 0, 10).expect("models").0.into_iter().map(|m| m.title).collect::<Vec<_>>()
    };
    assert_eq!(titles(chess.id), vec!["Bravo", "Charlie", "Alpha"], "no duplicates, user order");
    app.collections.remove(chess.id, &[charlie]).expect("remove");
    assert_eq!(titles(chess.id), vec!["Bravo", "Alpha"]);
    assert_eq!(app.collections.list().expect("list")[0].1, Some(2));

    let smart = app
        .collections
        .create(
            "Starting with C",
            Some(SmartSearch { text: "charlie".to_owned(), library: Some(library.id), scope: None }),
        )
        .expect("smart");
    assert_eq!(titles(smart.id), vec!["Charlie"]);
    assert!(app.collections.add(smart.id, &[alpha]).is_err(), "smart collections fill themselves");
    app.collections.delete(chess.id).expect("delete");
    assert_eq!(app.collections.list().expect("list").len(), 1);
}

#[test]
fn identical_files_are_found_across_models_but_lookalikes_are_not() {
    let library_dir = tempfile::tempdir().expect("library");
    let index_dir = tempfile::tempdir().expect("index");
    let content = vec![9_u8; 300 * 1024];
    let mut lookalike = content.clone();
    lookalike[150 * 1024] = 1;
    for (path, bytes) in [
        ("Minis/Dragon/dragon.stl", &content),
        ("Packs/Vol 1/Dragon/dragon.stl", &content),
        ("Minis/Other/other.stl", &lookalike),
    ] {
        let file = library_dir.path().join(path);
        fs::create_dir_all(file.parent().expect("parent")).expect("dirs");
        fs::write(file, bytes).expect("write");
    }
    let app = App::open(index_dir.path(), 2, Arc::new(Silent)).expect("open");
    let library = app.add_library.execute("Minis", library_dir.path().to_str().expect("utf-8")).expect("add");
    app.scan_library.execute(library.id, ScanMode::Incremental).expect("scan");

    let groups = app.find_duplicates.execute().expect("duplicates");
    assert_eq!(groups.len(), 1);
    let paths: Vec<_> = groups[0].files.iter().map(|file| file.path.as_str().to_owned()).collect();
    assert_eq!(paths, vec!["Minis/Dragon/dragon.stl", "Packs/Vol 1/Dragon/dragon.stl"]);
    assert!(groups[0].files.iter().all(|file| file.model.is_some()), "each file links to its model");
}

#[test]
fn members_that_vanish_for_a_while_come_back_and_a_lost_cover_falls_back() {
    let library_dir = tempfile::tempdir().expect("library");
    let index_dir = tempfile::tempdir().expect("index");
    for name in ["Alpha", "Bravo", "Charlie"] {
        write_stl(library_dir.path(), &format!("Minis/{name}/model.stl"));
    }
    fs::write(library_dir.path().join("Minis/Alpha/a.png"), b"png").expect("image a");
    fs::write(library_dir.path().join("Minis/Alpha/b.png"), b"png").expect("image b");
    let app = App::open(index_dir.path(), 2, Arc::new(Silent)).expect("open");
    let library = app.add_library.execute("Minis", library_dir.path().to_str().expect("utf-8")).expect("add");
    let scan = || app.scan_library.execute(library.id, ScanMode::Incremental).expect("scan");
    scan();
    let id = |name: &str| {
        let request =
            SearchRequest { text: name.to_owned(), library: Some(library.id), scope: None, offset: 0, limit: 5 };
        app.search_models.execute(&request).expect("search").models.first().map(|model| model.id)
    };
    let (alpha, bravo, charlie) = (id("alpha").expect("a"), id("bravo").expect("b"), id("charlie").expect("c"));
    let set = app.collections.create("Set", None).expect("create");
    app.collections.add(set.id, &[alpha, bravo, charlie]).expect("add");

    let hidden = library_dir.path().join("Minis/Bravo");
    fs::rename(&hidden, library_dir.path().join("Bravo-away")).expect("hide bravo");
    scan();
    app.collections.move_to(set.id, charlie, 0).expect("reorder while Bravo is missing");
    fs::rename(library_dir.path().join("Bravo-away"), &hidden).expect("bring bravo back");
    scan();
    let titles: Vec<String> =
        app.collections.models(set.id, 0, 10).expect("models").0.into_iter().map(|m| m.title).collect();
    assert_eq!(titles, vec!["Charlie", "Alpha", "Bravo"], "Bravo kept its slot after Alpha");

    let picked = hoarddeck_domain::RelPath::new("Minis/Alpha/b.png").expect("valid");
    let meta = ModelMeta { cover: Some(picked.clone()), ..ModelMeta::default() };
    let detail = app.update_model_meta.execute(alpha, meta).expect("pick cover");
    assert_eq!(detail.summary.cover, Some(picked));
    fs::remove_file(library_dir.path().join("Minis/Alpha/b.png")).expect("lose the picked image");
    scan();
    let alpha_now = id("alpha").expect("alpha");
    let summary = app.get_model.execute(alpha_now).expect("get").expect("exists").summary;
    assert_eq!(
        summary.cover.map(String::from),
        Some("Minis/Alpha/a.png".to_owned()),
        "falls back to the detected cover"
    );
}

#[test]
fn folder_rules_survive_a_lost_index_through_the_metadata_files() {
    use hoarddeck_application::scan::FolderRole;
    use hoarddeck_domain::RelPath;

    let library_dir = tempfile::tempdir().expect("library");
    let root = library_dir.path();
    fs::create_dir_all(root.join("Minis/Dragon/Base")).expect("dirs");
    fs::write(root.join("Minis/Dragon/dragon.stl"), vec![0_u8; 134]).expect("stl");
    fs::write(root.join("Minis/Dragon/Base/base.stl"), vec![0_u8; 134]).expect("stl");
    let dragon = RelPath::new("Minis/Dragon").expect("valid");
    {
        let index_dir = tempfile::tempdir().expect("index");
        let app = App::open(index_dir.path(), 2, Arc::new(Silent)).expect("open");
        let library = app.add_library.execute("Minis", root.to_str().expect("utf-8")).expect("add");
        app.update_library_settings
            .execute(library.id, AccessMode::MetadataWrite, MetadataStorage::IndexAndSidecar)
            .expect("metadata files");
        app.scan_library.execute(library.id, ScanMode::Incremental).expect("scan");
        app.set_folder_role.execute(library.id, &dragon, Some(FolderRole::Model)).expect("mark");
        assert!(root.join(".3dhoarddeck").exists(), "the rules file is written at the library root");
    }
    // A new computer: a fresh index over the same folders.
    let index_dir = tempfile::tempdir().expect("new index");
    let app = App::open(index_dir.path(), 2, Arc::new(Silent)).expect("open");
    let library = app.add_library.execute("Minis", root.to_str().expect("utf-8")).expect("add");
    app.scan_library.execute(library.id, ScanMode::Incremental).expect("scan");
    let report = app.rebuild_from_sidecars.execute(library.id).expect("rebuild");
    assert_eq!(report.rules_restored, 1);
    let rules = app.folder_rules.list(library.id).expect("rules");
    assert_eq!(rules.len(), 1, "{rules:?}");
    let request = SearchRequest { text: "dragon".to_owned(), library: None, scope: None, offset: 0, limit: 5 };
    let found = app.search_models.execute(&request).expect("search").models;
    assert_eq!(found.len(), 1, "the dragon and its base are one model again");
    assert_eq!(found[0].path, dragon);
}

#[test]
fn pictures_of_a_folder_without_its_own_model_files_are_shown_on_its_page() {
    use hoarddeck_application::browse::{BrowseRequest, Sort};
    use hoarddeck_domain::RelPath;

    let library_dir = tempfile::tempdir().expect("library");
    let index_dir = tempfile::tempdir().expect("index");
    let root = library_dir.path();
    for file in ["Pack/A/a.stl", "Pack/B/b.stl", "Pack/Solo/solo.stl"] {
        fs::create_dir_all(root.join(file).parent().expect("parent")).expect("dirs");
        fs::write(root.join(file), vec![0_u8; 134]).expect("stl");
    }
    fs::write(root.join("Pack/overview.jpg"), b"jpg").expect("loose picture");
    fs::write(root.join("Pack/A.png"), b"png").expect("cover of A");
    let app = App::open(index_dir.path(), 2, Arc::new(Silent)).expect("open");
    let library = app.add_library.execute("Minis", root.to_str().expect("utf-8")).expect("add");
    app.scan_library.execute(library.id, ScanMode::Incremental).expect("scan");

    let page = app
        .browse_folder
        .execute(&BrowseRequest {
            library: library.id,
            folder: RelPath::new("Pack").expect("valid"),
            recursive: false,
            sort: Sort::Name,
            cursor: None,
            limit: 50,
        })
        .expect("browse");
    assert_eq!(page.images, [RelPath::new("Pack/overview.jpg").expect("valid")], "A.png is A's cover, not loose");
}

#[test]
fn scanning_one_folder_updates_it_and_leaves_the_rest_alone() {
    use hoarddeck_domain::RelPath;

    let library_dir = tempfile::tempdir().expect("library");
    let index_dir = tempfile::tempdir().expect("index");
    let root = library_dir.path();
    for file in ["A/old/old.stl", "B/kept/kept.stl"] {
        fs::create_dir_all(root.join(file).parent().expect("parent")).expect("dirs");
        fs::write(root.join(file), vec![0_u8; 134]).expect("stl");
    }
    let app = App::open(index_dir.path(), 2, Arc::new(Silent)).expect("open");
    let library = app.add_library.execute("Minis", root.to_str().expect("utf-8")).expect("add");
    app.scan_library.execute(library.id, ScanMode::Incremental).expect("scan");

    fs::remove_dir_all(root.join("A/old")).expect("remove in A");
    fs::create_dir_all(root.join("A/new")).expect("dir");
    fs::write(root.join("A/new/new.stl"), vec![0_u8; 134]).expect("new in A");
    fs::create_dir_all(root.join("B/later")).expect("dir");
    fs::write(root.join("B/later/later.stl"), vec![0_u8; 134]).expect("new in B");
    fs::remove_dir_all(root.join("B/kept")).expect("remove in B");

    let report = app
        .scan_library
        .scan_folder(library.id, &RelPath::new("A").expect("valid"), ScanMode::Incremental)
        .expect("folder scan");
    assert!(report.partial);
    let titles = |text: &str| {
        let request = SearchRequest { text: text.to_owned(), library: None, scope: None, offset: 0, limit: 5 };
        app.search_models.execute(&request).expect("search").models.len()
    };
    assert_eq!(titles("new"), 1, "the new model in A is found");
    assert_eq!(titles("old"), 0, "the removed model in A is gone");
    assert_eq!(titles("later"), 0, "B was not scanned");
    assert_eq!(titles("kept"), 1, "B keeps what it had");
}

#[test]
fn a_chosen_folder_cover_works_read_only_and_is_copied_where_writing_is_allowed() {
    use hoarddeck_domain::RelPath;

    let library_dir = tempfile::tempdir().expect("library");
    let index_dir = tempfile::tempdir().expect("index");
    let root = library_dir.path();
    for file in ["Creator/Dragon/dragon.stl", "Creator/Orc/orc.stl"] {
        fs::create_dir_all(root.join(file).parent().expect("parent")).expect("dirs");
        fs::write(root.join(file), vec![0_u8; 134]).expect("stl");
    }
    fs::write(root.join("Creator/Dragon/dragon.jpg"), b"detected").expect("image");
    fs::write(root.join("Creator/Orc/orc_render.png"), b"chosen").expect("image");
    let app = App::open(index_dir.path(), 2, Arc::new(Silent)).expect("open");
    let library = app.add_library.execute("Minis", root.to_str().expect("utf-8")).expect("add");
    app.scan_library.execute(library.id, ScanMode::Incremental).expect("scan");
    let creator = RelPath::new("Creator").expect("valid");
    let chosen = RelPath::new("Creator/Orc/orc_render.png").expect("valid");
    let cover = || {
        let folders = app.get_folder_children.execute(library.id, &RelPath::root()).expect("folders");
        folders.into_iter().find(|folder| folder.path == creator).expect("Creator").cover
    };
    assert_eq!(cover(), Some(RelPath::new("Creator/Dragon/dragon.jpg").expect("valid")), "detected");

    assert_eq!(app.set_folder_cover.execute(library.id, &creator, Some(&chosen)).expect("choose"), None);
    assert_eq!(cover(), Some(chosen.clone()), "read-only: kept in the index");
    assert!(!root.join("Creator/folder.png").exists(), "nothing written into a read-only library");
    assert!(
        app.set_folder_cover
            .execute(library.id, &creator, Some(&RelPath::new("Creator/Orc/orc.stl").expect("valid")))
            .is_err()
    );

    app.update_library_settings
        .execute(library.id, AccessMode::MetadataWrite, MetadataStorage::IndexOnly)
        .expect("write");
    let copy = app.set_folder_cover.execute(library.id, &creator, Some(&chosen)).expect("choose again");
    assert_eq!(copy, Some(RelPath::new("Creator/folder.png").expect("valid")));
    assert_eq!(fs::read(root.join("Creator/folder.png")).expect("copied"), b"chosen");
    assert_eq!(cover(), copy, "the copy is the cover");

    app.set_folder_cover.execute(library.id, &creator, None).expect("back to automatic");
    assert_eq!(cover(), copy, "folder.png is now the folder's own picture, so it stays the detected cover");
}
