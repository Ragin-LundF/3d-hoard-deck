mod common;

use common::{open, path};
use hoarddeck_application::ports::HashCache;

#[test]
fn a_changed_file_forgets_its_fingerprints() {
    let (_dir, index, library) = open();
    let file = (library, path("a.stl"), 2000, 10);
    index.store(&file, false, [1; 32]).expect("store");
    index.store(&file, true, [2; 32]).expect("store");
    assert_eq!(index.cached(&file, true).expect("read"), Some([2; 32]));

    let changed = (library, path("a.stl"), 2000, 11);
    assert_eq!(index.cached(&changed, false).expect("read"), None, "another modification time");
    index.store(&changed, false, [3; 32]).expect("store");
    assert_eq!(index.cached(&changed, false).expect("read"), Some([3; 32]));
    assert_eq!(index.cached(&changed, true).expect("read"), None, "the stale full fingerprint is dropped");
}
