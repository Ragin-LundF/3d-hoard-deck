use hoarddeck_domain::{FileFormat, Sniff, classify_by_name};

use super::detect;
use super::images::model_stem;
use crate::scan::CompiledRules;
use crate::scan::{FolderRole, FolderRule, ScanRules};
use hoarddeck_domain::{Catalog, Entry, EntryKind, FileKind, ModelKind, RelPath, YearMonth};

fn file(path: &str, size: u64) -> Entry {
    let name = path.rsplit('/').next().unwrap_or(path);
    let kind = match classify_by_name(name) {
        Sniff::Known(kind) => kind,
        Sniff::NeedsHeader => FileKind::Other,
    };
    Entry { path: RelPath::new(path).expect("valid path"), kind: EntryKind::File(kind), size, mtime: 0 }
}

fn detect_with(rules: &ScanRules, entries: &[Entry]) -> Catalog {
    detect(entries, &CompiledRules::compile(rules).expect("valid rules"))
}

fn path(value: &str) -> RelPath {
    RelPath::new(value).expect("valid path")
}

#[test]
fn variant_subfolders_merge_into_one_model_with_a_named_cover() {
    let catalog = detect_with(
        &ScanRules::default(),
        &[
            file("Minis/Turtle/Supported/turtle.stl", 100),
            file("Minis/Turtle/Unsupported/turtle.stl", 90),
            file("Minis/Turtle/images/DSC_1234.jpg", 50),
            file("Minis/Turtle/Cover.png", 5),
        ],
    );
    assert_eq!(catalog.models.len(), 1);
    let model = &catalog.models[0];
    assert_eq!((model.kind, model.title.as_str(), &model.folder), (ModelKind::Folder, "Turtle", &path("Minis")));
    assert_eq!(model.cover, Some(path("Minis/Turtle/Cover.png")));
    assert_eq!(model.formats().into_iter().collect::<Vec<_>>(), vec![FileFormat::Stl]);
    let variants: Vec<_> = model.files.iter().filter_map(|f| f.variant.as_deref()).collect();
    assert_eq!(variants, vec!["Supported", "Unsupported", "images"]);
    assert_eq!(catalog.folders.len(), 1);
    assert_eq!((catalog.folders[0].direct_models, catalog.folders[0].total_models), (1, 1));
}

#[test]
fn release_folders_get_dates_and_sibling_covers() {
    let catalog = detect_with(
        &ScanRules::default(),
        &[
            file("Creator/2022-09 - September Release/Dragon/dragon.stl", 100),
            file("Creator/2022-09 - September Release.jpg", 10),
        ],
    );
    let model = &catalog.models[0];
    assert_eq!(model.released, Some(YearMonth { year: 2022, month: 9 }));
    let release =
        catalog.folders.iter().find(|f| f.path == path("Creator/2022-09 - September Release")).expect("release folder");
    assert_eq!(release.display_name, "September Release");
    assert_eq!(release.cover, Some(path("Creator/2022-09 - September Release.jpg")));
    let creator = catalog.folders.iter().find(|f| f.path == path("Creator")).expect("creator folder");
    assert_eq!((creator.direct_models, creator.total_models), (0, 1));
}

#[test]
fn loose_files_next_to_model_folders_are_models_of_their_own() {
    let catalog = detect_with(
        &ScanRules::default(),
        &[file("Makerworld/13661.3mf", 100), file("Makerworld/13661.png", 10), file("Makerworld/Busts/bust.stl", 100)],
    );
    let summary: Vec<_> = catalog.models.iter().map(|m| (m.kind, m.title.as_str(), m.cover.clone())).collect();
    assert_eq!(
        summary,
        vec![(ModelKind::File, "13661", Some(path("Makerworld/13661.png"))), (ModelKind::Folder, "Busts", None),]
    );
}

fn marked(folders: &[(&str, FolderRole)]) -> ScanRules {
    let folder_rules = folders.iter().map(|(folder, role)| FolderRule::for_folder(&path(folder), *role)).collect();
    ScanRules { folder_rules, ..ScanRules::default() }
}

#[test]
fn a_folder_with_model_files_and_a_part_subfolder_splits_until_the_user_marks_it() {
    let entries = [
        file("Minis/Dragon/dragon.stl", 100),
        file("Minis/Dragon/Supported/dragon.stl", 100),
        file("Minis/Dragon/Base/base.stl", 50),
        file("Minis/Dragon/dragon.jpg", 10),
    ];
    let automatic = detect_with(&ScanRules::default(), &entries);
    assert_eq!(automatic.models.len(), 3, "without a decision: two loose files plus the Base folder");

    let catalog = detect_with(&marked(&[("Minis/Dragon", FolderRole::Model)]), &entries);
    assert_eq!(catalog.models.len(), 1);
    let dragon = &catalog.models[0];
    assert_eq!((dragon.kind, dragon.title.as_str(), dragon.files.len()), (ModelKind::Folder, "Dragon", 4));
    assert_eq!(dragon.cover, Some(path("Minis/Dragon/dragon.jpg")));
    assert!(dragon.marked_by_user && !automatic.models[0].marked_by_user);
    let variants: Vec<_> = dragon.files.iter().filter_map(|f| f.variant.as_deref()).collect();
    assert_eq!(variants, vec!["Base", "Supported"]);
}

#[test]
fn a_collection_groups_folder_models_and_loose_files_below_it() {
    let rules = ScanRules {
        folder_rules: vec![FolderRule { pattern: "**/Loot Studios/*/*".to_owned(), role: FolderRole::Collection }],
        ..ScanRules::default()
    };
    let catalog = detect_with(
        &rules,
        &[
            file("Loot Studios/Fantasy/2023-01 - Dragons/Red/red.stl", 100),
            file("Loot Studios/Fantasy/2023-01 - Dragons/Blue/Supported/blue.stl", 100),
            file("Loot Studios/Fantasy/2023-01 - Dragons/bonus.stl", 100),
            file("Elsewhere/Other/other.stl", 100),
        ],
    );
    let membership: Vec<_> =
        catalog.models.iter().map(|m| (m.title.as_str(), m.collection.as_ref().map(RelPath::as_str))).collect();
    let dragons = Some("Loot Studios/Fantasy/2023-01 - Dragons");
    assert_eq!(membership, vec![("Other", None), ("Blue", dragons), ("Red", dragons), ("bonus", dragons)]);
    let collection = catalog.folders.iter().find(|f| f.path.as_str() == "Loot Studios/Fantasy/2023-01 - Dragons");
    assert_eq!(
        collection.map(|f| (f.is_collection, f.display_name.as_str(), f.total_models)),
        Some((true, "Dragons", 3))
    );
}

#[test]
fn disguised_meshes_look_for_covers_without_the_stl_extension() {
    assert_eq!(model_stem("Dragon.STL.txt"), "Dragon");
    assert_eq!(model_stem("bust.3mf"), "bust");
}

#[test]
fn ignored_files_and_image_only_folders_make_no_models() {
    let catalog = detect_with(&ScanRules::default(), &[file("_orynt3d/x.stl", 1), file("Photos/p.jpg", 1)]);
    assert_eq!(catalog, Catalog::default());
}

#[test]
fn files_in_the_root_and_in_top_level_variant_folders_are_kept() {
    let catalog = detect_with(&ScanRules::default(), &[file("x.stl", 1), file("Resin/y.stl", 1)]);
    let summary: Vec<_> = catalog.models.iter().map(|m| (m.kind, m.path.as_str())).collect();
    assert_eq!(summary, vec![(ModelKind::Folder, "Resin"), (ModelKind::File, "x.stl")]);
}
