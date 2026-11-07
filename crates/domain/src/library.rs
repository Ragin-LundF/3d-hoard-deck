use serde::{Deserialize, Serialize};

use crate::{DomainError, LibraryId};

/// What the app may change inside a library folder. Everything except `ReadOnly` is opt-in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum AccessMode {
    #[default]
    ReadOnly,
    MetadataWrite,
    Managed,
}

impl AccessMode {
    #[must_use]
    pub fn code(self) -> &'static str {
        match self {
            Self::ReadOnly => "read_only",
            Self::MetadataWrite => "metadata_write",
            Self::Managed => "managed",
        }
    }

    /// # Errors
    /// Returns [`DomainError::UnknownCode`] for an unknown code.
    pub fn from_code(code: &str) -> Result<Self, DomainError> {
        match code {
            "read_only" => Ok(Self::ReadOnly),
            "metadata_write" => Ok(Self::MetadataWrite),
            "managed" => Ok(Self::Managed),
            other => Err(DomainError::UnknownCode { what: "access mode", code: other.to_owned() }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Library {
    pub id: LibraryId,
    pub name: String,
    /// Absolute root folder in the platform's native form. Only adapters turn it into a file system path.
    pub root: String,
    pub access: AccessMode,
}
