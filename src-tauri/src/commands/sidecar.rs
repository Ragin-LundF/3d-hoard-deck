//! `.3dhoarddeck` files: write them all, status, backup file, restore from files or a backup, saved data of a folder.

use std::path::PathBuf;
use std::sync::Arc;

use hoarddeck_application::scan::ScanMode;
use hoarddeck_application::sidecar::{ExportReport, RestoreMode, RestoreReport, RestoreSource};
use hoarddeck_domain::LibraryId;

use super::AppState;
use crate::dto::IpcError;

fn count(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

#[allow(clippy::cast_precision_loss)] // seconds since 1970 fit a f64 exactly
fn seconds(value: i64) -> f64 {
    value as f64
}

fn blocking_error(error: &tauri::Error) -> IpcError {
    IpcError { code: "internal".to_owned(), message: error.to_string(), details: None }
}

#[derive(Debug, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FailedFileDto {
    pub file: String,
    pub reason: String,
}

#[derive(Debug, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ExportReportDto {
    pub written: u32,
    pub unchanged: u32,
    pub failed: Vec<FailedFileDto>,
}

impl From<ExportReport> for ExportReportDto {
    fn from(report: ExportReport) -> Self {
        Self {
            written: count(report.written),
            unchanged: count(report.unchanged),
            failed: report.failed.into_iter().map(|(file, reason)| FailedFileDto { file, reason }).collect(),
        }
    }
}

#[derive(Debug, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct MetadataStatusDto {
    /// Last full "Write metadata files", seconds since 1970.
    pub exported_at: Option<f64>,
    /// Files that could not be written yet.
    pub pending: u32,
}

#[derive(Debug, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RestoreReportDto {
    pub restored: u32,
    pub unchanged: u32,
    pub unreadable: Vec<String>,
    pub rules_restored: u32,
    pub dropped_members: u32,
    pub tag_rules_restored: u32,
    pub tag_rules_offered: u32,
}

impl From<RestoreReport> for RestoreReportDto {
    fn from(report: RestoreReport) -> Self {
        Self {
            restored: count(report.restored),
            unchanged: count(report.unchanged),
            unreadable: report.unreadable,
            rules_restored: count(report.rules_restored),
            dropped_members: count(report.dropped_members),
            tag_rules_restored: count(report.tag_rules_restored),
            tag_rules_offered: count(report.tag_rules_offered),
        }
    }
}

/// What a folder's root `.3dhoarddeck` holds, for "this folder has saved data".
#[derive(Debug, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SavedSummaryDto {
    pub name: String,
    pub exported_at: f64,
    pub folder_rules: u32,
    pub collections: u32,
    pub entries: u32,
}

fn source(backup: Option<String>) -> RestoreSource {
    backup.map_or(RestoreSource::Files, |path| RestoreSource::Backup(PathBuf::from(path)))
}

/// Writes the root file and a file for every model and folder with user data; progress as `MetadataProgressEvent`.
#[tauri::command]
#[specta::specta]
pub async fn write_metadata_files(app: AppState<'_>, library: u32) -> Result<ExportReportDto, IpcError> {
    let app = Arc::clone(&app);
    let report = tauri::async_runtime::spawn_blocking(move || app.sidecar_sync.export(LibraryId(i64::from(library))))
        .await
        .map_err(|error| blocking_error(&error))??;
    Ok(report.into())
}

/// Stops a running "Write metadata files".
#[tauri::command]
#[specta::specta]
pub async fn cancel_metadata_files(app: AppState<'_>) -> Result<(), IpcError> {
    app.sidecar_sync.cancel();
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn metadata_status(app: AppState<'_>, library: u32) -> Result<MetadataStatusDto, IpcError> {
    let status = app.sidecar_sync.status(LibraryId(i64::from(library)))?;
    Ok(MetadataStatusDto { exported_at: status.exported_at.map(seconds), pending: count(status.pending) })
}

/// Tries the files that could not be written again.
#[tauri::command]
#[specta::specta]
pub async fn retry_metadata_files(app: AppState<'_>, library: u32) -> Result<MetadataStatusDto, IpcError> {
    let app = Arc::clone(&app);
    let id = LibraryId(i64::from(library));
    let status = tauri::async_runtime::spawn_blocking(move || {
        app.sidecar_sync.library_changed(id);
        app.sidecar_sync.status(id)
    })
    .await
    .map_err(|error| blocking_error(&error))??;
    Ok(MetadataStatusDto { exported_at: status.exported_at.map(seconds), pending: count(status.pending) })
}

/// Writes one backup file with everything of the library to `path`; returns the number of model and folder entries.
#[tauri::command]
#[specta::specta]
pub async fn export_backup(app: AppState<'_>, library: u32, path: String) -> Result<u32, IpcError> {
    let app = Arc::clone(&app);
    let entries = tauri::async_runtime::spawn_blocking(move || {
        app.sidecar_sync.backup(LibraryId(i64::from(library)), &PathBuf::from(path))
    })
    .await
    .map_err(|error| blocking_error(&error))??;
    Ok(count(entries))
}

/// Restores from the library's `.3dhoarddeck` files (after a quick scan), or from `backup`. `filesWin` replaces index values; otherwise a
/// file wins only when it is newer.
#[tauri::command]
#[specta::specta]
pub async fn restore_from_sidecars(
    app: AppState<'_>,
    library: u32,
    files_win: bool,
    backup: Option<String>,
) -> Result<RestoreReportDto, IpcError> {
    let app = Arc::clone(&app);
    let mode = if files_win { RestoreMode::FilesWin } else { RestoreMode::NewerWins };
    let report = tauri::async_runtime::spawn_blocking(move || {
        let id = LibraryId(i64::from(library));
        let source = source(backup);
        // The restore reads the metadata files the scan found; a quick scan first sees files added since.
        if source == RestoreSource::Files {
            app.scan_library.execute(id, ScanMode::Incremental)?;
        }
        app.restore_from_sidecars.execute(id, &source, mode)
    })
    .await
    .map_err(|error| blocking_error(&error))??;
    Ok(report.into())
}

/// After adding a library whose folder holds saved data: the first scan, then the restore.
#[tauri::command]
#[specta::specta]
pub async fn scan_and_restore(app: AppState<'_>, library: u32) -> Result<RestoreReportDto, IpcError> {
    let app = Arc::clone(&app);
    let id = LibraryId(i64::from(library));
    let report = tauri::async_runtime::spawn_blocking(move || {
        app.scan_library.execute(id, ScanMode::Incremental)?;
        app.restore_from_sidecars.execute(id, &RestoreSource::Files, RestoreMode::NewerWins)
    })
    .await
    .map_err(|error| blocking_error(&error))??;
    Ok(report.into())
}

/// Replaces the app's tag rules with those saved for the library (after the user agreed). Returns how many.
#[tauri::command]
#[specta::specta]
pub async fn use_saved_tag_rules(app: AppState<'_>, library: u32, backup: Option<String>) -> Result<u32, IpcError> {
    Ok(count(app.restore_from_sidecars.use_saved_tag_rules(LibraryId(i64::from(library)), &source(backup))?))
}

/// What the `.3dhoarddeck` at the root of `folder` holds; `None` when there is no saved library data.
#[tauri::command]
#[specta::specta]
pub async fn saved_library_data(app: AppState<'_>, folder: String) -> Result<Option<SavedSummaryDto>, IpcError> {
    Ok(app.restore_from_sidecars.saved_summary(&folder).map(|saved| SavedSummaryDto {
        name: saved.name,
        exported_at: seconds(saved.exported_at),
        folder_rules: count(saved.folder_rules),
        collections: count(saved.collections),
        entries: count(saved.entries),
    }))
}
