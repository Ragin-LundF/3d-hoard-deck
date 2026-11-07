//! Slicer list and opening files in a slicer.

use hoarddeck_domain::ModelId;

use super::AppState;
use crate::dto::{IpcError, SlicerDto, rel_path};

#[tauri::command]
#[specta::specta]
pub async fn list_slicers(app: AppState<'_>) -> Result<Vec<SlicerDto>, IpcError> {
    Ok(app.slicers.list()?.slicers.into_iter().map(Into::into).collect())
}

/// Replaces the list as edited in Settings; the first entry is the default.
#[tauri::command]
#[specta::specta]
pub async fn save_slicers(app: AppState<'_>, slicers: Vec<SlicerDto>) -> Result<Vec<SlicerDto>, IpcError> {
    let list = app.slicers.save(slicers.into_iter().map(Into::into).collect())?;
    Ok(list.slicers.into_iter().map(Into::into).collect())
}

#[tauri::command]
#[specta::specta]
pub async fn detect_slicers(app: AppState<'_>) -> Result<Vec<SlicerDto>, IpcError> {
    Ok(app.slicers.detect()?.slicers.into_iter().map(Into::into).collect())
}

/// Opens model files in a slicer: `files` (empty = the model's 3MF projects or main meshes) in slicer number
/// `slicer` (`None` = the first that opens them). Returns the slicer's name.
#[tauri::command]
#[specta::specta]
pub async fn open_in_slicer(
    app: AppState<'_>,
    model: u32,
    files: Vec<String>,
    slicer: Option<u32>,
) -> Result<String, IpcError> {
    let files = files.iter().map(|file| rel_path(file)).collect::<Result<Vec<_>, _>>()?;
    Ok(app.open_in_slicer.execute(ModelId(i64::from(model)), &files, slicer.map(|index| index as usize))?)
}
