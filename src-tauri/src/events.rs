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
    /// Folders and files that could not be read; their previous index data is kept.
    pub unreadable: f64,
    /// `path: reason` for the first of them.
    pub unreadable_examples: Vec<String>,
    /// Only one folder was scanned; `unreadable` covers just that part.
    pub partial: bool,
    /// Set when `phase` is `failed`; same codes as command errors.
    pub error: Option<crate::dto::IpcError>,
}

/// Pushed while an archive is extracted: `done` of `total` bytes written, `file` being written.
#[derive(Debug, Clone, Serialize, Deserialize, Type, Event)]
#[serde(rename_all = "camelCase")]
pub struct ExtractProgressEvent {
    pub archive: String,
    pub done: f64,
    pub total: f64,
    pub file: String,
}

/// Pushed while "Write metadata files" runs: `done` of `total` files handled.
#[derive(Debug, Clone, Serialize, Deserialize, Type, Event)]
#[serde(rename_all = "camelCase")]
pub struct MetadataProgressEvent {
    pub library: u32,
    pub done: u32,
    pub total: u32,
}

/// Pushed while duplicates are searched: `done` of `total` groups of same-size files compared.
#[derive(Debug, Clone, Serialize, Deserialize, Type, Event)]
#[serde(rename_all = "camelCase")]
pub struct DuplicatesProgressEvent {
    pub done: u32,
    pub total: u32,
}

/// Pushed while an AI suggests tags: `done` of `total` models asked, `failed` without an answer.
#[derive(Debug, Clone, Serialize, Deserialize, Type, Event)]
#[serde(rename_all = "camelCase")]
pub struct SuggestProgressEvent {
    pub library: u32,
    pub done: u32,
    pub total: u32,
    pub failed: u32,
    pub running: bool,
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
        Self {
            library,
            phase: "failed".to_owned(),
            folders: 0.0,
            files: 0.0,
            unreadable: 0.0,
            unreadable_examples: Vec::new(),
            partial: false,
            error: Some(error),
        }
    }
}

impl EventSink for TauriEvents {
    #[allow(clippy::cast_precision_loss)]
    fn scan_progress(&self, progress: &ScanProgress) {
        let phase = match progress.phase {
            ScanPhase::Walking => "walking",
            ScanPhase::Grouping => "grouping",
            ScanPhase::Done => "done",
            ScanPhase::Regrouped => "regrouped",
        };
        let event = ScanProgressEvent {
            library: u32::try_from(progress.library.0).unwrap_or(u32::MAX),
            phase: phase.to_owned(),
            folders: progress.dirs as f64,
            files: progress.files as f64,
            unreadable: progress.failed as f64,
            unreadable_examples: progress.failures.clone(),
            partial: progress.partial,
            error: None,
        };
        if progress.phase == ScanPhase::Done {
            // The run configuration's console is where a user can read why a folder is missing.
            for failure in &progress.failures {
                eprintln!("could not read {failure}");
            }
        }
        // A closed window cannot receive progress; nothing to recover.
        let _ = event.emit(&self.handle);
    }

    #[allow(clippy::cast_precision_loss)] // byte counts below 2^53 are exact
    fn extract_progress(&self, archive: &hoarddeck_domain::RelPath, done: u64, total: u64, file: &str) {
        let event = ExtractProgressEvent {
            archive: archive.to_string(),
            done: done as f64,
            total: total as f64,
            file: file.to_owned(),
        };
        let _ = event.emit(&self.handle);
    }

    fn metadata_progress(&self, library: hoarddeck_domain::LibraryId, done: usize, total: usize) {
        let count = |value: usize| u32::try_from(value).unwrap_or(u32::MAX);
        let library = u32::try_from(library.0).unwrap_or(0);
        let _ = MetadataProgressEvent { library, done: count(done), total: count(total) }.emit(&self.handle);
    }

    fn duplicates_progress(&self, done: usize, total: usize) {
        let count = |value: usize| u32::try_from(value).unwrap_or(u32::MAX);
        let _ = DuplicatesProgressEvent { done: count(done), total: count(total) }.emit(&self.handle);
    }

    fn suggest_progress(
        &self,
        library: hoarddeck_domain::LibraryId,
        done: usize,
        total: usize,
        failed: usize,
        running: bool,
    ) {
        let count = |value: usize| u32::try_from(value).unwrap_or(u32::MAX);
        let library = u32::try_from(library.0).unwrap_or(0);
        let _ =
            SuggestProgressEvent { library, done: count(done), total: count(total), failed: count(failed), running }
                .emit(&self.handle);
    }
}
