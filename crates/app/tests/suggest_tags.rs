//! AI tag suggestions end to end, against a small fake OpenAI-compatible server on localhost.

use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::thread;
use std::time::{Duration, Instant};

use hoarddeck_app::App;
use hoarddeck_application::ai::{AiFolders, AiProvider, AiSettings, Decision, KeyChange};
use hoarddeck_application::ports::EventSink;
use hoarddeck_application::scan::{ScanMode, ScanProgress};
use hoarddeck_application::search::SearchRequest;
use hoarddeck_domain::{RelPath, TagName};

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

/// Answers every chat request with the same tags (including ones the app must drop) and lists one model.
fn fake_server() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let address = format!("http://{}/v1", listener.local_addr().expect("address"));
    thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let mut reader = BufReader::new(stream);
            let mut request_line = String::new();
            reader.read_line(&mut request_line).expect("request");
            let mut length = 0;
            loop {
                let mut header = String::new();
                reader.read_line(&mut header).expect("header");
                if header.trim().is_empty() {
                    break;
                }
                if let Some(value) = header.to_lowercase().strip_prefix("content-length:") {
                    length = value.trim().parse().unwrap_or(0);
                }
            }
            let mut body = vec![0; length];
            reader.read_exact(&mut body).expect("body");
            let answer = if request_line.contains("/models") {
                r#"{"data": [{"id": "text-embedding-x"}, {"id": "vision-model"}]}"#.to_owned()
            } else {
                let content =
                    r#"{\"tags\": [\"Dragon\", \"winged\", \"supported\", \"32mm\"], \"description\": \"a dragon\"}"#;
                format!(r#"{{"choices": [{{"message": {{"content": "", "reasoning_content": "{content}"}}}}]}}"#)
            };
            let mut stream = reader.into_inner();
            let _ = write!(
                stream,
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{answer}",
                answer.len()
            );
        }
    });
    address
}

#[test]
fn suggestions_are_stored_reviewed_and_kept() {
    let library_dir = tempfile::tempdir().expect("library");
    let index_dir = tempfile::tempdir().expect("index");
    write_stl(library_dir.path(), "Minis/Red Dragon/dragon.stl");
    write_stl(library_dir.path(), "Minis/Green Dragon/dragon.stl");
    let app = App::open(index_dir.path(), 2, Arc::new(Silent)).expect("open");
    let library = app.add_library.execute("Lib", library_dir.path().to_str().expect("utf-8")).expect("add");
    app.scan_library.execute(library.id, ScanMode::Incremental).expect("scan");
    let id_of = |text: &str| {
        let request = SearchRequest { text: text.to_owned(), library: None, scope: None, offset: 0, limit: 1 };
        app.search_models.execute(&request).expect("search").models[0].id
    };
    let red = id_of("red");

    assert!(app.suggest_tags.suggest_model(red).is_err(), "AI is off by default");
    let settings = AiSettings { provider: AiProvider::Custom, base_url: fake_server(), ..AiSettings::default() };
    let test = app.suggest_tags.test(&settings, None).expect("test connection");
    assert_eq!((test.models, test.model.as_str()), (vec!["vision-model".to_owned()], "vision-model"));
    app.suggest_tags.save_settings(&settings, KeyChange::Keep).expect("save");

    let tags = app.suggest_tags.suggest_model(red).expect("suggest");
    assert_eq!(
        tags.iter().map(TagName::as_str).collect::<Vec<_>>(),
        ["dragon", "winged"],
        "sizes and support words dropped"
    );
    app.suggest_tags.resolve(red, &TagName::new("dragon").expect("valid"), true).expect("keep");
    assert_eq!(
        app.suggest_tags.suggestions(red).expect("left").iter().map(TagName::as_str).collect::<Vec<_>>(),
        ["winged"]
    );

    let total = app.suggest_tags.start(library.id, &RelPath::root()).expect("start");
    assert_eq!(total, 1, "only the model without suggestions and own tags");
    let deadline = Instant::now() + Duration::from_secs(10);
    while app.suggest_tags.running.load(Ordering::SeqCst) {
        assert!(Instant::now() < deadline, "the run ends");
        thread::sleep(Duration::from_millis(20));
    }
    let counts = app.suggest_tags.counts(library.id).expect("counts");
    assert_eq!(
        counts.iter().map(|(tag, count)| (tag.as_str(), *count)).collect::<Vec<_>>(),
        [("winged", 2), ("dragon", 1)]
    );

    let winged = TagName::new("winged").expect("valid");
    let review = app.suggest_tags.review(library.id, &RelPath::root(), Some(&winged)).expect("review");
    assert_eq!(review.len(), 2);
    let green = review.iter().find(|item| item.id != red).expect("green").id;
    let decisions = [
        Decision { id: red, keep: vec![winged.clone()], dismiss: Vec::new() },
        Decision { id: green, keep: Vec::new(), dismiss: vec![winged.clone()] },
    ];
    let done = app.suggest_tags.decide(library.id, &decisions).expect("keep on one, skip one");
    let request = SearchRequest { text: "tag:winged".to_owned(), library: None, scope: None, offset: 0, limit: 5 };
    assert_eq!(app.search_models.execute(&request).expect("search").total, 1);
    assert_eq!(
        app.suggest_tags.pending_count(library.id, &RelPath::root()).expect("pending"),
        1,
        "green still has dragon"
    );
    app.suggest_tags.undo(library.id, &done).expect("undo");
    assert_eq!(app.search_models.execute(&request).expect("search").total, 0, "undo takes the kept tag back");
    assert_eq!(app.suggest_tags.review(library.id, &RelPath::root(), Some(&winged)).expect("review").len(), 2);

    let folders = AiFolders { included: vec![RelPath::new("Minis").expect("valid")], ..AiFolders::default() };
    assert_eq!(app.suggest_tags.count_folders(library.id, &folders).expect("count"), 0, "every model was asked");

    assert_eq!(app.suggest_tags.accept_all(library.id, &TagName::new("winged").expect("valid")).expect("accept"), 2);
    assert_eq!(app.search_models.execute(&request).expect("search").total, 2);
    assert_eq!(app.suggest_tags.dismiss_all(library.id, &TagName::new("dragon").expect("valid")).expect("dismiss"), 1);
    assert_eq!(app.suggest_tags.counts(library.id).expect("counts"), Vec::new());
}
