//! Where the index lives.

use tauri::AppHandle;

use super::AppState;
use crate::dto::{ExistingIndexDto, IpcError};

/// The folder the index was opened from.
#[tauri::command]
#[specta::specta]
pub async fn index_location(app: AppState<'_>) -> Result<String, IpcError> {
    Ok(app.index_dir.display().to_string())
}

/// Moves the index to `folder` and restarts at once, so nothing is written to the old copy afterwards.
/// `existing` decides what happens when `folder` already holds an index: `ask` fails with `index_exists`.
#[tauri::command]
#[specta::specta]
pub async fn move_index_location(
    app: AppState<'_>,
    handle: AppHandle,
    folder: String,
    existing: ExistingIndexDto,
) -> Result<(), IpcError> {
    let target = std::fs::canonicalize(&folder).unwrap_or_else(|_| std::path::PathBuf::from(&folder));
    app.move_index_location.execute(&app.index_dir, &target, existing.into())?;
    handle.restart()
}
