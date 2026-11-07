//! Hand-made and smart collections.

use hoarddeck_domain::CollectionId;
use hoarddeck_domain::ModelId;

use super::AppState;
use crate::dto::{CollectionDto, IpcError, SearchResultDto};

#[tauri::command]
#[specta::specta]
pub async fn list_collections(app: AppState<'_>) -> Result<Vec<CollectionDto>, IpcError> {
    Ok(app.collections.list()?.into_iter().map(Into::into).collect())
}

/// `query`: search text for a smart collection, `None` for a hand-made one.
#[tauri::command]
#[specta::specta]
pub async fn create_collection(
    app: AppState<'_>,
    name: String,
    query: Option<String>,
) -> Result<CollectionDto, IpcError> {
    let collection = app.collections.create(&name, query.as_deref())?;
    let count = collection.query.is_none().then_some(0);
    Ok((collection, count).into())
}

#[tauri::command]
#[specta::specta]
pub async fn rename_collection(app: AppState<'_>, id: u32, name: String) -> Result<(), IpcError> {
    Ok(app.collections.rename(collection_id(id), &name)?)
}

/// Deletes the collection; its models stay in the library.
#[tauri::command]
#[specta::specta]
pub async fn delete_collection(app: AppState<'_>, id: u32) -> Result<(), IpcError> {
    Ok(app.collections.delete(collection_id(id))?)
}

#[tauri::command]
#[specta::specta]
pub async fn add_to_collection(app: AppState<'_>, id: u32, models: Vec<u32>) -> Result<(), IpcError> {
    Ok(app.collections.add(collection_id(id), &model_ids(&models))?)
}

#[tauri::command]
#[specta::specta]
pub async fn remove_from_collection(app: AppState<'_>, id: u32, models: Vec<u32>) -> Result<(), IpcError> {
    Ok(app.collections.remove(collection_id(id), &model_ids(&models))?)
}

/// Moves a member of a hand-made collection to position `index` (0 = first).
#[tauri::command]
#[specta::specta]
pub async fn move_in_collection(app: AppState<'_>, id: u32, model: u32, index: u32) -> Result<(), IpcError> {
    Ok(app.collections.move_to(collection_id(id), ModelId(i64::from(model)), index as usize)?)
}

#[tauri::command]
#[specta::specta]
pub async fn collection_models(
    app: AppState<'_>,
    id: u32,
    offset: u32,
    limit: u32,
) -> Result<SearchResultDto, IpcError> {
    let (models, total) = app.collections.models(collection_id(id), offset as usize, (limit as usize).clamp(1, 500))?;
    Ok(hoarddeck_application::search::SearchResult { models, total }.into())
}

fn collection_id(id: u32) -> CollectionId {
    CollectionId(i64::from(id))
}

fn model_ids(ids: &[u32]) -> Vec<ModelId> {
    ids.iter().map(|id| ModelId(i64::from(*id))).collect()
}
