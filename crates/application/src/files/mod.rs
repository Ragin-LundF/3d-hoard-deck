//! Renaming and moving models on disk. Only for libraries in Managed mode; every change is journaled and undoable.

mod disk;
mod move_model;
mod reconcile;
mod undo;

pub use move_model::{MoveModel, Target};
pub use reconcile::ReconcileJournal;
pub use undo::UndoFileOperation;

/// A finished rename, move or undo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileChange {
    pub journal: i64,
    /// Set when the files moved but something after it did not work, such as the rescan.
    pub warning: Option<String>,
}
