use std::sync::Arc;

use hoarddeck_domain::{ModelId, ModelKind, RelPath};

use super::FileChange;
use super::disk::{apply, check_name, refuse_ignored, require_managed, rescan};
use crate::AppError;
use crate::ports::{FileJournal, FileOperations, JournalEntry, JournalStatus, LibraryRepository, ModelRepository};
use crate::scan::{LibraryLocks, ScanLibrary, model_stem};

/// Where a model should go.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    /// Same folder, new name (for a single-file model the extension is kept).
    Rename(String),
    /// Same name, into another folder of the library.
    MoveTo(RelPath),
}

/// Renames or moves a model's folder (or file) on disk, carries its user data and folder rules along, journals it
/// and rescans.
pub struct MoveModel {
    pub libraries: Arc<dyn LibraryRepository>,
    pub models: Arc<dyn ModelRepository>,
    pub files: Arc<dyn FileOperations>,
    pub journal: Arc<dyn FileJournal>,
    pub locks: Arc<LibraryLocks>,
    pub scan: Arc<ScanLibrary>,
}

impl MoveModel {
    /// # Errors
    /// Fails unless the library is in Managed mode, for an invalid name or folder, an existing or ignored target, a
    /// busy library, or when the file system refuses. Disk and index are unchanged then.
    pub fn execute(&self, id: ModelId, target: &Target) -> Result<FileChange, AppError> {
        let detail = self.models.get(id)?.ok_or_else(|| AppError::InvalidInput("unknown model".to_owned()))?;
        let library =
            self.libraries.get(detail.summary.library)?.ok_or(AppError::LibraryNotFound(detail.summary.library))?;
        require_managed(&library)?;
        let (from, kind) = (detail.summary.path.clone(), detail.summary.kind);
        let to = destination(&from, kind, target)?;
        if to == from {
            return Err(AppError::InvalidInput("the model is already there".to_owned()));
        }
        let journal = {
            let _busy = self.locks.lock(library.id)?;
            let rules = self.libraries.scan_rules(library.id)?;
            refuse_ignored(&rules, &to)?;
            let id = self.journal.begin(library.id, &from, &to, kind)?;
            let entry =
                JournalEntry { id, library: library.id, from, to, kind, done_at: 0, status: JournalStatus::Pending };
            apply(self.files.as_ref(), self.journal.as_ref(), &library.root, &entry, &rules, true)?;
            id
        };
        Ok(FileChange { journal, warning: rescan(&self.scan, library.id) })
    }
}

fn destination(from: &RelPath, kind: ModelKind, target: &Target) -> Result<RelPath, AppError> {
    match target {
        Target::Rename(name) => {
            let name = name.trim();
            let name = match kind {
                // `.stl.txt` and `.stl.xxx` stay whole, so a disguised STL keeps its disguise.
                ModelKind::File => {
                    let original = from.name();
                    let extension = &original[model_stem(original).len()..];
                    if name.to_lowercase().ends_with(&extension.to_lowercase()) {
                        name.to_owned()
                    } else {
                        format!("{name}{extension}")
                    }
                }
                ModelKind::Folder => name.to_owned(),
            };
            check_name(&name)?;
            Ok(from.parent().unwrap_or_default().join(&name)?)
        }
        Target::MoveTo(folder) => {
            if folder.is_within(from) {
                return Err(AppError::InvalidInput("a folder cannot move into itself".to_owned()));
            }
            for segment in folder.segments() {
                check_name(segment)?;
            }
            Ok(folder.join(from.name())?)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn path(value: &str) -> RelPath {
        RelPath::new(value).expect("valid")
    }

    #[test]
    fn destinations_keep_extensions_and_refuse_nonsense() {
        let folder = path("Minis/Dragon");
        let rename = |from: &RelPath, kind, name: &str| destination(from, kind, &Target::Rename(name.into()));
        assert_eq!(rename(&folder, ModelKind::Folder, "Red Dragon").expect("ok"), path("Minis/Red Dragon"));
        let file = path("Makerworld/13661.3mf");
        assert_eq!(rename(&file, ModelKind::File, "Gear box").expect("ok"), path("Makerworld/Gear box.3mf"));
        assert_eq!(rename(&file, ModelKind::File, "Gear box.3MF").expect("ok"), path("Makerworld/Gear box.3MF"));
        let disguised = path("Minis/dragon.stl.txt");
        assert_eq!(rename(&disguised, ModelKind::File, "Red").expect("ok"), path("Minis/Red.stl.txt"));
        assert!(rename(&folder, ModelKind::Folder, "a/b").is_err());
        assert!(rename(&folder, ModelKind::Folder, "..").is_err());
        assert!(rename(&folder, ModelKind::Folder, "Dragon.").is_err());
        assert_eq!(
            destination(&folder, ModelKind::Folder, &Target::MoveTo(path("Archive"))).expect("ok"),
            path("Archive/Dragon")
        );
        assert!(destination(&folder, ModelKind::Folder, &Target::MoveTo(path("Minis/Dragon/Parts"))).is_err());
        assert!(destination(&folder, ModelKind::Folder, &Target::MoveTo(path("Old/AUX"))).is_err());
    }
}
