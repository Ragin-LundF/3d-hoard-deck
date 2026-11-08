use std::sync::Arc;

use std::time::{SystemTime, UNIX_EPOCH};

use hoarddeck_domain::{AccessMode, Entry, EntryKind, FileKind, ModelId, RelPath};

use crate::ports::{FileError, FileOperations, LibraryRepository, ModelRepository};
use crate::scan::{LibraryLocks, ScanLibrary, model_stem};
use crate::{AppError, Refusal};

/// Larger "screenshots" are not what the viewer makes; refusing them keeps a bad request from filling the disk.
const MAX_BYTES: usize = 32 * 1024 * 1024;
const PNG_SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1a, b'\n'];
/// Tried in turn when the names before are taken.
const MAX_NAMES: usize = 50;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Screenshot {
    /// Where the image was written, relative to the library.
    pub path: RelPath,
    /// Set when the image was written but the index could not be updated.
    pub warning: Option<String>,
}

/// Writes a picture of the 3D preview next to the mesh it shows, as `preview_<name>.png`, so models without images
/// get a cover. Never replaces a file; only for libraries that may write files. Only that folder is updated in the
/// index; the library is not walked.
pub struct SaveScreenshot {
    pub libraries: Arc<dyn LibraryRepository>,
    pub models: Arc<dyn ModelRepository>,
    pub files: Arc<dyn FileOperations>,
    pub locks: Arc<LibraryLocks>,
    pub scan: Arc<ScanLibrary>,
}

impl SaveScreenshot {
    /// # Errors
    /// Fails for an unknown model, a file that is not part of it, a read-only library, data that is not a PNG, or when
    /// the file cannot be written.
    pub fn execute(&self, id: ModelId, file: &RelPath, png: &[u8]) -> Result<Screenshot, AppError> {
        if png.len() > MAX_BYTES || !png.starts_with(&PNG_SIGNATURE) {
            return Err(AppError::InvalidInput("the screenshot is not a PNG image".to_owned()));
        }
        let detail = self.models.get(id)?.ok_or_else(|| AppError::InvalidInput("unknown model".to_owned()))?;
        let library =
            self.libraries.get(detail.summary.library)?.ok_or(AppError::LibraryNotFound(detail.summary.library))?;
        if library.settings.access() == AccessMode::ReadOnly {
            return Err(Refusal::ReadOnly(library.name).into());
        }
        if !detail.files.iter().any(|candidate| &candidate.path == file) {
            return Err(AppError::InvalidInput(format!("{file} is not a file of this model")));
        }
        let path = {
            let _busy = self.locks.lock(library.id)?;
            let mut written = None;
            for candidate in names(file) {
                match self.files.create_file(&library.root, &candidate, png) {
                    Ok(()) => {
                        written = Some(candidate);
                        break;
                    }
                    Err(FileError::AlreadyExists) => {}
                    Err(FileError::Failed(reason)) => return Err(AppError::InvalidInput(reason)),
                }
            }
            written.ok_or_else(|| AppError::InvalidInput("no free name for the screenshot".to_owned()))?
        };
        let entry = Entry {
            path: path.clone(),
            kind: EntryKind::File(FileKind::Image),
            size: png.len() as u64,
            mtime: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(0)),
        };
        let warning =
            self.scan.add_files(library.id, &path.parent().unwrap_or_default(), vec![entry]).err().map(|error| {
                format!("The screenshot was saved, but the index could not be updated ({error}). Rescan the library.")
            });
        Ok(Screenshot { path, warning })
    }
}

/// `Minis/dragon.stl` → `Minis/preview_dragon.png`, then `preview_dragon 2.png`…; found as the file's cover by name,
/// and `preview` makes it the cover of a folder model too.
fn names(file: &RelPath) -> Vec<RelPath> {
    let folder = file.parent().unwrap_or_default();
    let base = format!("preview_{}", model_stem(file.name()));
    std::iter::once(format!("{base}.png"))
        .chain((2..=MAX_NAMES).map(|number| format!("{base} {number}.png")))
        .filter_map(|name| folder.join(&name).ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn path(value: &str) -> RelPath {
        RelPath::new(value).expect("valid")
    }

    #[test]
    fn screenshots_are_named_after_their_mesh() {
        let names = names(&path("Minis/Dragon/Supported/dragon.stl.txt"));
        assert_eq!(
            &names[..2],
            [path("Minis/Dragon/Supported/preview_dragon.png"), path("Minis/Dragon/Supported/preview_dragon 2.png")]
        );
    }
}
