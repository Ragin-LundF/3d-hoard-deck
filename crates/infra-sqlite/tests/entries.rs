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

#[test]
fn unchanged_folders_cost_no_file_writes_and_vanished_folders_go_with_everything_below() {
    let (_dir, index, id) = open();
    let (first, _) = index.begin_scan(id).expect("begin");
    index
        .apply(
            id,
            first,
            &[
                ScanItem::Listed {
                    dir: RelPath::root(),
                    mtime: 1,
                    children: vec![dir("Keep"), dir("Gone"), dir("Was")],
                },
                ScanItem::Listed { dir: path("Keep"), mtime: 1, children: vec![stl("Keep/a.stl")] },
                ScanItem::Listed { dir: path("Gone"), mtime: 1, children: vec![dir("Gone/Deep")] },
                ScanItem::Listed { dir: path("Gone/Deep"), mtime: 1, children: vec![stl("Gone/Deep/b.stl")] },
                ScanItem::Listed { dir: path("Was"), mtime: 1, children: vec![stl("Was/c.stl")] },
            ],
        )
        .expect("apply");
    index.finish_scan(id, first).expect("finish");

    // Gone vanished, Was became a file, Keep is unchanged (only visited).
    let (second, _) = index.begin_scan(id).expect("begin again");
    index
        .apply(
            id,
            second,
            &[
                ScanItem::Listed { dir: RelPath::root(), mtime: 2, children: vec![dir("Keep"), stl("Was")] },
                ScanItem::Unchanged { dir: path("Keep") },
            ],
        )
        .expect("apply");
    assert_eq!(index.finish_scan(id, second).expect("finish"), 3, "Gone, Gone/Deep and its STL");
    let files: Vec<String> = index.files(id).expect("files").into_iter().map(|e| e.path.to_string()).collect();
    assert_eq!(files, ["Keep/a.stl", "Was"], "the file under the unchanged folder stays, Was/c.stl is gone");
}

#[test]
fn a_folder_scan_of_a_vanished_folder_removes_it_and_leaves_the_rest() {
    let (_dir, index, id) = open();
    let (first, _) = index.begin_scan(id).expect("begin");
    index
        .apply(
            id,
            first,
            &[
                ScanItem::Listed { dir: RelPath::root(), mtime: 1, children: vec![dir("A"), dir("B")] },
                ScanItem::Listed { dir: path("A"), mtime: 1, children: vec![stl("A/a.stl")] },
                ScanItem::Listed { dir: path("B"), mtime: 1, children: vec![stl("B/b.stl")] },
            ],
        )
        .expect("apply");
    index.finish_scan(id, first).expect("finish");
    let (second, _) = index.begin_scan(id).expect("folder scan");
    assert_eq!(
        index.finish_folder_scan(id, second, &path("A"), false).expect("finish"),
        2,
        "A was not visited: it is gone"
    );
    let unchanged = index.begin_scan(id).expect("again").0;
    index.apply(id, unchanged, &[ScanItem::Unchanged { dir: path("B") }]).expect("apply");
    assert_eq!(
        index.finish_folder_scan(id, unchanged, &path("B"), true).expect("finish"),
        0,
        "visited B keeps its files"
    );
}
