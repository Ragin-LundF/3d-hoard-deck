//! ZIP archives: their file list, and single files for the viewer. Reads only.

use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant, SystemTime};

use hoarddeck_application::ports::{ArchiveEntry, ArchiveReader};
use hoarddeck_domain::RelPath;
use zip::ZipArchive;

/// A huge archive lists only this many files; the viewer is no file manager.
pub const MAX_ENTRIES: usize = 5000;
/// The viewer often loads several files of one pack in a row; its directory is kept open this long.
const KEEP_OPEN: Duration = Duration::from_secs(60);

pub struct ZipArchives;

impl ArchiveReader for ZipArchives {
    fn list(&self, root: &str, archive: &RelPath) -> Result<Vec<ArchiveEntry>, String> {
        let path = inside(root, archive)?;
        let mut zip = open(&path)?;
        let mut entries = Vec::with_capacity(zip.len().min(MAX_ENTRIES));
        for index in 0..zip.len() {
            if entries.len() == MAX_ENTRIES {
                break;
            }
            let file = zip.by_index_raw(index).map_err(|error| format!("{archive}: {error}"))?;
            if file.is_file() {
                let name = file.name().map_err(|error| format!("{archive}: {error}"))?.into_owned();
                entries.push(ArchiveEntry { name, size: file.size() });
            }
        }
        entries.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(entries)
    }
}

/// One file from a ZIP, refused when it unpacks to more than `max_bytes` (so a zip bomb cannot fill memory).
/// # Errors
/// Fails when the archive or entry cannot be read, uses an unsupported compression, or is too large.
pub fn read_archive_entry(archive: &Path, name: &str, max_bytes: u64) -> Result<Vec<u8>, String> {
    let modified = fs::metadata(archive).and_then(|metadata| metadata.modified()).ok();
    let mut cached = OPEN.lock().unwrap_or_else(PoisonError::into_inner);
    let reusable = cached
        .as_ref()
        .is_some_and(|open| open.path == archive && open.modified == modified && open.opened.elapsed() < KEEP_OPEN);
    if !reusable {
        *cached = Some(Open { path: archive.to_path_buf(), modified, opened: Instant::now(), zip: open(archive)? });
    }
    let zip = &mut cached.as_mut().ok_or("archive cache is empty")?.zip;
    let file = zip.by_name(name).map_err(|error| format!("{name}: {error}"))?;
    if file.size() > max_bytes {
        return Err(format!("{name} is too large to preview"));
    }
    let mut bytes = Vec::new();
    // The declared size can lie; the read is capped either way.
    file.take(max_bytes + 1).read_to_end(&mut bytes).map_err(|error| format!("{name}: {error}"))?;
    if bytes.len() as u64 > max_bytes {
        return Err(format!("{name} is too large to preview"));
    }
    Ok(bytes)
}

struct Open {
    path: PathBuf,
    modified: Option<SystemTime>,
    opened: Instant,
    zip: ZipArchive<File>,
}

/// The last archive read from, so its central directory is not parsed again for every file.
static OPEN: Mutex<Option<Open>> = Mutex::new(None);

fn open(path: &Path) -> Result<ZipArchive<File>, String> {
    let file = File::open(path).map_err(|error| format!("{}: {error}", path.display()))?;
    ZipArchive::new(file).map_err(|error| format!("{}: {error}", path.display()))
}

/// The archive's real path, refused when a symlink leads outside the library.
fn inside(root: &str, archive: &RelPath) -> Result<PathBuf, String> {
    let root = fs::canonicalize(root).map_err(|error| format!("{root}: {error}"))?;
    let mut path = root.clone();
    path.extend(archive.segments());
    let real = fs::canonicalize(&path).map_err(|error| format!("{archive}: {error}"))?;
    if real.starts_with(&root) { Ok(real) } else { Err(format!("{archive} leads outside the library")) }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use zip::write::SimpleFileOptions;

    use super::*;

    #[test]
    fn lists_files_and_refuses_entries_that_unpack_too_large() {
        let dir = tempfile::tempdir().expect("temp dir");
        let mut writer = zip::ZipWriter::new(File::create(dir.path().join("pack.zip")).expect("create"));
        writer.add_directory("Dragon/", SimpleFileOptions::default()).expect("dir");
        writer.start_file("Dragon/dragon.stl", SimpleFileOptions::default()).expect("file");
        writer.write_all(&[0_u8; 4096]).expect("write");
        writer.finish().expect("finish");

        let root = dir.path().to_str().expect("utf-8");
        let entries = ZipArchives.list(root, &RelPath::new("pack.zip").expect("valid")).expect("list");
        assert_eq!(entries, [ArchiveEntry { name: "Dragon/dragon.stl".to_owned(), size: 4096 }]);
        let archive = dir.path().join("pack.zip");
        assert_eq!(read_archive_entry(&archive, "Dragon/dragon.stl", 4096).expect("read").len(), 4096);
        assert!(read_archive_entry(&archive, "Dragon/dragon.stl", 1000).is_err());
    }
}
