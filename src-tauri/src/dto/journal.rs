use hoarddeck_application::files::FileChange;
use hoarddeck_application::ports::JournalEntry;
use serde::Serialize;
use specta::Type;

use super::id32;

/// One rename or move on disk, newest first in lists.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct JournalEntryDto {
    pub id: u32,
    pub library: u32,
    pub from: String,
    pub to: String,
    /// Seconds since 1970, as a JavaScript-safe number.
    pub done_at: f64,
    /// `pending`, `done`, `failed`, `undoing` or `undone`; only `done` can be undone.
    pub status: String,
    /// An archive extracted into a new folder (`from` the archive, `to` the folder), not a move.
    pub extract: bool,
}

impl From<JournalEntry> for JournalEntryDto {
    #[allow(clippy::cast_precision_loss)] // seconds since 1970 fit a f64 exactly
    fn from(entry: JournalEntry) -> Self {
        Self {
            id: id32(entry.id),
            library: id32(entry.library.0),
            from: entry.from.to_string(),
            to: entry.to.to_string(),
            done_at: entry.done_at as f64,
            status: entry.status.code().to_owned(),
            extract: entry.extract.is_some(),
        }
    }
}

/// A finished rename, move or undo; `warning` when something after it did not work, like the rescan.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct FileChangeDto {
    pub journal: u32,
    pub warning: Option<String>,
}

impl From<FileChange> for FileChangeDto {
    fn from(change: FileChange) -> Self {
        Self { journal: id32(change.journal), warning: change.warning }
    }
}

/// Where a screenshot was written, relative to its library; `warning` when the rescan afterwards did not work.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ScreenshotDto {
    pub path: String,
    pub warning: Option<String>,
}
