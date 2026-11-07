//! A file added to a watched local library shows up without a manual rescan.

use std::sync::Arc;
use std::time::{Duration, Instant};
use std::{fs, thread};

use hoarddeck_app::App;
use hoarddeck_application::ports::EventSink;
use hoarddeck_application::scan::{ScanMode, ScanProgress};
use hoarddeck_application::search::SearchRequest;

struct Silent;

impl EventSink for Silent {
    fn scan_progress(&self, _: &ScanProgress) {}
}

#[test]
fn a_new_file_is_indexed_after_the_folder_settles() {
    let library_dir = tempfile::tempdir().expect("library");
    let index_dir = tempfile::tempdir().expect("index");
    let root = fs::canonicalize(library_dir.path()).expect("canonical");
    let app = App::open(index_dir.path(), 2, Arc::new(Silent)).expect("open");
    let library = app.add_library.execute("Minis", root.to_str().expect("utf-8")).expect("add");
    app.scan_library.execute(library.id, ScanMode::Incremental).expect("scan");
    let start = Instant::now();
    assert_eq!(app.auto_rescan.tick(start).expect("tick"), []);
    assert_eq!(
        app.auto_rescan.tick(start + Duration::from_secs(11)).expect("tick"),
        [],
        "scanned recently: no startup scan"
    );
    thread::sleep(Duration::from_millis(300));

    fs::create_dir_all(root.join("Dragon")).expect("dir");
    fs::write(root.join("Dragon/dragon.stl"), vec![0_u8; 134]).expect("stl");
    let deadline = Instant::now() + Duration::from_secs(20);
    let mut scanned = Vec::new();
    while scanned.is_empty() && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(250));
        scanned = app.auto_rescan.tick(Instant::now() + Duration::from_secs(11)).expect("tick");
    }
    assert_eq!(scanned, [library.id]);
    let request = SearchRequest { text: "dragon".to_owned(), library: None, scope: None, offset: 0, limit: 5 };
    assert_eq!(app.search_models.execute(&request).expect("search").models.len(), 1);

    // Overwritten in place: the folder mtime stays, the reported folder is listed again anyway.
    thread::sleep(Duration::from_millis(300));
    fs::write(root.join("Dragon/dragon.stl"), vec![1_u8; 2000]).expect("overwrite");
    let deadline = Instant::now() + Duration::from_secs(20);
    let mut size = 0;
    while size < 2000 && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(250));
        app.auto_rescan.tick(Instant::now() + Duration::from_secs(11)).expect("tick");
        let found = app.search_models.execute(&request).expect("search").models;
        size = found.first().map_or(0, |model| model.total_size);
    }
    assert_eq!(size, 2000, "the new size was picked up");
}
