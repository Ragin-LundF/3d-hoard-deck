//! Optional tag suggestions from an AI. The API key goes from the settings form to the keychain and never back.

use std::sync::Arc;

use hoarddeck_application::AppError;
use hoarddeck_application::ai::KeyChange;
use hoarddeck_domain::{LibraryId, ModelId, TagName};

use super::AppState;
use crate::dto::{AiSettingsDto, AiTestDto, AiViewDto, IpcError, TagCountDto, rel_path};

fn tag(name: &str) -> Result<TagName, IpcError> {
    TagName::new(name).map_err(|error| AppError::from(error).into())
}

fn names(tags: Vec<TagName>) -> Vec<String> {
    tags.into_iter().map(String::from).collect()
}

/// Runs a blocking AI call off the IPC thread.
async fn blocking<T: Send + 'static>(
    app: &AppState<'_>,
    work: impl FnOnce(&hoarddeck_app::App) -> Result<T, AppError> + Send + 'static,
) -> Result<T, IpcError> {
    let app = Arc::clone(app);
    Ok(tauri::async_runtime::spawn_blocking(move || work(&app)).await.map_err(|error| IpcError {
        code: "internal".to_owned(),
        message: error.to_string(),
        details: None,
    })??)
}

#[tauri::command]
#[specta::specta]
pub async fn ai_settings(app: AppState<'_>) -> Result<AiViewDto, IpcError> {
    blocking(&app, |app| app.suggest_tags.settings()).await.map(Into::into)
}

/// `key`: `None` keeps the stored key, an empty text removes it.
#[tauri::command]
#[specta::specta]
pub async fn save_ai_settings(app: AppState<'_>, settings: AiSettingsDto, key: Option<String>) -> Result<(), IpcError> {
    let change = match key {
        None => KeyChange::Keep,
        Some(key) if key.is_empty() => KeyChange::Remove,
        Some(key) => KeyChange::Set(key),
    };
    blocking(&app, move |app| app.suggest_tags.save_settings(&settings.into(), change)).await
}

/// Lists the server's models and asks one question; `key` is a typed key not saved yet.
#[tauri::command]
#[specta::specta]
pub async fn test_ai_connection(
    app: AppState<'_>,
    settings: AiSettingsDto,
    key: Option<String>,
) -> Result<AiTestDto, IpcError> {
    blocking(&app, move |app| app.suggest_tags.test(&settings.into(), key)).await.map(Into::into)
}

#[tauri::command]
#[specta::specta]
pub async fn suggest_tags(app: AppState<'_>, id: u32) -> Result<Vec<String>, IpcError> {
    let id = ModelId(i64::from(id));
    blocking(&app, move |app| app.suggest_tags.suggest_model(id)).await.map(names)
}

#[tauri::command]
#[specta::specta]
pub async fn tag_suggestions(app: AppState<'_>, id: u32) -> Result<Vec<String>, IpcError> {
    let id = ModelId(i64::from(id));
    blocking(&app, move |app| app.suggest_tags.suggestions(id)).await.map(names)
}

/// Keeps a suggested tag as a normal tag, or dismisses it.
#[tauri::command]
#[specta::specta]
pub async fn resolve_tag_suggestion(app: AppState<'_>, id: u32, tag_name: String, keep: bool) -> Result<(), IpcError> {
    let (id, tag) = (ModelId(i64::from(id)), tag(&tag_name)?);
    blocking(&app, move |app| app.suggest_tags.resolve(id, &tag, keep)).await
}

/// Starts suggesting tags for the models in `folder` in the background. Returns how many models it asks about.
#[tauri::command]
#[specta::specta]
pub async fn start_tag_suggestions(app: AppState<'_>, library: u32, folder: String) -> Result<u32, IpcError> {
    let folder = rel_path(&folder)?;
    let total = app.suggest_tags.start(LibraryId(i64::from(library)), &folder)?;
    Ok(u32::try_from(total).unwrap_or(u32::MAX))
}

#[tauri::command]
#[specta::specta]
pub async fn cancel_tag_suggestions(app: AppState<'_>) -> Result<(), IpcError> {
    app.suggest_tags.cancel();
    Ok(())
}

/// Suggested tags of a library with the number of models, for the review.
#[tauri::command]
#[specta::specta]
pub async fn suggested_tag_counts(app: AppState<'_>, library: u32) -> Result<Vec<TagCountDto>, IpcError> {
    let counts = app.suggest_tags.counts(LibraryId(i64::from(library)))?;
    Ok(counts.into_iter().map(|(name, count)| TagCountDto { name: name.into(), count }).collect())
}

/// Keeps (`keep`) or dismisses one suggested tag on every model it was suggested for. Returns the number of models.
#[tauri::command]
#[specta::specta]
pub async fn resolve_suggested_tag(
    app: AppState<'_>,
    library: u32,
    tag_name: String,
    keep: bool,
) -> Result<u32, IpcError> {
    let (library, tag) = (LibraryId(i64::from(library)), tag(&tag_name)?);
    let changed = blocking(&app, move |app| {
        if keep { app.suggest_tags.accept_all(library, &tag) } else { app.suggest_tags.dismiss_all(library, &tag) }
    })
    .await?;
    Ok(u32::try_from(changed).unwrap_or(u32::MAX))
}
