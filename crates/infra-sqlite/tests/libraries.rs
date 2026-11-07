mod common;

use common::open;
use hoarddeck_application::ports::LibraryRepository;
use hoarddeck_application::scan::ScanRules;
use hoarddeck_domain::AccessMode;

#[test]
fn libraries_are_read_only_by_default_and_roots_are_unique() {
    let (_dir, index, id) = open();
    let library = LibraryRepository::get(&index, id).expect("get").expect("exists");
    assert_eq!(library.settings.access(), AccessMode::ReadOnly);
    assert_eq!(index.scan_rules(id).expect("rules"), ScanRules::default());
    assert!(index.add("again", "/Volumes/volume/3D Print", &ScanRules::default()).is_err());
    assert_eq!(index.list().expect("list").len(), 1);
}

#[test]
fn settings_are_saved_and_the_index_can_be_copied_elsewhere() {
    use hoarddeck_application::ports::IndexExport;
    use hoarddeck_domain::{LibrarySettings, MetadataStorage};
    use hoarddeck_infra_sqlite::SqliteIndex;

    let (_dir, index, id) = open();
    let settings = LibrarySettings::new(AccessMode::MetadataWrite, MetadataStorage::IndexAndSidecar).expect("valid");
    index.save_settings(id, settings).expect("save settings");
    assert_eq!(LibraryRepository::get(&index, id).expect("get").expect("exists").settings, settings);

    let target = tempfile::tempdir().expect("target");
    assert_eq!(index.existing_index(target.path()), None);
    index.export_to(target.path()).expect("export");
    assert!(index.existing_index(target.path()).is_some());
    index.export_to(target.path()).expect("replacing an existing copy works");
    let copy = SqliteIndex::open(target.path()).expect("open copy");
    assert_eq!(copy.list().expect("list").len(), 1);
}

#[test]
fn the_slicer_list_is_empty_until_saved_and_round_trips() {
    use hoarddeck_application::ports::SlicerStore;
    use hoarddeck_domain::{Slicer, SlicerList};

    let (_dir, index, _) = open();
    assert_eq!(index.load_slicers().expect("load"), None);
    let bambu = Slicer::new("Bambu Studio", "/Applications/BambuStudio.app", Vec::new(), &["stl".into(), "3mf".into()])
        .expect("valid");
    let list = SlicerList { slicers: vec![bambu] };
    index.save_slicers(&list).expect("save");
    assert_eq!(index.load_slicers().expect("load again"), Some(list));
}
