use std::sync::Arc;

use hoarddeck_domain::{LibraryId, RelPath};

use crate::AppError;
use crate::ports::{ArchiveEntry, ArchiveReader, LibraryRepository};

/// The files inside a ZIP archive of a library. Reads only.
pub struct ListArchive {
    pub libraries: Arc<dyn LibraryRepository>,
    pub archives: Arc<dyn ArchiveReader>,
}

impl ListArchive {
    /// # Errors
    /// Fails for an unknown library, an archive that is not a ZIP or cannot be read (offline, damaged).
    pub fn execute(&self, library: LibraryId, archive: &RelPath) -> Result<Vec<ArchiveEntry>, AppError> {
        if !archive.name().to_ascii_lowercase().ends_with(".zip") {
            return Err(AppError::InvalidInput("only ZIP archives can be opened here".to_owned()));
        }
        let library = self.libraries.get(library)?.ok_or(AppError::LibraryNotFound(library))?;
        self.archives.list(&library.root, archive).map_err(AppError::InvalidInput)
    }
}
