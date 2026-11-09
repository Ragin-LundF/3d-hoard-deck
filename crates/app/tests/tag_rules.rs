//! Tag rules with the real adapters: tags from folder names, searchable and counted, hidden per model.

use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use hoarddeck_app::App;
use hoarddeck_application::ports::EventSink;
use hoarddeck_application::scan::{ScanMode, ScanProgress};
use hoarddeck_application::search::SearchRequest;
use hoarddeck_application::tagging::tag_rule_presets;
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

#[test]
fn rule_tags_are_searchable_counted_and_can_be_hidden() {
    let library_dir = tempfile::tempdir().expect("library");
    let index_dir = tempfile::tempdir().expect("index");
    write_stl(library_dir.path(), "Miniatures/Forge/Resin/32mm/Knight/Supported/knight.stl");
    write_stl(library_dir.path(), "Miniatures/Forge/Resin/32mm/Orc/orc.stl");
    write_stl(library_dir.path(), "Hardware/Robust Bracket 10mm/bracket.stl");
    let app = App::open(index_dir.path(), 2, Arc::new(Silent)).expect("open");
    let library = app.add_library.execute("Lib", library_dir.path().to_str().expect("utf-8")).expect("add");
    app.scan_library.execute(library.id, ScanMode::Incremental).expect("scan");
    let total = |text: &str| {
        let request = SearchRequest { text: text.to_owned(), library: None, scope: None, offset: 0, limit: 5 };
        app.search_models.execute(&request).expect("search").total
    };

    let presets = tag_rule_presets();
    let counts = app.tag_rules.counts(&presets).expect("counts");
    let count_of = |tag: &str| counts[presets.iter().position(|rule| rule.tag == tag).expect("preset")];
    assert_eq!((count_of("{1}mm"), count_of("resin"), count_of("presupported"), count_of("{1}")), (2, 2, 1, 2));
    assert_eq!(total("tag:resin"), 0, "counting saves nothing");

    app.tag_rules.save(&presets).expect("save");
    assert!(app.tag_rules.editor.wait_for_regroups(Duration::from_secs(10)), "regrouped in the background");
    assert_eq!((total("tag:32mm"), total("tag:resin"), total("tag:presupported"), total("tag:forge")), (2, 2, 1, 2));
    assert_eq!(total("tag:10mm") + total("tag:bust"), 0, "no tags from unrelated words");
    let facets = app.tags.facets(library.id, &RelPath::root(), "", 10).expect("facets");
    assert!(facets.iter().any(|(name, count)| name.as_str() == "32mm" && *count == 2), "{facets:?}");

    let knight = {
        let request = SearchRequest { text: "knight".to_owned(), library: None, scope: None, offset: 0, limit: 1 };
        app.search_models.execute(&request).expect("search").models[0].id
    };
    let detail = app.get_model.execute(knight).expect("get").expect("found");
    assert!(detail.auto_tags.iter().any(|tag| tag.as_str() == "presupported"));
    let mut meta = detail.meta;
    meta.hidden_tags.insert(TagName::new("presupported").expect("valid"));
    app.update_model_meta.execute(knight, meta).expect("hide");
    assert_eq!(total("tag:presupported"), 0, "a hidden rule tag is not searched");
    let facets = app.tags.facets(library.id, &RelPath::root(), "", 10).expect("facets");
    assert!(!facets.iter().any(|(name, _)| name.as_str() == "presupported"), "nor counted: {facets:?}");

    app.tag_rules.save(&[]).expect("remove all rules");
    assert!(app.tag_rules.editor.wait_for_regroups(Duration::from_secs(10)));
    assert_eq!(total("tag:32mm"), 0, "removing a rule removes its tags");
}
