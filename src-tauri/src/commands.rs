//! Tauri commands: deserialize, call one use case, map to a DTO. Nothing else.

use std::sync::Arc;

use hoarddeck_app::App;
use hoarddeck_application::AppError;
use hoarddeck_application::browse::BrowseRequest;
use hoarddeck_application::scan::ScanMode;
use hoarddeck_application::search::SearchRequest;
use hoarddeck_domain::{LibraryId, ModelId};
use tauri::{AppHandle, State};
use tauri_specta::Event;

use crate::events::ScanProgressEvent;

use crate::dto::{
    BrowseRequestDto, FolderDto, FolderPageDto, FolderRoleDto, IpcError, LibraryDto, ModelDetailDto, SearchRequestDto,
    SearchResultDto, rel_path,
};

type AppState<'a> = State<'a, Arc<App>>;

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

#[tauri::command]
#[specta::specta]
pub async fn browse_folder(app: AppState<'_>, request: BrowseRequestDto) -> Result<FolderPageDto, IpcError> {
    let request = BrowseRequest {
        library: LibraryId(i64::from(request.library)),
        folder: rel_path(&request.folder)?,
        recursive: request.recursive,
        sort: request.sort.into(),
        cursor: request.cursor.map(Into::into),
        limit: request.limit as usize,
    };
    Ok(app.browse_folder.execute(&request)?.into())
}

#[tauri::command]
#[specta::specta]
pub async fn folder_children(app: AppState<'_>, library: u32, folder: String) -> Result<Vec<FolderDto>, IpcError> {
    let folders = app.get_folder_children.execute(LibraryId(i64::from(library)), &rel_path(&folder)?)?;
    Ok(folders.into_iter().map(Into::into).collect())
}

#[tauri::command]
#[specta::specta]
pub async fn search_models(app: AppState<'_>, request: SearchRequestDto) -> Result<SearchResultDto, IpcError> {
    let request = SearchRequest {
        text: request.text,
        library: request.library.map(|id| LibraryId(i64::from(id))),
        scope: request.scope.as_deref().map(rel_path).transpose()?,
        offset: request.offset as usize,
        limit: request.limit as usize,
    };
    Ok(app.search_models.execute(&request)?.into())
}

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
    // Regrouping a big library takes a moment; keep it off the async runtime.
    tauri::async_runtime::spawn_blocking(move || {
        app.set_folder_role.execute(LibraryId(i64::from(library)), &folder, role.map(Into::into))
    })
    .await
    .map_err(|error| IpcError { code: "internal".to_owned(), message: error.to_string() })??;
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn get_model(app: AppState<'_>, id: u32) -> Result<Option<ModelDetailDto>, IpcError> {
    Ok(app.get_model.execute(ModelId(i64::from(id)))?.map(Into::into))
}
