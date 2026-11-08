//! The steps rename, move, undo and the startup check share.

use std::fmt::Write;

use hoarddeck_domain::{AccessMode, Library, LibraryId, ModelKind, RelPath};

use crate::organize::sidecar_path;
use crate::ports::{FileError, FileJournal, FileOperations, JournalEntry, JournalStatus};
use crate::scan::{CompiledRules, ScanLibrary, ScanMode, ScanRules, model_stem};
use crate::{AppError, Refusal};

/// Cover images a single-file model may have beside it (`dragon.stl` → `dragon.jpg`).
const COVER_EXTENSIONS: [&str; 12] =
    ["jpg", "jpeg", "png", "webp", "gif", "bmp", "JPG", "JPEG", "PNG", "WEBP", "GIF", "BMP"];

pub(super) fn require_managed(library: &Library) -> Result<(), AppError> {
    if library.settings.access() == AccessMode::Managed {
        Ok(())
    } else {
        Err(Refusal::NotManaged(library.name.clone()).into())
    }
}

/// Names Windows and SMB shares can store: no `\ / : * ? " < > |`, control characters, trailing dot or space, or
/// reserved device names like `CON` or `COM1`.
pub(super) fn check_name(name: &str) -> Result<(), AppError> {
    let owned = || name.to_owned();
    if name.is_empty() || name == "." || name == ".." {
        return Err(Refusal::EmptyName(owned()).into());
    }
    if name.chars().any(|c| c.is_control() || "\\/:*?\"<>|".contains(c)) {
        return Err(Refusal::ForbiddenCharacter(owned()).into());
    }
    if name.ends_with(['.', ' ']) {
        return Err(Refusal::TrailingDotOrSpace(owned()).into());
    }
    let device = name.split('.').next().unwrap_or_default().trim_end().to_ascii_uppercase();
    let numbered = |prefix: &str| {
        device.strip_prefix(prefix).is_some_and(|n| n.len() == 1 && n.chars().all(|c| c.is_ascii_digit() && c != '0'))
    };
    if ["CON", "PRN", "AUX", "NUL"].contains(&device.as_str()) || numbered("COM") || numbered("LPT") {
        return Err(Refusal::ReservedName(owned()).into());
    }
    Ok(())
}

/// Nothing may be moved into a folder the scan skips: the model would silently disappear from the library.
pub(super) fn refuse_ignored(rules: &ScanRules, to: &RelPath) -> Result<(), AppError> {
    let compiled = CompiledRules::compile(rules)?;
    if compiled.is_ignored(to) || to.ancestors().iter().any(|dir| compiled.is_ignored_dir(dir)) {
        return Err(AppError::InvalidInput(format!("{to} is in a folder this library ignores")));
    }
    Ok(())
}

/// A move that did not happen. `restored` is false when even putting things back failed.
pub(super) struct MoveFailure {
    pub error: AppError,
    pub restored: bool,
}

/// Moves the model, then its companions; a failure puts everything back and removes the folders it created.
/// Returns the folders it created, deepest first.
pub(super) fn move_files(
    files: &dyn FileOperations,
    root: &str,
    from: &RelPath,
    to: &RelPath,
    kind: ModelKind,
) -> Result<Vec<RelPath>, MoveFailure> {
    let created: Vec<RelPath> = to
        .ancestors()
        .into_iter()
        .rev()
        .filter(|dir| !dir.is_root() && files.exists(root, dir).is_ok_and(|found| !found))
        .collect();
    let mut moved: Vec<(RelPath, RelPath)> = Vec::new();
    let pairs = std::iter::once((from.clone(), to.clone())).chain(companions(from, to, kind));
    for (index, (old, new)) in pairs.enumerate() {
        let step = match files.exists(root, &old) {
            Ok(false) if index > 0 => continue,
            Ok(_) => files.rename(root, &old, &new),
            Err(error) => Err(FileError::Failed(error)),
        };
        if let Err(error) = step {
            let mut restored = true;
            let mut unrestored = String::new();
            for (old, new) in moved.iter().rev() {
                if let Err(undo) = files.rename(root, new, old) {
                    restored = false;
                    let _ = write!(unrestored, "; {new} could not be put back: {undo}");
                }
            }
            if restored {
                remove_empty(files, root, &created);
            }
            let error = match error {
                FileError::AlreadyExists if restored => Refusal::AlreadyExists(new.to_string()).into(),
                error => AppError::InvalidInput(format!("could not move {old}: {error}{unrestored}")),
            };
            return Err(MoveFailure { error, restored });
        }
        moved.push((old, new));
    }
    Ok(created)
}

/// Removes folders a failed move created; one that is not empty (anymore) stays.
fn remove_empty(files: &dyn FileOperations, root: &str, dirs: &[RelPath]) {
    for dir in dirs {
        let _ = files.remove_empty_dir(root, dir);
    }
}

/// A single-file model's `.3dhoarddeck` and same-named cover images; folder models carry theirs inside.
fn companions(from: &RelPath, to: &RelPath, kind: ModelKind) -> Vec<(RelPath, RelPath)> {
    if kind != ModelKind::File {
        return Vec::new();
    }
    let mut pairs: Vec<(RelPath, RelPath)> = sidecar_path(from, kind).zip(sidecar_path(to, kind)).into_iter().collect();
    let (old_dir, new_dir) = (from.parent().unwrap_or_default(), to.parent().unwrap_or_default());
    let (old_stem, new_stem) = (model_stem(from.name()), model_stem(to.name()));
    // shortcut: probes fixed spellings instead of listing the folder; a `Dragon.Jpg` cover stays behind.
    for extension in COVER_EXTENSIONS {
        if let (Ok(old), Ok(new)) =
            (old_dir.join(&format!("{old_stem}.{extension}")), new_dir.join(&format!("{new_stem}.{extension}")))
            && old != new
        {
            pairs.push((old, new));
        }
    }
    pairs
}

/// Moves the files of `entry` forwards (`from` → `to`) or back, then completes the journal and the index in one
/// transaction. On failure disk and index stay as they were; a crash in between leaves the entry for
/// [`super::ReconcileJournal`].
pub(super) fn apply(
    files: &dyn FileOperations,
    journal: &dyn FileJournal,
    root: &str,
    entry: &JournalEntry,
    rules: &ScanRules,
    forward: bool,
) -> Result<(), AppError> {
    let (from, to, done, abandoned) = if forward {
        (&entry.from, &entry.to, JournalStatus::Done, JournalStatus::Failed)
    } else {
        (&entry.to, &entry.from, JournalStatus::Undone, JournalStatus::Done)
    };
    // Whenever disk and index might disagree, the entry stays unfinished for [`super::ReconcileJournal`].
    let created = match move_files(files, root, from, to, entry.kind) {
        Ok(created) => created,
        Err(failure) => {
            if failure.restored {
                journal.set_status(entry.id, abandoned)?;
            }
            return Err(failure.error);
        }
    };
    if let Err(error) = journal.finish(entry.id, entry.library, from, to, &rules.moved(from, to), done) {
        // The index did not change, so the files go back.
        if move_files(files, root, to, from, entry.kind).is_ok() {
            remove_empty(files, root, &created);
            journal.set_status(entry.id, abandoned)?;
        }
        return Err(error.into());
    }
    Ok(())
}

/// The change is done once files and index agree; a failed rescan only delays the grid, so it becomes a warning.
pub(super) fn rescan(scan: &ScanLibrary, library: LibraryId) -> Option<String> {
    scan.execute(library, ScanMode::Incremental).err().map(|error| {
        format!("The files were moved, but the library could not be rescanned ({error}). Rescan it in Settings.")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_windows_cannot_store_are_refused() {
        for bad in ["", "..", "a:b", "a?", "tab\there", "dot.", "space ", "CON", "con.stl", "COM1", "lpt9.txt", "NUL "]
        {
            assert!(check_name(bad).is_err(), "{bad:?} should be refused");
        }
        for good in ["Red Dragon", "COM10", "CONSOLE", "Dragon v1.2", "Ärger"] {
            assert!(check_name(good).is_ok(), "{good:?} should be fine");
        }
    }

    #[test]
    fn single_files_take_their_sidecar_and_cover_along() {
        let path = |p: &str| RelPath::new(p).expect("valid");
        let pairs = companions(&path("Minis/dragon.stl.txt"), &path("Minis/Red Dragon.stl.txt"), ModelKind::File);
        assert!(
            pairs.contains(&(path("Minis/dragon.stl.txt.3dhoarddeck"), path("Minis/Red Dragon.stl.txt.3dhoarddeck")))
        );
        assert!(pairs.contains(&(path("Minis/dragon.jpg"), path("Minis/Red Dragon.jpg"))));
        assert_eq!(companions(&path("Minis/Dragon"), &path("Archive/Dragon"), ModelKind::Folder), []);
    }

    /// Paths that exist; renaming anything into `fail_into` fails.
    struct FakeDisk {
        paths: std::sync::Mutex<std::collections::BTreeSet<String>>,
        fail_into: &'static str,
    }

    impl FileOperations for FakeDisk {
        fn rename(&self, _: &str, from: &RelPath, to: &RelPath) -> Result<(), FileError> {
            if to.as_str() == self.fail_into {
                return Err(FileError::Failed("refused".to_owned()));
            }
            let mut paths = self.paths.lock().expect("lock");
            for dir in to.ancestors().into_iter().filter(|dir| !dir.is_root()) {
                paths.insert(dir.to_string());
            }
            paths.remove(from.as_str());
            paths.insert(to.to_string());
            Ok(())
        }
        fn create_file(&self, _: &str, _: &RelPath, _: &[u8]) -> Result<(), FileError> {
            Err(FileError::Failed("not used here".to_owned()))
        }
        fn exists(&self, _: &str, path: &RelPath) -> Result<bool, String> {
            Ok(self.paths.lock().expect("lock").contains(path.as_str()))
        }
        fn remove_empty_dir(&self, _: &str, dir: &RelPath) -> Result<(), String> {
            let mut paths = self.paths.lock().expect("lock");
            if paths.iter().any(|other| other.starts_with(&format!("{dir}/"))) {
                return Err("not empty".to_owned());
            }
            paths.remove(dir.as_str());
            Ok(())
        }
        fn recover(&self, _: &str, _: &RelPath) -> Result<bool, String> {
            Ok(false)
        }
    }

    #[test]
    fn a_failed_move_puts_everything_back_and_leaves_no_new_folders() {
        let path = |p: &str| RelPath::new(p).expect("valid");
        let disk = FakeDisk {
            paths: std::sync::Mutex::new(
                ["Minis", "Minis/dragon.stl", "Minis/dragon.stl.3dhoarddeck"].map(String::from).into(),
            ),
            fail_into: "New/Deep/dragon.stl.3dhoarddeck",
        };
        let failure = move_files(&disk, "/", &path("Minis/dragon.stl"), &path("New/Deep/dragon.stl"), ModelKind::File)
            .expect_err("the sidecar move fails");
        assert!(failure.restored);
        let left: Vec<String> = disk.paths.lock().expect("lock").iter().cloned().collect();
        assert_eq!(left, ["Minis", "Minis/dragon.stl", "Minis/dragon.stl.3dhoarddeck"]);
    }
}
