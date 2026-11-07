mod common;

use common::{dir, open, path, stl};
use hoarddeck_application::ports::EntryStore;
use hoarddeck_application::scan::ScanItem;
use hoarddeck_domain::RelPath;

#[test]
fn rescans_keep_unchanged_folders_remove_missing_files_and_spare_unreadable_ones() {
    let (_dir, index, id) = open();
    let (first, _) = index.begin_scan(id).expect("begin");
    index
        .apply(
            id,
            first,
            &[
                ScanItem::Listed { dir: RelPath::root(), mtime: 100, children: vec![dir("Minis"), stl("loose.stl")] },
                ScanItem::Listed { dir: path("Minis"), mtime: 200, children: vec![stl("Minis/turtle.stl")] },
            ],
        )
        .expect("apply");
    assert_eq!(index.finish_scan(id, first).expect("finish"), 0);

    let (second, known) = index.begin_scan(id).expect("begin again");
    assert_eq!(
        known.get(&RelPath::root()).map(|k| (k.mtime, k.child_dirs.clone())),
        Some((100, vec!["Minis".to_owned()]))
    );
    index
        .apply(
            id,
            second,
            &[
                ScanItem::Listed { dir: RelPath::root(), mtime: 101, children: vec![dir("Minis")] },
                ScanItem::Unchanged { dir: path("Minis") },
            ],
        )
        .expect("apply");
    assert_eq!(index.finish_scan(id, second).expect("finish"), 1, "loose.stl is gone");

    let (third, _) = index.begin_scan(id).expect("begin third");
    index
        .apply(
            id,
            third,
            &[
                ScanItem::Unchanged { dir: RelPath::root() },
                ScanItem::Failed { dir: path("Minis"), message: "permission denied".to_owned() },
            ],
        )
        .expect("apply");
    assert_eq!(index.finish_scan(id, third).expect("finish"), 0);
    let files: Vec<String> = index.files(id).expect("files").into_iter().map(|e| e.path.to_string()).collect();
    assert_eq!(files, vec!["Minis/turtle.stl"]);
}
