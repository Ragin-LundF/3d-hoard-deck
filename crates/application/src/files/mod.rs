//! Changes on disk: renaming and moving models (Managed libraries only, journaled and undoable) and screenshots of
//! the 3D preview (any library that may write files).

mod disk;
mod move_model;
mod reconcile;
mod rendered;
mod save_screenshot;
mod undo;

pub use move_model::{MoveModel, Target};
pub use reconcile::ReconcileJournal;
pub use rendered::{Kept, RenderedPreviews};
pub use save_screenshot::{SaveScreenshot, Screenshot};
pub use undo::UndoFileOperation;

/// A finished rename, move or undo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileChange {
    pub journal: i64,
    /// Set when the files moved but something after it did not work, such as the rescan.
    pub warning: Option<String>,
}
