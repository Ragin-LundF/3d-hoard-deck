//! The in-app update check: Tauri's updater reads `latest.json` of the latest published GitHub release.
//! macOS, Windows and Linux `AppImage` install the update themselves; `.deb`/`.rpm` installs get the package saved to
//! Downloads; without an updater key in this build, the release page is opened instead.

use std::path::Path;
use std::time::Duration;

use serde::Serialize;
use specta::Type;
use tauri::{AppHandle, Manager};
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
    /// The app downloads, installs and restarts itself.
    Install,
    /// A Linux system package: the new `.deb` is saved to Downloads.
    Deb,
    /// A Linux system package: the new `.rpm` is saved to Downloads.
    Rpm,
    /// Only the release page can help (no updater key in this build, or an unknown Linux install).
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

/// How the app was installed on Linux.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LinuxInstall {
    AppImage,
    Deb,
    Rpm,
    Unknown,
}

/// Picks the method from facts about the system, so it can be tested.
fn method_for(os: &str, keyed: bool, linux: LinuxInstall) -> UpdateMethod {
    match (os, linux) {
        ("linux", LinuxInstall::Deb) => UpdateMethod::Deb,
        ("linux", LinuxInstall::Rpm) => UpdateMethod::Rpm,
        ("linux", LinuxInstall::Unknown) => UpdateMethod::Page,
        _ if keyed => UpdateMethod::Install,
        _ => UpdateMethod::Page,
    }
}

fn method(app: &AppHandle) -> UpdateMethod {
    let exists = |tool: &str| ["/usr/bin", "/bin"].iter().any(|dir| Path::new(dir).join(tool).exists());
    // `APPIMAGE` is set only while running as an AppImage.
    let linux = if std::env::var_os("APPIMAGE").is_some() {
        LinuxInstall::AppImage
    } else if exists("dpkg") {
        LinuxInstall::Deb
    } else if exists("rpm") {
        LinuxInstall::Rpm
    } else {
        LinuxInstall::Unknown
    };
    method_for(std::env::consts::OS, has_key(app), linux)
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

/// Saves the new `.deb` or `.rpm` to Downloads and shows it in the file manager. Returns the file path.
#[tauri::command]
#[specta::specta]
pub async fn download_update_package(app: AppHandle, version: String) -> Result<String, IpcError> {
    // GitHub stores the spaces of "3D Hoard Deck" as dots in asset names.
    let name = match method(&app) {
        UpdateMethod::Deb => format!("3D.Hoard.Deck_{version}_amd64.deb"),
        UpdateMethod::Rpm => format!("3D.Hoard.Deck-{version}-1.x86_64.rpm"),
        _ => return Err(failed("this installation does not use a system package")),
    };
    let target = app.path().download_dir().map_err(failed)?.join(&name);
    let url = format!("{RELEASES}/download/v{version}/{name}");
    tauri::async_runtime::spawn_blocking(move || -> Result<String, IpcError> {
        let mut response = ureq::get(&url).call().map_err(failed)?;
        let mut file = std::fs::File::create(&target).map_err(failed)?;
        std::io::copy(&mut response.body_mut().as_reader(), &mut file).map_err(failed)?;
        Ok(target.to_string_lossy().into_owned())
    })
    .await
    .map_err(failed)?
    .inspect(|path| {
        let _ = app.opener().reveal_item_in_dir(path);
    })
}

/// Opens the release page of `version` in the browser.
#[tauri::command]
#[specta::specta]
pub async fn open_release_page(app: AppHandle, version: String) -> Result<(), IpcError> {
    app.opener().open_url(page(&version), None::<&str>).map_err(failed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_installation_gets_the_update_it_can_take() {
        assert_eq!(method_for("macos", true, LinuxInstall::Unknown), UpdateMethod::Install);
        assert_eq!(method_for("windows", false, LinuxInstall::Unknown), UpdateMethod::Page, "no key in this build");
        assert_eq!(method_for("linux", true, LinuxInstall::AppImage), UpdateMethod::Install, "AppImage updates itself");
        assert_eq!(method_for("linux", true, LinuxInstall::Deb), UpdateMethod::Deb);
        assert_eq!(method_for("linux", true, LinuxInstall::Rpm), UpdateMethod::Rpm);
        assert_eq!(method_for("linux", true, LinuxInstall::Unknown), UpdateMethod::Page);
    }
}
