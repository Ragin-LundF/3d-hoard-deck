//! Tag rules in Settings: tags from folder and file names.

use std::sync::Arc;

use hoarddeck_application::tagging::TagRule;

use super::AppState;
use crate::dto::{IpcError, TagRuleDto};

fn rules(rules: Vec<TagRuleDto>) -> Vec<TagRule> {
    rules.into_iter().map(Into::into).collect()
}

#[tauri::command]
#[specta::specta]
pub async fn tag_rules(app: AppState<'_>) -> Result<Vec<TagRuleDto>, IpcError> {
    Ok(app.tag_rules.rules()?.into_iter().map(Into::into).collect())
}

#[tauri::command]
#[specta::specta]
pub async fn tag_rule_presets(app: AppState<'_>) -> Result<Vec<TagRuleDto>, IpcError> {
    Ok(app.tag_rules.presets().into_iter().map(Into::into).collect())
}

/// Saves the rules; every library is regrouped in the background from the index alone.
#[tauri::command]
#[specta::specta]
pub async fn save_tag_rules(app: AppState<'_>, list: Vec<TagRuleDto>) -> Result<(), IpcError> {
    Ok(app.tag_rules.save(&rules(list))?)
}

/// The tags the (unsaved) rules give a typed path.
#[tauri::command]
#[specta::specta]
pub async fn test_tag_rules(app: AppState<'_>, list: Vec<TagRuleDto>, path: String) -> Result<Vec<String>, IpcError> {
    Ok(app.tag_rules.test(&rules(list), &path)?)
}

/// How many models each (unsaved) rule would tag, in all libraries.
#[tauri::command]
#[specta::specta]
pub async fn count_tag_rules(app: AppState<'_>, list: Vec<TagRuleDto>) -> Result<Vec<u32>, IpcError> {
    let app = Arc::clone(&app);
    let counts = tauri::async_runtime::spawn_blocking(move || app.tag_rules.counts(&rules(list)))
        .await
        .map_err(|error| IpcError { code: "internal".to_owned(), message: error.to_string(), details: None })??;
    Ok(counts)
}
