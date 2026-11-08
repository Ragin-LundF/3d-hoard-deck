use std::sync::Arc;

use hoarddeck_domain::{AccessMode, Library, LibraryId, LibrarySettings, MetadataStorage};

use crate::AppError;
use crate::organize::sidecar_path;
use crate::ports::{LibraryRepository, MetaStore, ModelRepository, SidecarFiles};

/// Changes what the app may write into a library: nothing, metadata files only, or also renames and moves.
/// Switching metadata files on writes them for every model that already has names, tags, marks or notes, and the
/// folder rules into the `.3dhoarddeck` at the library root.
pub struct UpdateLibrarySettings {
    pub libraries: Arc<dyn LibraryRepository>,
    pub models: Arc<dyn ModelRepository>,
    pub meta: Arc<dyn MetaStore>,
    pub sidecars: Arc<dyn SidecarFiles>,
}

impl UpdateLibrarySettings {
    /// Returns the library and how many metadata files were written.
    /// # Errors
    /// Fails for an unknown library, sidecar files without write access, or when the index or a file cannot be
    /// written.
    pub fn execute(
        &self,
        id: LibraryId,
        access: AccessMode,
        metadata: MetadataStorage,
    ) -> Result<(Library, usize), AppError> {
        let mut library = self.libraries.get(id)?.ok_or(AppError::LibraryNotFound(id))?;
        let before = writes_sidecars(library.settings);
        library.settings = LibrarySettings::new(access, metadata)?;
        self.libraries.save_settings(id, library.settings)?;
        let written = if writes_sidecars(library.settings) && !before { self.backfill(&library)? } else { 0 };
        Ok((library, written))
    }

    fn backfill(&self, library: &Library) -> Result<usize, AppError> {
        let rules = self.libraries.scan_rules(library.id)?.folder_rules;
        if !rules.is_empty() {
            self.sidecars
                .write_folder_rules(&library.root, &rules)
                .map_err(|error| AppError::InvalidInput(format!("metadata file: {error}")))?;
        }
        let mut written = 0;
        for document in self.models.search_documents(library.id)? {
            let meta = self.meta.meta(library.id, &document.path)?;
            if meta.is_empty() {
                continue;
            }
            if let Some(file) = sidecar_path(&document.path, document.kind) {
                self.sidecars
                    .write(&library.root, &file, &meta)
                    .map_err(|error| AppError::InvalidInput(format!("metadata file: {error}")))?;
                written += 1;
            }
        }
        Ok(written)
    }
}

/// Whether the library keeps `.3dhoarddeck` files beside its models (and its folder rules at its root).
pub(crate) fn writes_sidecars(settings: LibrarySettings) -> bool {
    settings.metadata() == MetadataStorage::IndexAndSidecar && settings.access() != AccessMode::ReadOnly
}
