use std::sync::Arc;

use hoarddeck_domain::{FileKind, ModelId, ModelMeta};

use crate::ports::{LibraryRepository, MetaStore, ModelRepository, SearchIndex};
use crate::scan::LibraryLocks;
use crate::{AppError, ModelDetail};

/// Saves what the user set on a model, then its `.3dhoarddeck` file where the library keeps metadata files.
pub struct UpdateModelMeta {
    pub libraries: Arc<dyn LibraryRepository>,
    pub models: Arc<dyn ModelRepository>,
    pub meta: Arc<dyn MetaStore>,
    pub sync: Arc<crate::sidecar::SidecarSync>,
    pub search: Arc<dyn SearchIndex>,
    pub locks: Arc<LibraryLocks>,
}

impl UpdateModelMeta {
    /// # Errors
    /// Fails for an unknown model, invalid input, or when the sidecar file or the index cannot be written.
    pub fn execute(&self, id: ModelId, meta: ModelMeta) -> Result<ModelDetail, AppError> {
        let meta = meta.normalized()?;
        let detail = self.models.get(id)?.ok_or_else(|| AppError::InvalidInput("unknown model".to_owned()))?;
        if let Some(cover) = &meta.cover
            && !detail.files.iter().any(|file| &file.path == cover && file.kind == FileKind::Image)
        {
            return Err(AppError::InvalidInput("the cover must be one of the model's images".to_owned()));
        }
        let library =
            self.libraries.get(detail.summary.library)?.ok_or(AppError::LibraryNotFound(detail.summary.library))?;
        self.meta.save_meta(library.id, &detail.summary.path, &meta)?;
        // A metadata file that cannot be written now waits in a queue; the change in the index stands.
        self.sync.paths_changed(library.id, std::slice::from_ref(&detail.summary.path));
        let _sync = self.locks.search_sync();
        if let Some(document) = self.models.search_document(id)? {
            self.search.update_documents(&[document])?;
        }
        self.models.get(id)?.ok_or_else(|| AppError::InvalidInput("unknown model".to_owned()))
    }
}
