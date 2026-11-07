use hoarddeck_domain::{DomainError, LibraryId};
use thiserror::Error;

/// Failure of an index adapter (SQLite).
#[derive(Debug, Error)]
#[error("index: {0}")]
pub struct IndexError(pub String);

/// Failure of the search adapter (Tantivy).
#[derive(Debug, Error)]
#[error("search index: {0}")]
pub struct SearchError(pub String);

/// Failure of a library walk as a whole. Single unreadable folders are reported as scan items instead.
#[derive(Debug, Error)]
pub enum ScanError {
    #[error("library folder is not reachable: {0}")]
    Offline(String),
    #[error("scan stopped: {0}")]
    Aborted(String),
}

/// Requests the app turns down for a reason the user can fix; the UI words them in its own language.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum Refusal {
    #[error("{0} already exists")]
    AlreadyExists(String),
    #[error("\"{0}\" cannot be used as a name: it is empty")]
    EmptyName(String),
    #[error("\"{0}\" cannot be used as a name: it contains one of \\ / : * ? \" < > | or a control character")]
    ForbiddenCharacter(String),
    #[error("\"{0}\" cannot be used as a name: it ends with a dot or a space")]
    TrailingDotOrSpace(String),
    #[error("\"{0}\" cannot be used as a name: Windows reserves it for devices")]
    ReservedName(String),
    #[error("a smart collection fills itself from its search")]
    SmartCollection,
    #[error("{0} does not allow renaming or moving files; change it under Settings › Libraries")]
    NotManaged(String),
}

#[derive(Debug, Error)]
pub enum AppError {
    #[error("library {0:?} not found")]
    LibraryNotFound(LibraryId),
    #[error("library {0:?} is being scanned or regrouped; try again when it is done")]
    LibraryBusy(LibraryId),
    #[error("an index already exists in that folder")]
    IndexExists { modified: i64 },
    #[error("the index must not be stored on {0}: SQLite files get damaged there")]
    UnsafeIndexLocation(String),
    #[error("no slicer in your list opens these files")]
    NoSlicer,
    #[error("the slicer could not be started: {0}")]
    LaunchFailed(String),
    #[error("cancelled")]
    Cancelled,
    #[error(transparent)]
    Refused(#[from] Refusal),
    #[error("invalid input: {0}")]
    InvalidInput(String),
    #[error(transparent)]
    Domain(#[from] DomainError),
    #[error(transparent)]
    Index(#[from] IndexError),
    #[error(transparent)]
    Search(#[from] SearchError),
    #[error(transparent)]
    Scan(#[from] ScanError),
}
