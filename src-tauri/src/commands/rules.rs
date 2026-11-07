//! Folder roles, folder rules and collection presets.

use std::sync::Arc;

use hoarddeck_application::AppError;
use hoarddeck_domain::LibraryId;

use super::AppState;
use crate::dto::{FolderRoleDto, FolderRuleDto, IpcError, PresetProposalDto, rel_path};

/// Marks a folder as one model or a collection (`None` = automatic detection) and regroups at once.
#[tauri::command]
#[specta::specta]
pub async fn set_folder_role(
    app: AppState<'_>,
    library: u32,
    folder: String,
    role: Option<FolderRoleDto>,
) -> Result<(), IpcError> {
    let app = Arc::clone(&app);
    let folder = rel_path(&folder)?;
    blocking(move || app.set_folder_role.execute(LibraryId(i64::from(library)), &folder, role.map(Into::into))).await
}

#[tauri::command]
#[specta::specta]
pub async fn folder_rules(app: AppState<'_>, library: u32) -> Result<Vec<FolderRuleDto>, IpcError> {
    Ok(app.folder_rules.list(LibraryId(i64::from(library)))?.into_iter().map(Into::into).collect())
}

#[tauri::command]
#[specta::specta]
pub async fn remove_folder_rule(app: AppState<'_>, library: u32, pattern: String) -> Result<(), IpcError> {
    let app = Arc::clone(&app);
    blocking(move || app.folder_rules.remove(LibraryId(i64::from(library)), &pattern)).await
}

/// Collection presets that match this library's folders and are not in use yet.
#[tauri::command]
#[specta::specta]
pub async fn propose_presets(app: AppState<'_>, library: u32) -> Result<Vec<PresetProposalDto>, IpcError> {
    Ok(app.collection_presets.propose(LibraryId(i64::from(library)))?.into_iter().map(Into::into).collect())
}

#[tauri::command]
#[specta::specta]
pub async fn accept_preset(app: AppState<'_>, library: u32, id: String) -> Result<(), IpcError> {
    let app = Arc::clone(&app);
    blocking(move || app.collection_presets.accept(LibraryId(i64::from(library)), &id)).await
}

/// Regrouping a big library takes a moment; keep it off the async runtime.
pub(super) async fn blocking(work: impl FnOnce() -> Result<(), AppError> + Send + 'static) -> Result<(), IpcError> {
    tauri::async_runtime::spawn_blocking(work).await.map_err(|error| IpcError {
        code: "internal".to_owned(),
        message: error.to_string(),
        details: None,
    })??;
    Ok(())
}
