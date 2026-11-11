use std::sync::Arc;

use hoarddeck_domain::{AccessMode, Library, LibraryId, LibrarySettings, MetadataStorage};

use crate::AppError;
use crate::ports::LibraryRepository;

/// Changes what the app may write into a library: nothing, metadata files only, or also renames and moves.
/// Switching metadata files on writes them for every model that already has names, tags, marks or notes, and the
/// folder rules into the `.3dhoarddeck` at the library root.
pub struct UpdateLibrarySettings {
    pub libraries: Arc<dyn LibraryRepository>,
    pub sync: Arc<crate::sidecar::SidecarSync>,
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
        // Switching metadata files on writes them for everything that has user data (the same as "Write metadata files").
        let written = if writes_sidecars(library.settings) && !before { self.sync.export(id)?.written } else { 0 };
        Ok((library, written))
    }
}

/// Whether the library keeps `.3dhoarddeck` files beside its models (and its folder rules at its root).
pub(crate) fn writes_sidecars(settings: LibrarySettings) -> bool {
    settings.metadata() == MetadataStorage::IndexAndSidecar && settings.access() != AccessMode::ReadOnly
}
