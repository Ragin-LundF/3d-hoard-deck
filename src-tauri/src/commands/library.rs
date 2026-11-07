//! Libraries: list, add, settings, scanning.

use std::sync::Arc;

use hoarddeck_application::AppError;
use hoarddeck_application::scan::ScanMode;
use hoarddeck_domain::LibraryId;
use tauri::AppHandle;
use tauri_specta::Event;

use super::AppState;
use crate::dto::{AccessModeDto, IpcError, LibraryDto, MetadataStorageDto};
use crate::events::ScanProgressEvent;

#[tauri::command]
#[specta::specta]
pub async fn list_libraries(app: AppState<'_>) -> Result<Vec<LibraryDto>, IpcError> {
    Ok(app.libraries.list().map_err(AppError::from)?.into_iter().map(Into::into).collect())
}

#[tauri::command]
#[specta::specta]
pub async fn add_library(app: AppState<'_>, name: String, root: String) -> Result<LibraryDto, IpcError> {
    // Store the resolved folder, so symlinks or `..` cannot register the same library twice or hide the index folder.
    let root = std::fs::canonicalize(&root).map_or(root, |path| path.display().to_string());
    Ok(app.add_library.execute(&name, &root)?.into())
}

/// Saves what the app may write into a library. Switching metadata files on writes them for existing data at once;
/// returns how many files were written.
#[tauri::command]
#[specta::specta]
pub async fn update_library_settings(
    app: AppState<'_>,
    library: u32,
    access: AccessModeDto,
    metadata: MetadataStorageDto,
) -> Result<u32, IpcError> {
    let app = Arc::clone(&app);
    let (_, written) = tauri::async_runtime::spawn_blocking(move || {
        app.update_library_settings.execute(LibraryId(i64::from(library)), access.into(), metadata.into())
    })
    .await
    .map_err(|error| IpcError { code: "internal".to_owned(), message: error.to_string(), details: None })??;
    Ok(u32::try_from(written).unwrap_or(u32::MAX))
}

/// Starts a scan in the background and returns at once; progress arrives as `ScanProgressEvent`s.
/// `full` also lists folders whose mtime did not change, to catch files overwritten in place.
#[tauri::command]
#[specta::specta]
pub async fn scan_library(app: AppState<'_>, handle: AppHandle, library: u32, full: bool) -> Result<(), IpcError> {
    let app = Arc::clone(&app);
    let mode = if full { ScanMode::Full } else { ScanMode::Incremental };
    std::thread::spawn(move || {
        match app.scan_library.execute(LibraryId(i64::from(library)), mode) {
            // A scan of this library is already running and reports its own progress; this request is dropped.
            // shortcut: a full rescan asked for during a normal scan is lost; queue it if that turns out to matter.
            Ok(_) | Err(AppError::LibraryBusy(_)) => {}
            Err(error) => {
                // A closed window cannot be told; nothing else to do.
                let _ = ScanProgressEvent::failed(library, error.into()).emit(&handle);
            }
        }
    });
    Ok(())
}
