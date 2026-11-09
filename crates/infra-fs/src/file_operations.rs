//! The only adapter that changes files inside a library. Used only for libraries in Managed mode.

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use hoarddeck_application::ports::{ArchiveStamp, Deleted, FileError, FileOperations, STAGING_PREFIX};
use hoarddeck_domain::RelPath;

pub struct FsFileOperations;

impl FileOperations for FsFileOperations {
    fn rename(&self, root: &str, from: &RelPath, to: &RelPath) -> Result<(), FileError> {
        let failed = |message: String| FileError::Failed(message);
        let root = fs::canonicalize(root).map_err(|error| failed(format!("{root}: {error}")))?;
        let (source, target) = (absolute(&root, from), absolute(&root, to));
        let parent = target.parent().ok_or_else(|| failed(format!("{to} has no parent folder")))?;
        // Symlinks must not lead a move out of the library, neither for the source nor for the target folder.
        inside(&root, &source).map_err(failed)?;
        inside(&root, &nearest_existing(parent)).map_err(failed)?;
        fs::create_dir_all(parent).map_err(|error| failed(format!("{}: {error}", parent.display())))?;
        inside(&root, parent).map_err(failed)?;
        let result = if same_file(&source, &target) {
            // A case-only rename on a case-insensitive disk: the "existing" target is the source itself.
            let step = renaming(&target);
            rename_new(&source, &step).and_then(|()| {
                rename_new(&step, &target).inspect_err(|_| {
                    let _ = rename_new(&step, &source);
                })
            })
        } else {
            rename_new(&source, &target)
        };
        result.map_err(|error| match error.kind() {
            io::ErrorKind::AlreadyExists => FileError::AlreadyExists,
            _ => failed(format!("{from} → {to}: {error}")),
        })
    }

    fn exists(&self, root: &str, path: &RelPath) -> Result<bool, String> {
        absolute(Path::new(root), path).try_exists().map_err(|error| format!("{path}: {error}"))
    }

    fn create_file(&self, root: &str, path: &RelPath, bytes: &[u8]) -> Result<(), FileError> {
        let failed = |message: String| FileError::Failed(message);
        let root = fs::canonicalize(root).map_err(|error| failed(format!("{root}: {error}")))?;
        let target = absolute(&root, path);
        let parent = target.parent().ok_or_else(|| failed(format!("{path} has no parent folder")))?;
        inside(&root, parent).map_err(failed)?;
        // `create_new` refuses an existing file atomically, also on network shares.
        let mut file = fs::OpenOptions::new().write(true).create_new(true).open(&target).map_err(|error| {
            if error.kind() == io::ErrorKind::AlreadyExists {
                FileError::AlreadyExists
            } else {
                failed(format!("{path}: {error}"))
            }
        })?;
        file.write_all(bytes).and_then(|()| file.sync_all()).map_err(|error| {
            // A half-written image is worse than none.
            let _ = fs::remove_file(&target);
            failed(format!("{path}: {error}"))
        })
    }

    fn copy_file(&self, root: &str, from: &RelPath, to: &RelPath) -> Result<u64, FileError> {
        let canonical = fs::canonicalize(root).map_err(|error| FileError::Failed(format!("{root}: {error}")))?;
        let source = absolute(&canonical, from);
        inside(&canonical, &source).map_err(FileError::Failed)?;
        let bytes = fs::read(&source).map_err(|error| FileError::Failed(format!("{from}: {error}")))?;
        self.create_file(root, to, &bytes)?;
        Ok(bytes.len() as u64)
    }

    fn remove_empty_dir(&self, root: &str, dir: &RelPath) -> Result<(), String> {
        // `remove_dir` refuses folders that are not empty.
        fs::remove_dir(absolute(Path::new(root), dir)).map_err(|error| format!("{dir}: {error}"))
    }

    fn recover(&self, root: &str, to: &RelPath) -> Result<bool, String> {
        let target = absolute(Path::new(root), to);
        let step = renaming(&target);
        if !step.try_exists().map_err(|error| error.to_string())? {
            return Ok(false);
        }
        rename_new(&step, &target).map(|()| true).map_err(|error| format!("{to}: {error}"))
    }

    fn stamp(&self, root: &str, path: &RelPath) -> Result<ArchiveStamp, String> {
        let metadata = fs::metadata(absolute(Path::new(root), path)).map_err(|error| format!("{path}: {error}"))?;
        let mtime = metadata
            .modified()
            .ok()
            .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
            .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX));
        Ok(ArchiveStamp { size: metadata.len(), mtime })
    }

    fn delete_archive(&self, root: &str, path: &RelPath, permanent: bool) -> Result<Deleted, FileError> {
        let failed = |message: String| FileError::Failed(message);
        let canonical = fs::canonicalize(root).map_err(|error| failed(format!("{root}: {error}")))?;
        let target = absolute(&canonical, path);
        inside(&canonical, &target).map_err(failed)?;
        if !fs::symlink_metadata(&target).map_err(|error| failed(format!("{path}: {error}")))?.is_file() {
            return Err(failed(format!("{path} is not a file")));
        }
        if permanent {
            fs::remove_file(&target).map_err(|error| failed(format!("{path}: {error}")))?;
            return Ok(Deleted::Removed);
        }
        match trash::delete(&target) {
            Ok(()) if !target.exists() => Ok(Deleted::Trashed),
            // Some drives (external disks without a trash folder) refuse; the file stays and the user decides.
            _ => Err(FileError::NoTrash),
        }
    }

    fn trash_dir(&self, root: &str, dir: &RelPath) -> Result<(), FileError> {
        let failed = |message: String| FileError::Failed(message);
        let canonical = fs::canonicalize(root).map_err(|error| failed(format!("{root}: {error}")))?;
        let target = absolute(&canonical, dir);
        inside(&canonical, &target).map_err(failed)?;
        let metadata = fs::symlink_metadata(&target).map_err(|error| failed(format!("{dir}: {error}")))?;
        if !metadata.is_dir() || dir.is_root() {
            return Err(failed(format!("{dir} is not a folder")));
        }
        match trash::delete(&target) {
            Ok(()) if !target.exists() => Ok(()),
            _ => Err(FileError::NoTrash),
        }
    }

    fn remove_staging_dir(&self, root: &str, dir: &RelPath) -> Result<(), String> {
        // The only recursive delete in the app, and only for its own hidden staging folders.
        if !dir.name().starts_with(STAGING_PREFIX) {
            return Err(format!("{dir} is not a staging folder"));
        }
        let canonical = fs::canonicalize(root).map_err(|error| format!("{root}: {error}"))?;
        let target = absolute(&canonical, dir);
        if !target.try_exists().map_err(|error| format!("{dir}: {error}"))? {
            return Ok(());
        }
        inside(&canonical, &target)?;
        if fs::symlink_metadata(&target).map_err(|error| format!("{dir}: {error}"))?.file_type().is_symlink() {
            return Err(format!("{dir} is a link"));
        }
        fs::remove_dir_all(&target).map_err(|error| format!("{dir}: {error}"))
    }
}

/// The temporary name of a case-only rename; the scanner skips it.
fn renaming(target: &Path) -> PathBuf {
    let name = target.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
    target.with_file_name(format!(".{name}.3dhoarddeck-renaming"))
}

fn absolute(root: &Path, path: &RelPath) -> PathBuf {
    let mut absolute = root.to_path_buf();
    absolute.extend(path.segments());
    absolute
}

fn inside(root: &Path, path: &Path) -> Result<(), String> {
    let real = fs::canonicalize(path).map_err(|error| format!("{}: {error}", path.display()))?;
    if real.starts_with(root) { Ok(()) } else { Err(format!("{} leads outside the library", path.display())) }
}

fn nearest_existing(path: &Path) -> PathBuf {
    path.ancestors().find(|candidate| candidate.exists()).unwrap_or(path).to_path_buf()
}

fn same_file(a: &Path, b: &Path) -> bool {
    match (fs::metadata(a), fs::metadata(b)) {
        #[cfg(unix)]
        (Ok(a), Ok(b)) => {
            use std::os::unix::fs::MetadataExt;
            a.dev() == b.dev() && a.ino() == b.ino()
        }
        #[cfg(not(unix))]
        (Ok(_), Ok(_)) => fs::canonicalize(a).ok() == fs::canonicalize(b).ok(),
        _ => false,
    }
}

/// Renames without ever replacing an existing target, atomically where the system offers it.
#[cfg(any(target_os = "macos", target_os = "linux"))]
fn rename_new(source: &Path, target: &Path) -> io::Result<()> {
    use rustix::fs::{CWD, RenameFlags, renameat_with};
    use rustix::io::Errno;
    match renameat_with(CWD, source, CWD, target, RenameFlags::NOREPLACE) {
        // Some file systems and network shares do not know the flag.
        Err(error) if [Errno::INVAL, Errno::NOTSUP, Errno::OPNOTSUPP, Errno::NOSYS].contains(&error) => {
            checked_rename(source, target)
        }
        result => result.map_err(io::Error::from),
    }
}

/// shortcut: on Windows the check and the rename are two steps; `MoveFileExW` without REPLACE_EXISTING would close
/// the gap but needs a Windows API crate.
#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn rename_new(source: &Path, target: &Path) -> io::Result<()> {
    checked_rename(source, target)
}

fn checked_rename(source: &Path, target: &Path) -> io::Result<()> {
    if target.try_exists()? {
        return Err(io::Error::new(io::ErrorKind::AlreadyExists, "target exists"));
    }
    fs::rename(source, target)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn path(value: &str) -> RelPath {
        RelPath::new(value).expect("valid")
    }

    #[test]
    fn renames_but_never_overwrites() {
        let dir = tempfile::tempdir().expect("temp dir");
        let root = dir.path().to_str().expect("utf-8");
        fs::create_dir_all(dir.path().join("Minis/Dragon")).expect("dirs");
        fs::write(dir.path().join("Minis/Dragon/a.stl"), b"a").expect("file");
        fs::create_dir_all(dir.path().join("Minis/Taken")).expect("taken");
        fs::write(dir.path().join("Minis/b.stl"), b"b").expect("file");
        fs::write(dir.path().join("Minis/c.stl"), b"c").expect("file");
        assert!(FsFileOperations.rename(root, &path("Minis/Dragon"), &path("Minis/Taken")).is_err(), "empty folder");
        assert!(FsFileOperations.rename(root, &path("Minis/b.stl"), &path("Minis/c.stl")).is_err(), "file");
        assert_eq!(fs::read(dir.path().join("Minis/c.stl")).expect("read"), b"c");
        FsFileOperations.rename(root, &path("Minis/Dragon"), &path("Archive/Dragon")).expect("move into a new folder");
        assert!(dir.path().join("Archive/Dragon/a.stl").exists());
        assert!(!FsFileOperations.exists(root, &path("Minis/Dragon")).expect("check"));
    }

    #[test]
    fn case_only_renames_work_on_any_disk() {
        let dir = tempfile::tempdir().expect("temp dir");
        let root = dir.path().to_str().expect("utf-8");
        fs::create_dir_all(dir.path().join("dragon")).expect("dir");
        FsFileOperations.rename(root, &path("dragon"), &path("Dragon")).expect("case-only rename");
        let names: Vec<String> = fs::read_dir(dir.path())
            .expect("list")
            .map(|entry| entry.expect("entry").file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, ["Dragon"]);
    }

    #[test]
    fn new_files_never_replace_existing_ones() {
        let dir = tempfile::tempdir().expect("temp dir");
        let root = dir.path().to_str().expect("utf-8");
        FsFileOperations.create_file(root, &path("preview.png"), b"one").expect("create");
        assert_eq!(FsFileOperations.create_file(root, &path("preview.png"), b"two"), Err(FileError::AlreadyExists));
        assert_eq!(fs::read(dir.path().join("preview.png")).expect("read"), b"one");
    }

    #[test]
    fn an_interrupted_case_only_rename_is_finished() {
        let dir = tempfile::tempdir().expect("temp dir");
        let root = dir.path().to_str().expect("utf-8");
        fs::create_dir_all(dir.path().join(".Dragon.3dhoarddeck-renaming")).expect("left by a crash");
        assert!(FsFileOperations.recover(root, &path("Dragon")).expect("recover"));
        assert!(dir.path().join("Dragon").exists());
        assert!(!FsFileOperations.recover(root, &path("Dragon")).expect("nothing left"));
    }

    #[cfg(unix)]
    #[test]
    fn symlinks_cannot_lead_out_of_the_library() {
        let library = tempfile::tempdir().expect("library");
        let outside = tempfile::tempdir().expect("outside");
        let root = library.path().to_str().expect("utf-8");
        fs::create_dir_all(library.path().join("Dragon")).expect("dir");
        std::os::unix::fs::symlink(outside.path(), library.path().join("Elsewhere")).expect("link");
        assert!(FsFileOperations.rename(root, &path("Dragon"), &path("Elsewhere/Dragon")).is_err());
        assert!(library.path().join("Dragon").exists() && !outside.path().join("Dragon").exists());
    }
}
