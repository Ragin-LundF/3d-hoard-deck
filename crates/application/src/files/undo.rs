use std::sync::Arc;

use super::FileChange;
use super::disk::{apply, require_managed, rescan};
use crate::AppError;
use crate::ports::{FileJournal, FileOperations, JournalStatus, LibraryRepository, SidecarFiles};
use crate::scan::{LibraryLocks, ScanLibrary, save_rules_file};

/// Reverses a finished rename or move, when nothing took the old place meanwhile.
pub struct UndoFileOperation {
    pub libraries: Arc<dyn LibraryRepository>,
    pub files: Arc<dyn FileOperations>,
    pub journal: Arc<dyn FileJournal>,
    pub locks: Arc<LibraryLocks>,
    pub scan: Arc<ScanLibrary>,
    pub sidecars: Arc<dyn SidecarFiles>,
}

impl UndoFileOperation {
    /// # Errors
    /// Fails for an unknown entry or one that is not done, a library not in Managed mode, an occupied old place, or
    /// when the file system refuses. Disk and index are unchanged then.
    pub fn execute(&self, id: i64) -> Result<FileChange, AppError> {
        let entry =
            self.journal.entry(id)?.ok_or_else(|| AppError::InvalidInput("unknown journal entry".to_owned()))?;
        if entry.extract.is_some() {
            return Err(AppError::InvalidInput("an extraction is undone with \"Discard extracted files\"".to_owned()));
        }
        if entry.status != JournalStatus::Done {
            return Err(AppError::InvalidInput("only finished changes can be undone, and only once".to_owned()));
        }
        let library = self.libraries.get(entry.library)?.ok_or(AppError::LibraryNotFound(entry.library))?;
        require_managed(&library)?;
        {
            let _busy = self.locks.lock(library.id)?;
            self.journal.set_status(id, JournalStatus::Undoing)?;
            apply(self.files.as_ref(), self.journal.as_ref(), &library.root, &entry, false)?;
            save_rules_file(self.sidecars.as_ref(), &library, &self.libraries.scan_rules(library.id)?);
        }
        Ok(FileChange { journal: id, warning: rescan(&self.scan, library.id) })
    }
}
