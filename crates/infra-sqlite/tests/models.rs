mod common;

use common::{model, open, path};
use hoarddeck_application::browse::{BrowseRequest, Sort};
use hoarddeck_application::ports::ModelRepository;
use hoarddeck_domain::{Catalog, FileFormat, Folder, Model, RelPath};

#[test]
fn catalog_replacement_keeps_ids_and_browsing_pages_by_keyset() {
    let (_dir, index, id) = open();
    let folder = Folder {
        path: path("Minis"),
        display_name: "Minis".to_owned(),
        released: None,
        direct_models: 3,
        total_models: 4,
        cover: None,
        is_collection: false,
    };
    let mut catalog = Catalog {
        folders: vec![folder],
        models: vec![
            model("Minis/Charlie", "Charlie"),
            model("Minis/alpha", "alpha"),
            model("Minis/Bravo", "Bravo"),
            model("Minis/Deep/Delta", "Delta"),
        ],
    };
    index.replace_catalog(id, &catalog).expect("replace");

    let request = |cursor| BrowseRequest {
        library: id,
        folder: path("Minis"),
        recursive: false,
        sort: Sort::Name,
        cursor,
        limit: 2,
    };
    let first = index.browse(&request(None)).expect("first page");
    let titles: Vec<_> = first.models.iter().map(|m| m.title.as_str()).collect();
    assert_eq!(titles, vec!["alpha", "Bravo"], "case-insensitive name order");
    let second = index.browse(&request(first.next.clone())).expect("second page");
    assert_eq!(second.models.iter().map(|m| m.title.as_str()).collect::<Vec<_>>(), vec!["Charlie"]);
    assert!(second.next.is_none());

    let everything = BrowseRequest { recursive: true, limit: 10, ..request(None) };
    assert_eq!(index.browse(&everything).expect("recursive").models.len(), 4);
    assert_eq!(index.child_folders(id, &RelPath::root()).expect("folders").len(), 1);

    let bravo = first.models[1].id;
    catalog.models.remove(0);
    index.replace_catalog(id, &catalog).expect("replace again");
    let detail = ModelRepository::get(&index, bravo).expect("get").expect("Bravo keeps its id");
    assert_eq!(detail.files.len(), 1);
    assert_eq!(detail.summary.formats, vec![FileFormat::Stl]);
    assert_eq!(index.search_documents(id).expect("docs").len(), 3);
    let ids = [first.models[1].id, first.models[0].id];
    let ordered: Vec<_> = index.summaries(&ids).expect("summaries").iter().map(|s| s.id).collect();
    assert_eq!(ordered, ids.to_vec());
}

#[test]
fn recursive_scope_excludes_sibling_folders_that_share_a_prefix() {
    let (_dir, index, id) = open();
    let catalog = Catalog {
        folders: Vec::new(),
        models: vec![
            model("A/m1", "in A"),
            model("A/x/m2", "below A"),
            model("A B/m3", "sibling"),
            model("A0/m4", "sibling 0"),
        ],
    };
    index.replace_catalog(id, &catalog).expect("replace");
    let request =
        BrowseRequest { library: id, folder: path("A"), recursive: true, sort: Sort::Name, cursor: None, limit: 10 };
    let titles: Vec<String> = index.browse(&request).expect("browse").models.into_iter().map(|m| m.title).collect();
    assert_eq!(titles, vec!["below A", "in A"]);
}

#[test]
fn release_and_added_sorts_page_newest_first() {
    let (_dir, index, id) = open();
    let released = |path: &str, year_month: Option<(u16, u8)>| Model {
        released: year_month.map(|(year, month)| hoarddeck_domain::YearMonth { year, month }),
        ..model(path, path)
    };
    let catalog = Catalog {
        folders: Vec::new(),
        models: vec![
            released("R/a", Some((2019, 5))),
            released("R/b", None),
            released("R/c", Some((2022, 9))),
            released("R/d", Some((2022, 9))),
        ],
    };
    index.replace_catalog(id, &catalog).expect("replace");
    let all_pages = |sort| {
        let mut titles = Vec::new();
        let mut cursor = None;
        loop {
            let request = BrowseRequest { library: id, folder: path("R"), recursive: false, sort, cursor, limit: 1 };
            let page = index.browse(&request).expect("page");
            titles.extend(page.models.into_iter().map(|m| m.title));
            match page.next {
                Some(next) => cursor = Some(next),
                None => return titles,
            }
        }
    };
    assert_eq!(all_pages(Sort::Released), vec!["R/d", "R/c", "R/a", "R/b"], "same month: newer id first; undated last");
    assert_eq!(all_pages(Sort::Added), vec!["R/d", "R/c", "R/b", "R/a"]);
}
