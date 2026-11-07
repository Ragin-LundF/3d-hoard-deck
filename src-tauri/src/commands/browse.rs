//! Browsing, search and the model page.

use hoarddeck_application::browse::BrowseRequest;
use hoarddeck_application::search::SearchRequest;
use hoarddeck_domain::LibraryId;
use hoarddeck_domain::ModelId;

use super::AppState;
use crate::dto::{
    ArchiveEntryDto, BrowseRequestDto, FolderDto, FolderPageDto, IpcError, ModelDetailDto, SearchRequestDto,
    SearchResultDto, rel_path,
};

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

#[tauri::command]
#[specta::specta]
pub async fn get_model(app: AppState<'_>, id: u32) -> Result<Option<ModelDetailDto>, IpcError> {
    Ok(app.get_model.execute(ModelId(i64::from(id)))?.map(Into::into))
}

/// The files inside a ZIP archive, read without unpacking it.
#[tauri::command]
#[specta::specta]
pub async fn archive_contents(app: AppState<'_>, library: u32, path: String) -> Result<Vec<ArchiveEntryDto>, IpcError> {
    let app = std::sync::Arc::clone(&app);
    let path = rel_path(&path)?;
    let entries =
        tauri::async_runtime::spawn_blocking(move || app.list_archive.execute(LibraryId(i64::from(library)), &path))
            .await
            .map_err(|error| IpcError { code: "internal".to_owned(), message: error.to_string(), details: None })??;
    #[allow(clippy::cast_precision_loss)] // sizes stay exact as JavaScript numbers up to 8 PB
    Ok(entries.into_iter().map(|entry| ArchiveEntryDto { name: entry.name, size: entry.size as f64 }).collect())
}
