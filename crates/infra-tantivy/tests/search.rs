use hoarddeck_application::ports::SearchIndex;
use hoarddeck_application::search::{SearchDocument, SearchQuery};
use hoarddeck_domain::{FileFormat, LibraryId, ModelId, RelPath};
use hoarddeck_infra_tantivy::TantivySearchIndex;

const LIBRARY: LibraryId = LibraryId(1);

fn document(id: i64, title: &str, path: &str, formats: Vec<FileFormat>) -> SearchDocument {
    let path = RelPath::new(path).expect("valid path");
    SearchDocument {
        id: ModelId(id),
        library: LIBRARY,
        title: title.to_owned(),
        folder: path.parent().expect("not root"),
        path,
        formats,
    }
}

fn query(words: &[&str]) -> SearchQuery {
    SearchQuery {
        words: words.iter().map(|w| (*w).to_owned()).collect(),
        formats: Vec::new(),
        library: Some(LIBRARY),
        scope: None,
        offset: 0,
        limit: 50,
    }
}

fn index() -> (tempfile::TempDir, TantivySearchIndex) {
    let dir = tempfile::tempdir().expect("temp dir");
    let index = TantivySearchIndex::open(&dir.path().join("search")).expect("open");
    assert!(index.was_reset(), "a new index needs a rebuild");
    index
        .replace_library(
            LIBRARY,
            &[
                document(1, "Wolverine", "Marvel/Heroes/Wolverine", vec![FileFormat::Stl]),
                document(2, "Gustav The Turtle", "Miniatures/Wild Life/Gustav The Turtle", vec![FileFormat::ThreeMf]),
                document(3, "Dragón Head", "Miniatures/Fantasy/Dragon_Head", vec![FileFormat::Stl]),
                document(4, "Turtle Shell Planter", "Home/Turtle Shell Planter", vec![FileFormat::Stl]),
                // Decomposed (NFD) as macOS and SMB shares often report it: "u" + U+0308.
                document(5, "Wu\u{308}rfel", "Spiele/Wu\u{308}rfel/Wu\u{308}rfel", vec![FileFormat::Stl]),
            ],
        )
        .expect("index docs");
    (dir, index)
}

fn ids(index: &TantivySearchIndex, query: &SearchQuery) -> Vec<i64> {
    index.search(query).expect("search").ids.into_iter().map(|id| id.0).collect()
}

#[test]
fn typos_prefixes_and_accents_still_find_the_model() {
    let (_dir, index) = index();
    assert_eq!(ids(&index, &query(&["wolverin"])), vec![1]);
    assert_eq!(ids(&index, &query(&["wolvrine"])), vec![1]);
    assert_eq!(ids(&index, &query(&["gus"])), vec![2]);
    assert_eq!(ids(&index, &query(&["dragon", "head"])), vec![3]);
}

#[test]
fn decomposed_names_match_composed_queries_and_scopes() {
    let (_dir, index) = index();
    assert_eq!(ids(&index, &query(&["würfel"])), vec![5]);
    let scoped = SearchQuery { scope: Some(RelPath::new("Spiele/Würfel").expect("valid")), ..query(&[]) };
    assert_eq!(ids(&index, &scoped), vec![5]);
}

#[test]
fn folder_scope_and_format_filters_narrow_results() {
    let (_dir, index) = index();
    let all_turtles = ids(&index, &query(&["turtle"]));
    assert_eq!(all_turtles.len(), 2);
    let scoped = SearchQuery { scope: Some(RelPath::new("Miniatures").expect("valid")), ..query(&["turtle"]) };
    assert_eq!(ids(&index, &scoped), vec![2]);
    let stl_only = SearchQuery { formats: vec![FileFormat::Stl], ..query(&["turtle"]) };
    assert_eq!(ids(&index, &stl_only), vec![4]);
    assert_eq!(index.search(&query(&[])).expect("all").total, 5);
}

#[test]
fn reopening_keeps_the_index_and_replacing_a_library_drops_old_documents() {
    let (dir, index) = index();
    drop(index);
    let index = TantivySearchIndex::open(&dir.path().join("search")).expect("reopen");
    assert!(!index.was_reset());
    assert_eq!(index.search(&query(&[])).expect("all").total, 5);
    index.replace_library(LIBRARY, &[document(9, "Benchy", "Calibration/Benchy", vec![])]).expect("replace");
    assert_eq!(ids(&index, &query(&[])), vec![9]);
}
