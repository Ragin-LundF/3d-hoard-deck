//! Tags as a whole: the tag bar, the tag browser, renaming, merging, groups and pins.

use std::sync::Arc;

use hoarddeck_domain::{LibraryId, TagName};

use super::AppState;
use crate::dto::{IpcError, TagCountDto, TagGroupDto, TagInfoDto, rel_path};

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
