//! The archive adapter on real files: passwords, checksums, 7z, and nothing written outside the staging folder.

use std::fs;
use std::io::Write;
use std::path::Path;
use std::sync::atomic::AtomicBool;

use hoarddeck_application::ports::{ArchiveExtractor, ExtractControl, ExtractError};
use hoarddeck_domain::RelPath;
use hoarddeck_infra_archive::Archives;

fn rel(path: &str) -> RelPath {
    RelPath::new(path).expect("path")
}

fn zip_with(path: &Path, password: Option<&str>) {
    let mut zip = zip::ZipWriter::new(fs::File::create(path).expect("create"));
    let options = zip::write::SimpleFileOptions::default();
    let options = match password {
        Some(password) => options.with_aes_encryption(zip::AesMode::Aes256, password),
        None => options,
    };
    zip.start_file("Pack/a.stl", options).expect("start");
    zip.write_all(&[7_u8; 5000]).expect("write");
    zip.finish().expect("finish");
}

fn run(root: &Path, archive: &str, plan: &[(usize, RelPath)], password: Option<&str>) -> Result<(), ExtractError> {
    let cancel = AtomicBool::new(false);
    let progress = |_: u64, _: u64, _: &str| {};
    let control = ExtractControl { progress: &progress, cancel: &cancel };
    let root = root.to_str().expect("utf-8");
    Archives.extract(root, &rel(archive), &rel(".hoarddeck-extract-1"), plan, password, &control)
}

#[test]
fn encrypted_zips_need_the_right_password() {
    let dir = tempfile::tempdir().expect("dir");
    zip_with(&dir.path().join("Secret.zip"), Some("open sesame"));
    let root = dir.path().to_str().expect("utf-8");
    let info = Archives.inspect(root, &rel("Secret.zip"), None).expect("inspect");
    assert!(info.encrypted);
    let plan = [(0, rel("a.stl"))];
    assert_eq!(run(dir.path(), "Secret.zip", &plan, Some("wrong")), Err(ExtractError::WrongPassword));
    fs::remove_dir_all(dir.path().join(".hoarddeck-extract-1")).expect("clean");
    run(dir.path(), "Secret.zip", &plan, Some("open sesame")).expect("extract");
    assert_eq!(fs::read(dir.path().join(".hoarddeck-extract-1/a.stl")).expect("read"), vec![7_u8; 5000]);
}

#[test]
fn seven_zip_archives_are_listed_and_extracted() {
    let dir = tempfile::tempdir().expect("dir");
    let source = dir.path().join("src");
    fs::create_dir_all(source.join("Parts")).expect("dirs");
    fs::write(source.join("Parts/b.stl"), [3_u8; 2048]).expect("write");
    let library = dir.path().join("lib");
    fs::create_dir_all(&library).expect("lib");
    sevenz_rust2::compress_to_path(&source, library.join("Pack.7z")).expect("7z");
    let info = Archives.inspect(library.to_str().expect("utf-8"), &rel("Pack.7z"), None).expect("inspect");
    let file = info.entries.iter().find(|entry| !entry.is_dir).expect("a file");
    assert_eq!(file.size, 2048);
    run(&library, "Pack.7z", &[(file.index, rel("b.stl"))], None).expect("extract");
    assert_eq!(fs::read(library.join(".hoarddeck-extract-1/b.stl")).expect("read").len(), 2048);
}

#[test]
fn an_existing_staging_folder_or_another_folder_name_is_refused() {
    let dir = tempfile::tempdir().expect("dir");
    zip_with(&dir.path().join("Pack.zip"), None);
    let root = dir.path().to_str().expect("utf-8");
    let cancel = AtomicBool::new(false);
    let progress = |_: u64, _: u64, _: &str| {};
    let control = ExtractControl { progress: &progress, cancel: &cancel };
    let plan = [(0, rel("a.stl"))];
    assert!(
        Archives.extract(root, &rel("Pack.zip"), &rel("Pack"), &plan, None, &control).is_err(),
        "not a staging name"
    );
    fs::create_dir(dir.path().join(".hoarddeck-extract-1")).expect("existing");
    assert!(run(dir.path(), "Pack.zip", &plan, None).is_err(), "never writes into an existing folder");
}
