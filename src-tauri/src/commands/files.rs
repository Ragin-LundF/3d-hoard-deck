//! Renaming and moving models on disk (Managed libraries only), with undo.

use std::sync::Arc;

use hoarddeck_application::AppError;
use hoarddeck_application::files::Target;
use hoarddeck_domain::ModelId;

use super::AppState;
use crate::dto::{FileChangeDto, IpcError, JournalEntryDto, rel_path};

/// Renames the model on disk (`rename`) or moves it into `folder`; exactly one must be given.
/// Returns the journal id for undo, and a warning when the files moved but the rescan did not work.
#[tauri::command]
#[specta::specta]
pub async fn move_model(
    app: AppState<'_>,
    id: u32,
    rename: Option<String>,
    folder: Option<String>,
) -> Result<FileChangeDto, IpcError> {
    let target = match (rename, folder) {
        (Some(name), None) => Target::Rename(name),
        (None, Some(folder)) => Target::MoveTo(rel_path(&folder)?),
        _ => return Err(invalid("give either a new name or a target folder")),
    };
    let app = Arc::clone(&app);
    let change = tauri::async_runtime::spawn_blocking(move || app.move_model.execute(ModelId(i64::from(id)), &target))
        .await
        .map_err(|error| internal(&error))??;
    Ok(change.into())
}

#[tauri::command]
#[specta::specta]
pub async fn undo_file_operation(app: AppState<'_>, id: u32) -> Result<FileChangeDto, IpcError> {
    let app = Arc::clone(&app);
    let change = tauri::async_runtime::spawn_blocking(move || app.undo_file_operation.execute(i64::from(id)))
        .await
        .map_err(|error| internal(&error))??;
    Ok(change.into())
}

/// The latest renames and moves across all libraries, newest first.
#[tauri::command]
#[specta::specta]
pub async fn file_journal(app: AppState<'_>, limit: u32) -> Result<Vec<JournalEntryDto>, IpcError> {
    let entries = app.file_journal.recent(usize::try_from(limit).unwrap_or(50)).map_err(AppError::from)?;
    Ok(entries.into_iter().map(Into::into).collect())
}

fn invalid(message: &str) -> IpcError {
    IpcError { code: "invalid_input".to_owned(), message: message.to_owned(), details: None }
}

fn internal(error: &tauri::Error) -> IpcError {
    IpcError { code: "internal".to_owned(), message: error.to_string(), details: None }
}
