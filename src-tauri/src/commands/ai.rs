//! Optional tag suggestions from an AI. The API key goes from the settings form to the keychain and never back.

use std::sync::Arc;

use hoarddeck_application::AppError;
use hoarddeck_application::ai::{AiFolders, Decision, KeyChange, Resolution};
use hoarddeck_domain::{LibraryId, ModelId, RelPath, TagName};

use super::AppState;
use crate::dto::{
    AiFoldersDto, AiSettingsDto, AiTestDto, AiViewDto, DecisionDto, IpcError, ResolutionDto, ReviewItemDto,
    TagCountDto, rel_path,
};

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

/// How many models a run over `folder` would ask about.
#[tauri::command]
#[specta::specta]
pub async fn count_tag_suggestions(app: AppState<'_>, library: u32, folder: String) -> Result<u32, IpcError> {
    let folder = rel_path(&folder)?;
    let count = blocking(&app, move |app| app.suggest_tags.count(LibraryId(i64::from(library)), &folder)).await?;
    Ok(u32::try_from(count).unwrap_or(u32::MAX))
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

fn tag_names(tags: &[String]) -> Result<Vec<TagName>, IpcError> {
    tags.iter().map(|name| tag(name)).collect()
}

fn paths(paths: &[String]) -> Result<Vec<RelPath>, IpcError> {
    paths.iter().map(|path| rel_path(path)).collect()
}

fn folders(dto: &AiFoldersDto) -> Result<AiFolders, IpcError> {
    Ok(AiFolders {
        included: paths(&dto.included)?,
        excluded: paths(&dto.excluded)?,
        only_untagged: dto.only_untagged,
        after_scan: dto.after_scan,
        skip_without_preview: dto.skip_without_preview,
    })
}

/// Models in `folder` with suggestions waiting, optionally only those suggested `tag_name`.
#[tauri::command]
#[specta::specta]
pub async fn review_suggestions(
    app: AppState<'_>,
    library: u32,
    folder: String,
    tag_name: Option<String>,
) -> Result<Vec<ReviewItemDto>, IpcError> {
    let (folder, tag) = (rel_path(&folder)?, tag_name.as_deref().map(tag).transpose()?);
    let items =
        blocking(&app, move |app| app.suggest_tags.review(LibraryId(i64::from(library)), &folder, tag.as_ref()))
            .await?;
    Ok(items.into_iter().map(Into::into).collect())
}

#[tauri::command]
#[specta::specta]
pub async fn pending_suggestions(app: AppState<'_>, library: u32, folder: String) -> Result<u32, IpcError> {
    let folder = rel_path(&folder)?;
    let count =
        blocking(&app, move |app| app.suggest_tags.pending_count(LibraryId(i64::from(library)), &folder)).await?;
    Ok(u32::try_from(count).unwrap_or(u32::MAX))
}

/// Keeps and drops suggested tags on many models in one go. Returns what undo needs.
#[tauri::command]
#[specta::specta]
pub async fn decide_suggestions(
    app: AppState<'_>,
    library: u32,
    decisions: Vec<DecisionDto>,
) -> Result<Vec<ResolutionDto>, IpcError> {
    let decisions = decisions
        .iter()
        .map(|d| {
            Ok(Decision { id: ModelId(i64::from(d.id)), keep: tag_names(&d.keep)?, dismiss: tag_names(&d.dismiss)? })
        })
        .collect::<Result<Vec<_>, IpcError>>()?;
    let done = blocking(&app, move |app| app.suggest_tags.decide(LibraryId(i64::from(library)), &decisions)).await?;
    Ok(done.into_iter().map(Into::into).collect())
}

#[tauri::command]
#[specta::specta]
pub async fn undo_suggestions(
    app: AppState<'_>,
    library: u32,
    resolutions: Vec<ResolutionDto>,
) -> Result<(), IpcError> {
    let resolutions = resolutions
        .iter()
        .map(|r| {
            Ok(Resolution { path: rel_path(&r.path)?, added: tag_names(&r.added)?, removed: tag_names(&r.removed)? })
        })
        .collect::<Result<Vec<_>, IpcError>>()?;
    blocking(&app, move |app| app.suggest_tags.undo(LibraryId(i64::from(library)), &resolutions)).await
}

#[tauri::command]
#[specta::specta]
pub async fn ai_folders(app: AppState<'_>, library: u32) -> Result<AiFoldersDto, IpcError> {
    Ok(app.suggest_tags.folders(LibraryId(i64::from(library)))?.into())
}

#[tauri::command]
#[specta::specta]
pub async fn save_ai_folders(app: AppState<'_>, library: u32, settings: AiFoldersDto) -> Result<(), IpcError> {
    Ok(app.suggest_tags.save_folders(LibraryId(i64::from(library)), &folders(&settings)?)?)
}

/// How many models a run over these (unsaved) AI folders would ask about.
#[tauri::command]
#[specta::specta]
pub async fn count_ai_folders(app: AppState<'_>, library: u32, settings: AiFoldersDto) -> Result<u32, IpcError> {
    let settings = folders(&settings)?;
    let count =
        blocking(&app, move |app| app.suggest_tags.count_folders(LibraryId(i64::from(library)), &settings)).await?;
    Ok(u32::try_from(count).unwrap_or(u32::MAX))
}

/// Starts a run over the saved AI folders of a library. Returns how many models it asks about.
#[tauri::command]
#[specta::specta]
pub async fn start_ai_folders(app: AppState<'_>, library: u32) -> Result<u32, IpcError> {
    let total = blocking(&app, move |app| {
        let suggest = std::sync::Arc::clone(&app.suggest_tags);
        suggest.start_folders(LibraryId(i64::from(library)))
    })
    .await?;
    Ok(u32::try_from(total).unwrap_or(u32::MAX))
}
