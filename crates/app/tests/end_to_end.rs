//! Scan a generated library with the real adapters, then browse and search it.

use std::fs;
use std::path::Path;
use std::sync::Arc;

use hoarddeck_app::App;
use hoarddeck_application::browse::{BrowseRequest, Sort};
use hoarddeck_application::ports::EventSink;
use hoarddeck_application::scan::{FolderRole, ScanMode, ScanProgress};
use hoarddeck_application::search::SearchRequest;
use hoarddeck_domain::{ModelKind, RelPath};

struct Silent;

impl EventSink for Silent {
    fn scan_progress(&self, _: &ScanProgress) {}
}

fn write(root: &Path, relative: &str) {
    let path = root.join(relative);
    fs::create_dir_all(path.parent().expect("has parent")).expect("create dirs");
    let mut stl = vec![0_u8; 134];
    stl[80] = 1; // one triangle: 84 + 50 bytes
    fs::write(path, stl).expect("write file");
}

#[test]
fn scan_browse_and_search_a_library_without_writing_into_it() {
    let library_dir = tempfile::tempdir().expect("library");
    let index_dir = tempfile::tempdir().expect("index");
    let root = library_dir.path();
    write(root, "Miniatures/Wild Life/Gustav The Turtle/Supported/gustav.stl");
    write(root, "Miniatures/Wild Life/Gustav The Turtle/Unsupported/gustav.stl.xxx");
    write(root, "Miniatures/Creator/2022-09 - September Release/Wolverine/wolverine.stl");
    write(root, "Makerworld/13661.3mf");
    write(root, "Makerworld/Busts/bust.stl");
    write(root, "Makerworld/@eaDir/thumb.stl");
    let before = snapshot(root);

    let app = App::open(index_dir.path(), 2, Arc::new(Silent)).expect("open app");
    let library = app.add_library.execute("3D Print", root.to_str().expect("utf-8")).expect("add");
    let report = app.scan_library.execute(library.id, ScanMode::Incremental).expect("scan");
    assert_eq!(report.models, 4);
    assert_eq!(snapshot(root), before, "the library must stay untouched");

    let rescan = app.scan_library.execute(library.id, ScanMode::Incremental).expect("rescan");
    assert_eq!(
        (rescan.dirs_listed, rescan.entries_removed, rescan.models),
        (0, 0, 4),
        "nothing changed, nothing listed"
    );
    let full = app.scan_library.execute(library.id, ScanMode::Full).expect("full rescan");
    assert_eq!(
        (full.dirs_listed, full.entries_removed, full.models),
        (rescan.dirs_unchanged, 0, 4),
        "full lists every folder"
    );

    let browse = |folder: &str, recursive| {
        let request = BrowseRequest {
            library: library.id,
            folder: RelPath::new(folder).expect("valid"),
            recursive,
            sort: Sort::Name,
            cursor: None,
            limit: 50,
        };
        app.browse_folder.execute(&request).expect("browse")
    };
    let top = browse("", false);
    assert_eq!(
        top.folders.iter().map(|f| f.display_name.as_str()).collect::<Vec<_>>(),
        vec!["Makerworld", "Miniatures"]
    );
    let minis = browse("Miniatures", true);
    assert_eq!(
        minis.page.models.iter().map(|m| m.title.as_str()).collect::<Vec<_>>(),
        vec!["Gustav The Turtle", "Wolverine"]
    );
    let gustav = app.get_model.execute(minis.page.models[0].id).expect("get").expect("exists");
    assert_eq!(gustav.files.len(), 2, "variants merged into one model");

    let search = |text: &str| {
        let request =
            SearchRequest { text: text.to_owned(), library: Some(library.id), scope: None, offset: 0, limit: 10 };
        app.search_models.execute(&request).expect("search").models
    };
    assert_eq!(search("wolverin").first().map(|m| m.title.as_str()), Some("Wolverine"));
    let loose = search("13661 type:3mf");
    assert_eq!(loose.first().map(|m| m.kind), Some(ModelKind::File));
    assert_eq!(search("turtle in:Makerworld"), Vec::new());

    let proposals = app.collection_presets.propose(library.id).expect("propose");
    assert_eq!(proposals.iter().map(|p| (p.id.as_str(), p.matches)).collect::<Vec<_>>(), vec![("creator-releases", 1)]);
    assert_eq!(proposals[0].samples, vec!["September Release"]);
    app.collection_presets.accept(library.id, "creator-releases").expect("accept");
    let wolverine = search("wolverine");
    let release = RelPath::new("Miniatures/Creator/2022-09 - September Release").expect("valid");
    assert_eq!(wolverine[0].collection, Some(release.clone()), "the release became a collection");
    assert!(
        app.collection_presets.propose(library.id).expect("propose again").is_empty(),
        "accepted presets are not proposed again"
    );
    let rules = app.folder_rules.list(library.id).expect("rules");
    app.folder_rules.remove(library.id, &rules[0].pattern).expect("remove");
    assert_eq!(search("wolverine")[0].collection, None);
}

#[test]
fn marking_folders_regroups_without_rescanning() {
    let library_dir = tempfile::tempdir().expect("library");
    let index_dir = tempfile::tempdir().expect("index");
    let root = library_dir.path();
    write(root, "Minis/Dragon/dragon.stl");
    write(root, "Minis/Dragon/Base/base.stl");
    let app = App::open(index_dir.path(), 2, Arc::new(Silent)).expect("open app");
    let library = app.add_library.execute("Minis", root.to_str().expect("utf-8")).expect("add");
    app.scan_library.execute(library.id, ScanMode::Incremental).expect("scan");
    let everything = || {
        let request = BrowseRequest {
            library: library.id,
            folder: RelPath::root(),
            recursive: true,
            sort: Sort::Name,
            cursor: None,
            limit: 50,
        };
        app.browse_folder.execute(&request).expect("browse").page.models
    };
    assert_eq!(everything().len(), 2, "automatic: loose dragon.stl and the Base folder");

    let dragon = RelPath::new("Minis/Dragon").expect("valid");
    app.set_folder_role.execute(library.id, &dragon, Some(FolderRole::Model)).expect("mark as model");
    let models = everything();
    assert_eq!(models.iter().map(|m| (m.title.as_str(), m.file_count)).collect::<Vec<_>>(), vec![("Dragon", 2)]);

    let minis = RelPath::new("Minis").expect("valid");
    app.set_folder_role.execute(library.id, &minis, Some(FolderRole::Collection)).expect("mark as collection");
    assert_eq!(everything()[0].collection, Some(minis.clone()));

    app.set_folder_role.execute(library.id, &dragon, None).expect("back to automatic");
    assert_eq!(everything().len(), 2);
    assert!(app.set_folder_role.execute(library.id, &RelPath::root(), Some(FolderRole::Model)).is_err());
}

fn snapshot(root: &Path) -> Vec<(String, u64)> {
    let mut files = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).expect("read dir") {
            let entry = entry.expect("entry");
            let metadata = entry.metadata().expect("metadata");
            if metadata.is_dir() {
                stack.push(entry.path());
            } else {
                files.push((entry.path().display().to_string(), metadata.len()));
            }
        }
    }
    files.sort();
    files
}
