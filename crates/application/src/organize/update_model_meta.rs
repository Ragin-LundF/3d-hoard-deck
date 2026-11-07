use std::sync::Arc;

use hoarddeck_domain::{AccessMode, FileKind, MetadataStorage, ModelId, ModelMeta};

use super::sidecar_path;
use crate::ports::{LibraryRepository, MetaStore, ModelRepository, SearchIndex, SidecarFiles};
use crate::scan::LibraryLocks;
use crate::{AppError, ModelDetail};

/// Saves what the user set on a model. With sidecar files allowed for the library, the `.3dhoarddeck` file is
/// written first, so the index never claims more than the folder holds.
pub struct UpdateModelMeta {
    pub libraries: Arc<dyn LibraryRepository>,
    pub models: Arc<dyn ModelRepository>,
    pub meta: Arc<dyn MetaStore>,
    pub sidecars: Arc<dyn SidecarFiles>,
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
        let writes_sidecars = library.settings.metadata() == MetadataStorage::IndexAndSidecar
            && library.settings.access() != AccessMode::ReadOnly;
        if writes_sidecars && let Some(file) = sidecar_path(&detail.summary.path, detail.summary.kind) {
            self.sidecars
                .write(&library.root, &file, &meta)
                .map_err(|error| AppError::InvalidInput(format!("metadata file: {error}")))?;
        }
        self.meta.save_meta(library.id, &detail.summary.path, &meta)?;
        let _sync = self.locks.search_sync();
        if let Some(document) = self.models.search_document(id)? {
            self.search.update_documents(&[document])?;
        }
        self.models.get(id)?.ok_or_else(|| AppError::InvalidInput("unknown model".to_owned()))
    }
}
