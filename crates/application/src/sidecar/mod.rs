//! `.3dhoarddeck` files that hold everything the user decided, so the index can be rebuilt from the library alone:
//! [`SidecarSync`] keeps them current and exports them, [`RestoreFromSidecars`] fills an index from them.

mod format;
mod restore;
mod sync;

pub use format::{
    BackupFile, EntryFile, FORMAT_VERSION, FolderFile, LibraryFile, RootFile, SavedCollection, SavedScanRules,
    SavedTagGroup,
};
pub use restore::{RestoreFromSidecars, RestoreMode, RestoreReport, RestoreSource, SavedSummary};
pub use sync::{ExportReport, SidecarStatus, SidecarSync, entry_path};
