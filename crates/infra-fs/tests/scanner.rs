use std::fs;
use std::path::Path;
use std::sync::mpsc::sync_channel;

use hoarddeck_application::ScanError;
use hoarddeck_application::ports::LibraryScanner;
use hoarddeck_application::scan::{CompiledRules, KnownDir, KnownDirs, ScanItem, ScanRules};
use hoarddeck_domain::{EntryKind, FileFormat, FileKind};
use hoarddeck_infra_fs::FsLibraryScanner;

fn write(root: &Path, relative: &str, bytes: &[u8]) {
    let path = root.join(relative);
    fs::create_dir_all(path.parent().expect("has parent")).expect("create dirs");
    fs::write(path, bytes).expect("write file");
}

fn binary_stl(triangles: u32) -> Vec<u8> {
    let mut bytes = vec![0_u8; 84 + 50 * triangles as usize];
    bytes[80..84].copy_from_slice(&triangles.to_le_bytes());
    bytes
}

fn scan(root: &Path, known: &KnownDirs) -> Result<Vec<ScanItem>, ScanError> {
    let (sender, receiver) = sync_channel(16);
    let scanner = FsLibraryScanner::new(4);
    let root = root.to_str().expect("utf-8 temp path").to_owned();
    let known = known.clone();
    let walker = std::thread::spawn(move || {
        let rules = CompiledRules::compile(&ScanRules::default()).expect("default rules");
        scanner.scan(&root, &known, &rules, &sender)
    });
    let items: Vec<ScanItem> = receiver.into_iter().collect();
    walker.join().expect("walker thread")?;
    Ok(items)
}

fn known_from(items: &[ScanItem]) -> KnownDirs {
    items
        .iter()
        .filter_map(|item| match item {
            ScanItem::Listed { dir, mtime, children } => Some((
                dir.clone(),
                KnownDir {
                    mtime: *mtime,
                    child_dirs: children
                        .iter()
                        .filter(|c| c.kind == EntryKind::Dir)
                        .map(|c| c.path.name().to_owned())
                        .collect(),
                },
            )),
            _ => None,
        })
        .collect()
}

fn library() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("temp dir");
    write(dir.path(), "Minis/Turtle/turtle.stl.xxx", &binary_stl(3));
    write(dir.path(), "Minis/Turtle/head.stl.txt", b"solid head\n facet normal 0 0 1\n");
    write(dir.path(), "Minis/Turtle/readme.txt", b"print at 0.1 mm");
    write(dir.path(), "Minis/Turtle/images/cover.jpg", b"jpg");
    write(dir.path(), "Minis/@eaDir/turtle.stl.xxx@SynoEAStream", b"nas metadata");
    write(dir.path(), "Minis/.DS_Store", b"finder");
    dir
}

#[test]
fn first_scan_lists_every_folder_and_sniffs_disguised_meshes() {
    let dir = library();
    let items = scan(dir.path(), &KnownDirs::new()).expect("scan");
    assert_eq!(items.iter().filter(|i| matches!(i, ScanItem::Listed { .. })).count(), 4, "@eaDir is not walked");
    let kind_of = |name: &str| {
        items.iter().find_map(|item| match item {
            ScanItem::Listed { children, .. } => children.iter().find(|c| c.path.name() == name).map(|c| c.kind),
            _ => None,
        })
    };
    let stl = Some(EntryKind::File(FileKind::Mesh(FileFormat::Stl)));
    assert_eq!(kind_of("turtle.stl.xxx"), stl);
    assert_eq!(kind_of("head.stl.txt"), stl);
    assert_eq!(kind_of("readme.txt"), Some(EntryKind::File(FileKind::Document)));
    assert_eq!((kind_of("@eaDir"), kind_of(".DS_Store")), (None, None), "ignored paths are not stored");
}

#[test]
fn rescan_skips_unchanged_folders_and_relists_changed_ones() {
    let dir = library();
    let first = scan(dir.path(), &KnownDirs::new()).expect("first scan");
    write(dir.path(), "Minis/Turtle/new.stl", &binary_stl(1));
    let second = scan(dir.path(), &known_from(&first)).expect("second scan");
    let listed: Vec<&str> = second
        .iter()
        .filter_map(|item| match item {
            ScanItem::Listed { dir, .. } => Some(dir.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(listed, vec!["Minis/Turtle"]);
    assert_eq!(second.iter().filter(|i| matches!(i, ScanItem::Unchanged { .. })).count(), 3);
}

#[test]
fn missing_root_is_reported_offline() {
    let result = scan(Path::new("/definitely/not/mounted/3D Print"), &KnownDirs::new());
    assert!(matches!(result, Err(ScanError::Offline(_))));
}
