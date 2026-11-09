//! Extracting archives into a new folder (Managed libraries only), then deleting or keeping the archive.

use std::sync::Arc;

use hoarddeck_application::files::{ArchiveChoice, ExtractPlan, Extracted};
use hoarddeck_application::ports::Deleted;
use hoarddeck_domain::{ModelId, RelPath};

use super::AppState;
use crate::dto::{IpcError, rel_path};

/// What extracting an archive would do; `problem` says why it cannot be done.
#[derive(Debug, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ExtractPlanDto {
    pub archive: String,
    pub parts: Vec<String>,
    pub folder: String,
    pub clash: bool,
    pub files: u32,
    /// Unpacked size in bytes.
    pub bytes: f64,
    pub unwrapped: Option<String>,
    pub encrypted: bool,
    pub free: Option<f64>,
    pub network: bool,
    pub problem: Option<String>,
}

#[derive(Debug, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RenamedDto {
    pub from: String,
    pub to: String,
}

/// A finished extraction waiting for the decision about the archive.
#[derive(Debug, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ExtractedDto {
    pub journal: u32,
    pub folder: String,
    pub files: u32,
    pub bytes: f64,
    pub renamed: Vec<RenamedDto>,
    pub skipped: Vec<String>,
    pub unwrapped: Option<String>,
    pub pictures: Vec<String>,
    pub warning: Option<String>,
}

/// An extracted archive that is still there.
#[derive(Debug, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PendingExtractionDto {
    pub journal: u32,
    pub archive: String,
    pub folder: String,
}

fn count(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

#[allow(clippy::cast_precision_loss)] // byte counts below 2^53 are exact
fn bytes(value: u64) -> f64 {
    value as f64
}

fn journal_id(id: i64) -> u32 {
    u32::try_from(id).unwrap_or(0)
}

impl From<ExtractPlan> for ExtractPlanDto {
    fn from(plan: ExtractPlan) -> Self {
        Self {
            archive: plan.archive.to_string(),
            parts: plan.parts.iter().map(RelPath::to_string).collect(),
            folder: plan.folder,
            clash: plan.clash,
            files: count(plan.files),
            bytes: bytes(plan.bytes),
            unwrapped: plan.unwrapped,
            encrypted: plan.encrypted,
            free: plan.free.map(bytes),
            network: plan.network,
            problem: plan.problem,
        }
    }
}

impl From<Extracted> for ExtractedDto {
    fn from(done: Extracted) -> Self {
        Self {
            journal: journal_id(done.journal),
            folder: done.folder.to_string(),
            files: count(done.files),
            bytes: bytes(done.bytes),
            renamed: done.renamed.into_iter().map(|(from, to)| RenamedDto { from, to }).collect(),
            skipped: done.skipped,
            unwrapped: done.unwrapped,
            pictures: done.pictures.iter().map(RelPath::to_string).collect(),
            warning: done.warning,
        }
    }
}

fn blocking_error(error: &tauri::Error) -> IpcError {
    IpcError { code: "internal".to_owned(), message: error.to_string(), details: None }
}

/// Plans for the given archives (files of model `id`). Reads the archives, writes nothing.
#[tauri::command]
#[specta::specta]
pub async fn plan_extraction(
    app: AppState<'_>,
    id: u32,
    archives: Vec<String>,
    password: Option<String>,
) -> Result<Vec<ExtractPlanDto>, IpcError> {
    let app = Arc::clone(&app);
    let archives = archives.iter().map(|archive| rel_path(archive)).collect::<Result<Vec<_>, _>>()?;
    let plans = tauri::async_runtime::spawn_blocking(move || {
        archives
            .iter()
            .map(|archive| app.extract_archives.plan(ModelId(i64::from(id)), archive, password.as_deref()))
            .collect::<Result<Vec<_>, _>>()
    })
    .await
    .map_err(|error| blocking_error(&error))??;
    Ok(plans.into_iter().map(Into::into).collect())
}

/// Extracts one archive into the new folder `name`; progress arrives as `ExtractProgressEvent`.
#[tauri::command]
#[specta::specta]
pub async fn extract_archive(
    app: AppState<'_>,
    id: u32,
    archive: String,
    name: String,
    password: Option<String>,
) -> Result<ExtractedDto, IpcError> {
    let app = Arc::clone(&app);
    let archive = rel_path(&archive)?;
    let done = tauri::async_runtime::spawn_blocking(move || {
        app.extract_archives.extract(ModelId(i64::from(id)), &archive, &name, password.as_deref())
    })
    .await
    .map_err(|error| blocking_error(&error))??;
    Ok(done.into())
}

/// Stops the running extraction; nothing is left behind.
#[tauri::command]
#[specta::specta]
pub async fn cancel_extraction(app: AppState<'_>) -> Result<(), IpcError> {
    app.extract_archives.cancel();
    Ok(())
}

/// How a settled extraction ended: `trashed` or `removed`, and a warning when the folder could not be read again.
#[derive(Debug, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FinishedDto {
    pub outcome: String,
    pub warning: Option<String>,
}

/// `delete` / `deletePermanently` the archive, or `discard` / `discardPermanently` the extracted folder (the plain
/// forms use the trash). Fails with code `no_trash` when the trash cannot be used; nothing is deleted then.
#[tauri::command]
#[specta::specta]
pub async fn finish_extraction(app: AppState<'_>, journal: u32, choice: String) -> Result<FinishedDto, IpcError> {
    let choice = match choice.as_str() {
        "delete" => ArchiveChoice::Delete { permanent: false },
        "deletePermanently" => ArchiveChoice::Delete { permanent: true },
        "discard" => ArchiveChoice::Discard { permanent: false },
        "discardPermanently" => ArchiveChoice::Discard { permanent: true },
        _ => return Err(hoarddeck_application::AppError::InvalidInput("unknown choice".to_owned()).into()),
    };
    let app = Arc::clone(&app);
    let finished =
        tauri::async_runtime::spawn_blocking(move || app.extract_archives.finish(i64::from(journal), choice))
            .await
            .map_err(|error| blocking_error(&error))??;
    let outcome = match finished.deleted {
        Deleted::Trashed => "trashed",
        Deleted::Removed => "removed",
    };
    Ok(FinishedDto { outcome: outcome.to_owned(), warning: finished.warning })
}

/// Extracted archives of model `id` that are still there, for "Extracted to … · Delete archive".
#[tauri::command]
#[specta::specta]
pub async fn pending_extractions(app: AppState<'_>, id: u32) -> Result<Vec<PendingExtractionDto>, IpcError> {
    Ok(app
        .extract_archives
        .pending(ModelId(i64::from(id)))?
        .into_iter()
        .map(|(journal, archive, folder)| PendingExtractionDto {
            journal: journal_id(journal),
            archive: archive.to_string(),
            folder: folder.to_string(),
        })
        .collect())
}
