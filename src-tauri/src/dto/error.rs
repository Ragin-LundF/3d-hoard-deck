//! IPC error shape and input checks.

use hoarddeck_application::{AppError, Refusal};
use hoarddeck_domain::RelPath;
use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct IpcError {
    /// Stable code the UI maps to a message: `library_not_found`, `offline`, `library_busy`, `index_exists`,
    /// `unsafe_location`, `no_slicer`, `launch_failed`, `cancelled`, the refusals below, `smart_collection`, `invalid_input`, `internal`.
    pub code: String,
    pub message: String,
    /// Extra data for some codes: for `index_exists`, the existing index's last change in seconds since 1970; for
    /// refusals (`already_exists`, `empty_name`, `forbidden_character`, `trailing_dot_or_space`, `reserved_name`,
    /// `not_managed`, `read_only`), the path, name or library concerned.
    pub details: Option<String>,
}

impl From<AppError> for IpcError {
    fn from(error: AppError) -> Self {
        let details = match &error {
            AppError::IndexExists { modified } => Some(modified.to_string()),
            AppError::Refused(
                Refusal::AlreadyExists(subject)
                | Refusal::EmptyName(subject)
                | Refusal::ForbiddenCharacter(subject)
                | Refusal::TrailingDotOrSpace(subject)
                | Refusal::ReservedName(subject)
                | Refusal::NotManaged(subject)
                | Refusal::ReadOnly(subject)
                | Refusal::NoTrash(subject),
            ) => Some(subject.clone()),
            _ => None,
        };
        let code = match &error {
            AppError::LibraryNotFound(_) => "library_not_found",
            AppError::IndexExists { .. } => "index_exists",
            AppError::UnsafeIndexLocation(_) => "unsafe_location",
            AppError::NoSlicer => "no_slicer",
            AppError::LaunchFailed(_) => "launch_failed",
            AppError::LibraryBusy(_) => "library_busy",
            AppError::Cancelled => "cancelled",
            AppError::Ai(_) => "ai_failed",
            AppError::Refused(refusal) => match refusal {
                Refusal::AlreadyExists(_) => "already_exists",
                Refusal::EmptyName(_) => "empty_name",
                Refusal::ForbiddenCharacter(_) => "forbidden_character",
                Refusal::TrailingDotOrSpace(_) => "trailing_dot_or_space",
                Refusal::ReservedName(_) => "reserved_name",
                Refusal::SmartCollection => "smart_collection",
                Refusal::NotManaged(_) => "not_managed",
                Refusal::ReadOnly(_) => "read_only",
                Refusal::NoTrash(_) => "no_trash",
                Refusal::WrongPassword => "wrong_password",
            },
            AppError::InvalidInput(_) | AppError::Domain(_) => "invalid_input",
            AppError::Scan(hoarddeck_application::ScanError::Offline(_)) => "offline",
            AppError::Index(_) | AppError::Search(_) | AppError::Scan(_) => "internal",
        };
        Self { code: code.to_owned(), message: error.to_string(), details }
    }
}

/// # Errors
/// Returns `invalid_input` for paths that are absolute or try to leave the library.
pub fn rel_path(path: &str) -> Result<RelPath, IpcError> {
    RelPath::new(path.trim_matches('/')).map_err(|error| AppError::from(error).into())
}
