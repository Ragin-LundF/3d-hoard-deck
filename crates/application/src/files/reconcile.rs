use std::sync::Arc;

use crate::AppError;
use crate::ports::{FileJournal, FileOperations, JournalStatus, LibraryRepository};

/// After a crash mid-change, settles unfinished journal entries by looking at which path exists on disk.
pub struct ReconcileJournal {
    pub libraries: Arc<dyn LibraryRepository>,
    pub files: Arc<dyn FileOperations>,
    pub journal: Arc<dyn FileJournal>,
}

impl ReconcileJournal {
    /// Entries of offline libraries, or where both or neither path exist, stay for the next start.
    /// # Errors
    /// Fails when the index cannot be read or written.
    pub fn execute(&self) -> Result<(), AppError> {
        for entry in self.journal.unfinished()? {
            let Some(library) = self.libraries.get(entry.library)? else { continue };
            if entry.extract.is_some() {
                super::extract_archive::reconcile_extraction(
                    self.files.as_ref(),
                    self.journal.as_ref(),
                    &library.root,
                    &entry,
                )?;
                continue;
            }
            let (old, new, done, abandoned) = match entry.status {
                JournalStatus::Pending => (&entry.from, &entry.to, JournalStatus::Done, JournalStatus::Failed),
                JournalStatus::Undoing => (&entry.to, &entry.from, JournalStatus::Undone, JournalStatus::Done),
                _ => continue,
            };
            // A crash in the middle of a case-only rename leaves the temporary name; finish that first.
            if self.files.recover(&library.root, new).is_err() {
                continue;
            }
            let exists = |path| self.files.exists(&library.root, path).ok();
            match (exists(old), exists(new)) {
                (Some(false), Some(true)) => {
                    self.journal.finish(entry.id, library.id, old, new, done)?;
                }
                (Some(true), Some(false)) => self.journal.set_status(entry.id, abandoned)?,
                _ => {}
            }
        }
        Ok(())
    }
}
