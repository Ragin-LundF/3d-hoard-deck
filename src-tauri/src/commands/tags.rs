//! Tags as a whole: the tag bar, the tag browser, renaming, merging, groups and pins.

use std::sync::Arc;

use hoarddeck_application::AppError;
use hoarddeck_application::organize::BatchTarget;
use hoarddeck_domain::{LibraryId, ModelId, TagName};

use super::AppState;
use crate::dto::{BatchResultDto, CheckItemDto, IpcError, TagCountDto, TagGroupDto, TagInfoDto, rel_path};

fn names(tags: &[String]) -> Result<Vec<TagName>, IpcError> {
    tags.iter()
        .map(|tag| TagName::new(tag).map_err(|error| hoarddeck_application::AppError::from(error).into()))
        .collect()
}

#[allow(clippy::cast_possible_truncation)] // group ids come from `list_tag_groups`
fn group_id(id: f64) -> i64 {
    id as i64
}

/// Every tag in use, most used first, with group and pin.
#[tauri::command]
#[specta::specta]
pub async fn list_tags(app: AppState<'_>, library: u32) -> Result<Vec<TagInfoDto>, IpcError> {
    Ok(app.tags.list(LibraryId(i64::from(library)))?.into_iter().map(Into::into).collect())
}

/// The most used tags of what is shown: `folder` (and below), narrowed by the search `query` when it is not empty.
#[tauri::command]
#[specta::specta]
pub async fn tag_facets(
    app: AppState<'_>,
    library: u32,
    folder: String,
    query: String,
    limit: u32,
) -> Result<Vec<TagCountDto>, IpcError> {
    let folder = rel_path(&folder)?;
    let app = Arc::clone(&app);
    let facets = tauri::async_runtime::spawn_blocking(move || {
        app.tags.facets(LibraryId(i64::from(library)), &folder, &query, limit as usize)
    })
    .await
    .map_err(|error| IpcError { code: "internal".to_owned(), message: error.to_string(), details: None })??;
    Ok(facets.into_iter().map(|(name, count)| TagCountDto { name: name.into(), count }).collect())
}

/// Renames tags, or merges several into one. Returns how many models changed.
#[tauri::command]
#[specta::specta]
pub async fn rename_tags(app: AppState<'_>, library: u32, from: Vec<String>, to: String) -> Result<u32, IpcError> {
    let (from, to) = (names(&from)?, names(&[to])?.remove(0));
    let app = Arc::clone(&app);
    let changed =
        tauri::async_runtime::spawn_blocking(move || app.tags.rename(LibraryId(i64::from(library)), &from, &to))
            .await
            .map_err(|error| IpcError { code: "internal".to_owned(), message: error.to_string(), details: None })??;
    Ok(u32::try_from(changed).unwrap_or(u32::MAX))
}

/// Removes tags from every model. Returns how many models changed.
#[tauri::command]
#[specta::specta]
pub async fn delete_tags(app: AppState<'_>, library: u32, tags: Vec<String>) -> Result<u32, IpcError> {
    let tags = names(&tags)?;
    let app = Arc::clone(&app);
    let changed = tauri::async_runtime::spawn_blocking(move || app.tags.delete(LibraryId(i64::from(library)), &tags))
        .await
        .map_err(|error| IpcError { code: "internal".to_owned(), message: error.to_string(), details: None })??;
    Ok(u32::try_from(changed).unwrap_or(u32::MAX))
}

#[tauri::command]
#[specta::specta]
pub async fn set_tags_group(
    app: AppState<'_>,
    library: u32,
    tags: Vec<String>,
    group: Option<f64>,
) -> Result<(), IpcError> {
    Ok(app.tags.set_group(LibraryId(i64::from(library)), &names(&tags)?, group.map(group_id))?)
}

#[tauri::command]
#[specta::specta]
pub async fn set_tags_pinned(app: AppState<'_>, library: u32, tags: Vec<String>, pinned: bool) -> Result<(), IpcError> {
    Ok(app.tags.set_pinned(LibraryId(i64::from(library)), &names(&tags)?, pinned)?)
}

#[tauri::command]
#[specta::specta]
pub async fn list_tag_groups(app: AppState<'_>, library: u32) -> Result<Vec<TagGroupDto>, IpcError> {
    Ok(app.tags.groups(LibraryId(i64::from(library)))?.into_iter().map(Into::into).collect())
}

/// Creates a group (`id` empty) or renames and recolours one. Returns its id.
#[tauri::command]
#[specta::specta]
#[allow(clippy::cast_precision_loss)] // ids stay far below 2^53
pub async fn save_tag_group(
    app: AppState<'_>,
    library: u32,
    id: Option<f64>,
    name: String,
    color: String,
) -> Result<f64, IpcError> {
    Ok(app.tags.save_group(LibraryId(i64::from(library)), id.map(group_id), &name, &color)? as f64)
}

#[tauri::command]
#[specta::specta]
pub async fn delete_tag_group(app: AppState<'_>, library: u32, id: f64) -> Result<(), IpcError> {
    Ok(app.tags.delete_group(LibraryId(i64::from(library)), group_id(id))?)
}

/// Adds and removes tags on the picked models (`ids`) or on every result of a search (`query` in `folder`), in one
/// go. Returns the number of models and a token for `undo_tag_batch`.
#[tauri::command]
#[specta::specta]
pub async fn batch_update_tags(
    app: AppState<'_>,
    library: u32,
    ids: Option<Vec<u32>>,
    query: Option<String>,
    folder: String,
    add: Vec<String>,
    remove: Vec<String>,
) -> Result<BatchResultDto, IpcError> {
    let target = match (ids, query) {
        (Some(ids), _) => BatchTarget::Models(ids.into_iter().map(|id| ModelId(i64::from(id))).collect()),
        (None, Some(text)) => BatchTarget::Search { text, folder: rel_path(&folder)? },
        (None, None) => return Err(AppError::InvalidInput("no models given".to_owned()).into()),
    };
    let (add, remove) = (names(&add)?, names(&remove)?);
    let app = Arc::clone(&app);
    let (changed, token) = tauri::async_runtime::spawn_blocking(move || {
        app.tags.batch(LibraryId(i64::from(library)), &target, &add, &remove)
    })
    .await
    .map_err(|error| IpcError { code: "internal".to_owned(), message: error.to_string(), details: None })??;
    Ok(BatchResultDto {
        changed: u32::try_from(changed).unwrap_or(u32::MAX),
        token: u32::try_from(token).unwrap_or(u32::MAX),
    })
}

#[tauri::command]
#[specta::specta]
pub async fn undo_tag_batch(app: AppState<'_>, token: u32) -> Result<u32, IpcError> {
    let app = Arc::clone(&app);
    let changed = tauri::async_runtime::spawn_blocking(move || app.tags.undo_batch(u64::from(token)))
        .await
        .map_err(|error| IpcError { code: "internal".to_owned(), message: error.to_string(), details: None })??;
    Ok(u32::try_from(changed).unwrap_or(u32::MAX))
}

/// The models in `folder` carrying `tag_name`, with where the tag comes from.
#[tauri::command]
#[specta::specta]
pub async fn check_tag(
    app: AppState<'_>,
    library: u32,
    tag_name: String,
    folder: String,
) -> Result<Vec<CheckItemDto>, IpcError> {
    let (tag, folder) = (names(&[tag_name])?.remove(0), rel_path(&folder)?);
    let app = Arc::clone(&app);
    let items =
        tauri::async_runtime::spawn_blocking(move || app.tags.check(LibraryId(i64::from(library)), &tag, &folder))
            .await
            .map_err(|error| IpcError { code: "internal".to_owned(), message: error.to_string(), details: None })??;
    Ok(items.into_iter().map(Into::into).collect())
}

/// Tags of the picked models with how many of them carry each, for "Remove tag".
#[tauri::command]
#[specta::specta]
pub async fn selection_tags(app: AppState<'_>, library: u32, ids: Vec<u32>) -> Result<Vec<TagCountDto>, IpcError> {
    let ids = ids.into_iter().map(|id| ModelId(i64::from(id))).collect();
    let counts = app.tags.counts_of(LibraryId(i64::from(library)), ids)?;
    Ok(counts.into_iter().map(|(name, count)| TagCountDto { name: name.into(), count }).collect())
}
