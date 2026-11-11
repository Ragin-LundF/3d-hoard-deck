use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use hoarddeck_domain::{AccessMode, Entry, EntryKind, FileKind, LibraryId, RelPath, Sniff, classify_by_name};

use crate::AppError;
use crate::ports::{FileOperations, FolderCoverStore, LibraryRepository};
use crate::scan::ScanLibrary;

/// Lets the user choose the picture a folder card shows, like a movie library's poster. The choice is kept in the
/// index (so it works in read-only libraries); where the library allows writing, the picture is also copied into the
/// folder as `folder.<ext>`, so it survives a rebuild and other programs see it too.
pub struct SetFolderCover {
    pub libraries: Arc<dyn LibraryRepository>,
    pub covers: Arc<dyn FolderCoverStore>,
    pub files: Arc<dyn FileOperations>,
    pub scan: Arc<ScanLibrary>,
    pub sync: Arc<crate::sidecar::SidecarSync>,
}

impl SetFolderCover {
    /// `None` goes back to the detected cover. Returns the copy's path when one was written.
    /// # Errors
    /// Fails for an unknown library, a root folder, a file that is not an image, or when the index cannot be written.
    /// A copy that cannot be written only means the choice stays in the index.
    pub fn execute(
        &self,
        id: LibraryId,
        folder: &RelPath,
        image: Option<&RelPath>,
    ) -> Result<Option<RelPath>, AppError> {
        if folder.is_root() {
            return Err(AppError::InvalidInput("the library itself has no cover".to_owned()));
        }
        let library = self.libraries.get(id)?.ok_or(AppError::LibraryNotFound(id))?;
        let Some(image) = image else {
            self.covers.set_folder_cover(id, folder, None)?;
            self.sync.paths_changed(id, std::slice::from_ref(folder));
            return Ok(None);
        };
        if classify_by_name(image.name()) != Sniff::Known(FileKind::Image) {
            return Err(AppError::InvalidInput(format!("{image} is not a picture")));
        }
        let copy = if library.settings.access() != AccessMode::ReadOnly && image.parent().as_ref() != Some(folder) {
            self.copy_into(&library.root, id, folder, image)
        } else {
            None
        };
        self.covers.set_folder_cover(id, folder, Some(copy.as_ref().unwrap_or(image)))?;
        self.sync.paths_changed(id, std::slice::from_ref(folder));
        Ok(copy)
    }

    /// `folder.png` (or `.jpg` …) in the folder; an existing file of that name is left alone and not used.
    fn copy_into(&self, root: &str, id: LibraryId, folder: &RelPath, image: &RelPath) -> Option<RelPath> {
        let extension = image.name().rsplit_once('.').map_or("png", |(_, extension)| extension).to_ascii_lowercase();
        let target = folder.join(&format!("folder.{extension}")).ok()?;
        let Ok(size) = self.files.copy_file(root, image, &target) else { return None };
        let mtime =
            SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(0));
        let entry = Entry { path: target.clone(), kind: EntryKind::File(FileKind::Image), size, mtime };
        // The copy is in the folder either way; if the index misses it now, the next scan finds it.
        let _ = self.scan.add_files(id, folder, vec![entry]);
        Some(target)
    }
}
