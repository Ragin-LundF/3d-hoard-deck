//! Names, tags, marks, notes, metadata files, `Orynt3D` import.

use std::sync::Arc;

use hoarddeck_domain::LibraryId;
use hoarddeck_domain::ModelId;

use super::AppState;
use crate::dto::{
    DuplicateGroupDto, ImportReportDto, IpcError, ModelDetailDto, ModelMetaDto, RebuildReportDto, TagCountDto,
};

/// Saves name, tags, marks and notes; also writes the `.3dhoarddeck` file when the library allows it.
#[tauri::command]
#[specta::specta]
pub async fn update_model_meta(app: AppState<'_>, id: u32, meta: ModelMetaDto) -> Result<ModelDetailDto, IpcError> {
    let meta = meta.try_into()?;
    Ok(app.update_model_meta.execute(ModelId(i64::from(id)), meta)?.into())
}

#[tauri::command]
#[specta::specta]
pub async fn list_tags(app: AppState<'_>, library: u32) -> Result<Vec<TagCountDto>, IpcError> {
    let tags = app.list_tags.execute(LibraryId(i64::from(library)))?;
    Ok(tags.into_iter().map(|(name, count)| TagCountDto { name: name.into(), count }).collect())
}

/// Restores names, tags, marks and notes from the library's `.3dhoarddeck` files (reads only).
#[tauri::command]
#[specta::specta]
pub async fn rebuild_from_sidecars(app: AppState<'_>, library: u32) -> Result<RebuildReportDto, IpcError> {
    let app = Arc::clone(&app);
    let report =
        tauri::async_runtime::spawn_blocking(move || app.rebuild_from_sidecars.execute(LibraryId(i64::from(library))))
            .await
            .map_err(|error| IpcError { code: "internal".to_owned(), message: error.to_string(), details: None })??;
    let count = |value: usize| u32::try_from(value).unwrap_or(u32::MAX);
    Ok(RebuildReportDto {
        restored: count(report.restored),
        unreadable: count(report.unreadable),
        rules_restored: count(report.rules_restored),
    })
}

/// Takes names, notes and tags from `Orynt3D`'s `config.orynt3d` files (reads only, safe to run again).
#[tauri::command]
#[specta::specta]
pub async fn import_orynt3d(app: AppState<'_>, library: u32) -> Result<ImportReportDto, IpcError> {
    let app = Arc::clone(&app);
    let report =
        tauri::async_runtime::spawn_blocking(move || app.import_orynt3d.execute(LibraryId(i64::from(library))))
            .await
            .map_err(|error| IpcError { code: "internal".to_owned(), message: error.to_string(), details: None })??;
    let count = |value: usize| u32::try_from(value).unwrap_or(u32::MAX);
    Ok(ImportReportDto { updated: count(report.updated), unreadable: count(report.unreadable) })
}

/// Identical model files across all libraries. Reads files, which takes a while over a network share.
#[tauri::command]
#[specta::specta]
pub async fn find_duplicates(app: AppState<'_>) -> Result<Vec<DuplicateGroupDto>, IpcError> {
    let app = Arc::clone(&app);
    let groups = tauri::async_runtime::spawn_blocking(move || app.find_duplicates.execute())
        .await
        .map_err(|error| IpcError { code: "internal".to_owned(), message: error.to_string(), details: None })??;
    Ok(groups.into_iter().map(Into::into).collect())
}

/// Stops a running duplicate search; it then fails with `cancelled`.
#[tauri::command]
#[specta::specta]
pub async fn cancel_duplicates(app: AppState<'_>) -> Result<(), IpcError> {
    app.find_duplicates.cancel();
    Ok(())
}
