mod common;

use common::open;
use hoarddeck_application::ports::LibraryRepository;
use hoarddeck_application::scan::ScanRules;
use hoarddeck_domain::AccessMode;

#[test]
fn libraries_are_read_only_by_default_and_roots_are_unique() {
    let (_dir, index, id) = open();
    let library = LibraryRepository::get(&index, id).expect("get").expect("exists");
    assert_eq!(library.access, AccessMode::ReadOnly);
    assert_eq!(index.scan_rules(id).expect("rules"), ScanRules::default());
    assert!(index.add("again", "/Volumes/volume/3D Print", &ScanRules::default()).is_err());
    assert_eq!(index.list().expect("list").len(), 1);
}
