use hoarddeck_application::ports::EventSink;
use hoarddeck_application::scan::{ScanPhase, ScanProgress};
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::AppHandle;
use tauri_specta::Event;

/// Pushed while a library is scanned. `phase` is `walking`, `grouping`, `done` or `failed`.
#[derive(Debug, Clone, Serialize, Deserialize, Type, Event)]
#[serde(rename_all = "camelCase")]
pub struct ScanProgressEvent {
    pub library: u32,
    pub phase: String,
    pub folders: f64,
    pub files: f64,
    /// Set when `phase` is `failed`; same codes as command errors.
    pub error: Option<crate::dto::IpcError>,
}

pub struct TauriEvents {
    handle: AppHandle,
}

impl TauriEvents {
    pub fn new(handle: AppHandle) -> Self {
        Self { handle }
    }
}

impl ScanProgressEvent {
    pub fn failed(library: u32, error: crate::dto::IpcError) -> Self {
        Self { library, phase: "failed".to_owned(), folders: 0.0, files: 0.0, error: Some(error) }
    }
}

impl EventSink for TauriEvents {
    #[allow(clippy::cast_precision_loss)]
    fn scan_progress(&self, progress: &ScanProgress) {
        let phase = match progress.phase {
            ScanPhase::Walking => "walking",
            ScanPhase::Grouping => "grouping",
            ScanPhase::Done => "done",
        };
        let event = ScanProgressEvent {
            library: u32::try_from(progress.library.0).unwrap_or(u32::MAX),
            phase: phase.to_owned(),
            folders: progress.dirs as f64,
            files: progress.files as f64,
            error: None,
        };
        // A closed window cannot receive progress; nothing to recover.
        let _ = event.emit(&self.handle);
    }
}
