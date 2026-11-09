//! The in-app update check: Tauri's updater reads `latest.json` of the latest published GitHub release, then downloads,
//! checks the signature and installs (macOS, Windows, Linux `AppImage`, `.deb` and `.rpm`). Without an updater key in
//! this build, the release page is opened instead.

use std::time::Duration;

use serde::Serialize;
use specta::Type;
use tauri::AppHandle;
use tauri_plugin_opener::OpenerExt;
use tauri_plugin_updater::UpdaterExt;
use tauri_specta::Event;

use crate::dto::IpcError;

const RELEASES: &str = "https://github.com/Ragin-LundF/3d-hoard-deck/releases";
/// Asking GitHub must never hold anything up; a slow or missing network means "no update".
const CHECK_TIMEOUT: Duration = Duration::from_secs(10);

/// How this installation can take an update.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum UpdateMethod {
    /// The app downloads, checks, installs and restarts itself.
    Install,
    /// Only the release page can help: this build has no updater key.
    Page,
}

#[derive(Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfoDto {
    pub version: String,
    pub current: String,
    /// Release notes (Markdown), possibly empty.
    pub notes: String,
    /// `YYYY-MM-DD`, when the release says.
    pub date: Option<String>,
    pub method: UpdateMethod,
    pub page: String,
}

/// Pushed while an update downloads: `done` of `total` bytes (`total` is 0 when unknown).
#[derive(Debug, Clone, Serialize, serde::Deserialize, Type, tauri_specta::Event)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProgressEvent {
    pub done: u32,
    pub total: u32,
}

fn failed(error: impl std::fmt::Display) -> IpcError {
    IpcError { code: "update_failed".to_owned(), message: error.to_string(), details: None }
}

/// The updater's public key from `tauri.conf.json`; empty until the release key pair exists.
fn has_key(app: &AppHandle) -> bool {
    app.config()
        .plugins
        .0
        .get("updater")
        .and_then(|updater| updater.get("pubkey"))
        .and_then(|key| key.as_str())
        .is_some_and(|key| !key.trim().is_empty())
}

fn method(app: &AppHandle) -> UpdateMethod {
    if has_key(app) { UpdateMethod::Install } else { UpdateMethod::Page }
}

fn page(version: &str) -> String {
    format!("{RELEASES}/tag/v{version}")
}

/// A newer published release, or `None`. Errors (no network, GitHub down) are for the log and "Check now" only.
#[tauri::command]
#[specta::specta]
pub async fn check_for_update(app: AppHandle) -> Result<Option<UpdateInfoDto>, IpcError> {
    let updater = app.updater_builder().timeout(CHECK_TIMEOUT).build().map_err(failed)?;
    let Some(update) = updater.check().await.map_err(failed)? else {
        return Ok(None);
    };
    Ok(Some(UpdateInfoDto {
        page: page(&update.version),
        method: method(&app),
        notes: update.body.clone().unwrap_or_default(),
        date: update.date.map(|date| date.date().to_string()),
        current: update.current_version.clone(),
        version: update.version,
    }))
}

/// Downloads, checks the signature, installs and restarts. Progress comes as [`UpdateProgressEvent`].
#[tauri::command]
#[specta::specta]
pub async fn install_update(app: AppHandle) -> Result<(), IpcError> {
    let updater = app.updater().map_err(failed)?;
    let update = updater.check().await.map_err(failed)?.ok_or_else(|| failed("there is no newer version any more"))?;
    let mut done = 0_u64;
    let events = app.clone();
    update
        .download_and_install(
            move |chunk, total| {
                done += chunk as u64;
                let bytes = |value: u64| u32::try_from(value).unwrap_or(u32::MAX);
                let _ = UpdateProgressEvent { done: bytes(done), total: bytes(total.unwrap_or(0)) }.emit(&events);
            },
            || {},
        )
        .await
        .map_err(failed)?;
    app.restart();
}

/// Opens the release page of `version` in the browser.
#[tauri::command]
#[specta::specta]
pub async fn open_release_page(app: AppHandle, version: String) -> Result<(), IpcError> {
    app.opener().open_url(page(&version), None::<&str>).map_err(failed)
}
