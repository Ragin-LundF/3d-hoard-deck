//! Renaming and moving models on disk (Managed libraries only), with undo.

use std::sync::Arc;

use hoarddeck_application::AppError;
use hoarddeck_application::files::Target;
use hoarddeck_domain::ModelId;

use super::AppState;
use crate::dto::{FileChangeDto, IpcError, JournalEntryDto, ScreenshotDto, rel_path};

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

/// Writes a PNG of the 3D preview of `file` next to it as `preview_<name>.png` (libraries that may write files). The bytes travel as a
/// JSON number array, fine for a screenshot of a few MB.
#[tauri::command]
#[specta::specta]
pub async fn save_screenshot(
    app: AppState<'_>,
    id: u32,
    file: String,
    png: Vec<u8>,
) -> Result<ScreenshotDto, IpcError> {
    let app = Arc::clone(&app);
    let file = rel_path(&file)?;
    let shot =
        tauri::async_runtime::spawn_blocking(move || app.save_screenshot.execute(ModelId(i64::from(id)), &file, &png))
            .await
            .map_err(|error| internal(&error))??;
    Ok(ScreenshotDto { path: shot.path.to_string(), warning: shot.warning })
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

/// The main mesh of a model to render a preview from, with its size in bytes.
#[derive(Debug, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RenderSourceDto {
    pub path: String,
    pub size: f64,
}

#[tauri::command]
#[specta::specta]
pub async fn render_source(app: AppState<'_>, id: u32) -> Result<Option<RenderSourceDto>, IpcError> {
    #[allow(clippy::cast_precision_loss)] // file sizes stay far below 2^53
    Ok(app
        .rendered
        .source(ModelId(i64::from(id)))?
        .map(|(path, size)| RenderSourceDto { path: path.into(), size: size as f64 }))
}

/// Stores a preview rendered in the app (in the app's folder, never the library).
#[tauri::command]
#[specta::specta]
pub async fn store_rendered_preview(app: AppState<'_>, id: u32, png: Vec<u8>) -> Result<(), IpcError> {
    let app = Arc::clone(&app);
    tauri::async_runtime::spawn_blocking(move || app.rendered.store(ModelId(i64::from(id)), &png))
        .await
        .map_err(|error| internal(&error))??;
    Ok(())
}

#[derive(Debug, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct KeptDto {
    pub written: u32,
    pub kept: u32,
}

/// Keeps rendered previews: written into the model folder where allowed, else kept in the index.
#[tauri::command]
#[specta::specta]
pub async fn keep_rendered_previews(app: AppState<'_>, ids: Vec<u32>) -> Result<KeptDto, IpcError> {
    let app = Arc::clone(&app);
    let ids: Vec<ModelId> = ids.into_iter().map(|id| ModelId(i64::from(id))).collect();
    let kept = tauri::async_runtime::spawn_blocking(move || app.rendered.keep(&ids))
        .await
        .map_err(|error| internal(&error))??;
    let count = |value: usize| u32::try_from(value).unwrap_or(u32::MAX);
    Ok(KeptDto { written: count(kept.written), kept: count(kept.kept) })
}

#[tauri::command]
#[specta::specta]
pub async fn skip_rendered_previews(app: AppState<'_>, ids: Vec<u32>) -> Result<(), IpcError> {
    let ids: Vec<ModelId> = ids.into_iter().map(|id| ModelId(i64::from(id))).collect();
    Ok(app.rendered.skip(&ids)?)
}

/// Sets rendered previews back to `state` (`cached`, or none to render again): undo of skip and of keeping in the index.
#[tauri::command]
#[specta::specta]
pub async fn reset_rendered_previews(app: AppState<'_>, ids: Vec<u32>, state: Option<String>) -> Result<(), IpcError> {
    let ids: Vec<ModelId> = ids.into_iter().map(|id| ModelId(i64::from(id))).collect();
    let state = match state.as_deref() {
        None => None,
        Some("cached") => Some("cached"),
        Some(_) => return Err(hoarddeck_application::AppError::InvalidInput("unknown state".to_owned()).into()),
    };
    Ok(app.rendered.reset(&ids, state)?)
}
