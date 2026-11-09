use std::collections::HashMap;
use std::sync::Arc;

use hoarddeck_domain::{AccessMode, Entry, FileKind, LibraryId, ModelId, RelPath};

use super::SaveScreenshot;
use crate::ports::{LibraryRepository, ModelRepository, RenderedCache, RenderedStore};
use crate::{AppError, ModelDetail};

/// A rendered picture larger than this is no render of ours.
const MAX_BYTES: usize = 16 * 1024 * 1024;
const PNG_SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1a, b'\n'];

/// What keeping rendered previews did.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Kept {
    /// Written into the library as `preview_<file>.png`.
    pub written: usize,
    /// Kept in the index (read-only libraries).
    pub kept: usize,
}

/// Previews the app renders for models without any picture. Renders wait in the app's own folder until the user keeps
/// them: written as `preview_<main file>.png` where the library allows it, else kept in the index. Skipped models are
/// not rendered again.
pub struct RenderedPreviews {
    pub libraries: Arc<dyn LibraryRepository>,
    pub models: Arc<dyn ModelRepository>,
    pub store: Arc<dyn RenderedStore>,
    pub cache: Arc<dyn RenderedCache>,
    pub screenshot: Arc<SaveScreenshot>,
}

impl RenderedPreviews {
    /// The model's main mesh to render (the first STL, OBJ or 3MF in file order) and its size.
    /// # Errors
    /// Fails when the index cannot be read.
    pub fn source(&self, id: ModelId) -> Result<Option<(RelPath, u64)>, AppError> {
        Ok(self.models.get(id)?.and_then(|detail| main_file(&detail).map(|file| (file.0.clone(), file.1))))
    }

    /// Stores a render in the app's folder; it shows with a "rendered" mark until kept.
    /// # Errors
    /// Fails for data that is not a PNG, an unknown model, or when the picture or the index cannot be written.
    pub fn store(&self, id: ModelId, png: &[u8]) -> Result<(), AppError> {
        if png.len() > MAX_BYTES || !png.starts_with(&PNG_SIGNATURE) {
            return Err(AppError::InvalidInput("the rendered preview is not a PNG image".to_owned()));
        }
        let detail = self.detail(id)?;
        self.cache.save_rendered(id, png).map_err(AppError::InvalidInput)?;
        Ok(self.store.set_rendered(detail.summary.library, &[detail.summary.path], Some("cached"))?)
    }

    /// Keeps renders: into the model folder where the library may write files, else in the index.
    /// # Errors
    /// Fails for an unknown model, a missing render, or when a file or the index cannot be written.
    pub fn keep(&self, ids: &[ModelId]) -> Result<Kept, AppError> {
        let mut kept = Kept::default();
        // All files first, then one index update per library: a regroup per picture would make "Save all" slow.
        let mut written: HashMap<LibraryId, HashMap<RelPath, Vec<Entry>>> = HashMap::new();
        // On a failure the files written so far still go into the index.
        let failed = ids.iter().try_for_each(|id| self.keep_one(*id, &mut kept, &mut written)).err();
        for (library, folders) in written {
            self.screenshot.scan.add_files_in(library, folders.into_iter().collect()).map_err(|error| {
                AppError::InvalidInput(format!(
                    "The previews were saved, but the index could not be updated ({error}). Rescan the library."
                ))
            })?;
        }
        failed.map_or(Ok(kept), Err)
    }

    fn keep_one(
        &self,
        id: ModelId,
        kept: &mut Kept,
        written: &mut HashMap<LibraryId, HashMap<RelPath, Vec<Entry>>>,
    ) -> Result<(), AppError> {
        let detail = self.detail(id)?;
        let (library, path) = (detail.summary.library, detail.summary.path.clone());
        let found = self.libraries.get(library)?.ok_or(AppError::LibraryNotFound(library))?;
        if found.settings.access() == AccessMode::ReadOnly {
            self.store.set_rendered(library, &[path], Some("kept"))?;
            kept.kept += 1;
            return Ok(());
        }
        let file =
            main_file(&detail).ok_or_else(|| AppError::InvalidInput("the model has no mesh".to_owned()))?.0.clone();
        let png = self.cache.rendered(id).map_err(AppError::InvalidInput)?;
        let (library, entry) = self.screenshot.write(id, &file, &png)?;
        let folder = entry.path.parent().unwrap_or_default();
        written.entry(library).or_default().entry(folder).or_default().push(entry);
        kept.written += 1;
        self.store.set_rendered(library, &[path], None)?;
        Ok(())
    }

    /// Marks renders as not wanted; those models are not rendered again by themselves.
    /// # Errors
    /// Fails for an unknown model or when the index cannot be written.
    pub fn skip(&self, ids: &[ModelId]) -> Result<(), AppError> {
        self.set(ids, Some("skipped"))
    }

    /// Back to waiting (undo of skip, or of keeping in a read-only library).
    /// # Errors
    /// Fails for an unknown model or when the index cannot be written.
    pub fn reset(&self, ids: &[ModelId], state: Option<&str>) -> Result<(), AppError> {
        self.set(ids, state)
    }

    fn set(&self, ids: &[ModelId], state: Option<&str>) -> Result<(), AppError> {
        for summary in self.models.summaries(ids)? {
            self.store.set_rendered(summary.library, &[summary.path], state)?;
        }
        Ok(())
    }

    fn detail(&self, id: ModelId) -> Result<ModelDetail, AppError> {
        self.models.get(id)?.ok_or_else(|| AppError::InvalidInput("unknown model".to_owned()))
    }
}

fn main_file(detail: &ModelDetail) -> Option<(&RelPath, u64)> {
    detail.files.iter().find(|file| matches!(file.kind, FileKind::Mesh(_))).map(|file| (&file.path, file.size))
}
