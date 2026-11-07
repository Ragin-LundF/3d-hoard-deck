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

/// Where tags, names, notes and collections are kept for a library.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum MetadataStorage {
    /// Only in the index; nothing is written into the library.
    #[default]
    IndexOnly,
    /// Also in a `.3dhoarddeck` file in each model folder, so the index can be rebuilt from the library.
    IndexAndSidecar,
}

impl MetadataStorage {
    #[must_use]
    pub fn code(self) -> &'static str {
        match self {
            Self::IndexOnly => "index_only",
            Self::IndexAndSidecar => "index_and_sidecar",
        }
    }

    /// # Errors
    /// Returns [`DomainError::UnknownCode`] for an unknown code.
    pub fn from_code(code: &str) -> Result<Self, DomainError> {
        match code {
            "index_only" => Ok(Self::IndexOnly),
            "index_and_sidecar" => Ok(Self::IndexAndSidecar),
            other => Err(DomainError::UnknownCode { what: "metadata storage", code: other.to_owned() }),
        }
    }
}

/// What the app may write into a library, chosen by the user per library.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LibrarySettings {
    access: AccessMode,
    metadata: MetadataStorage,
}

impl LibrarySettings {
    /// # Errors
    /// Returns [`DomainError::SidecarsNeedWriteAccess`] when sidecar files are wanted in a read-only library.
    pub fn new(access: AccessMode, metadata: MetadataStorage) -> Result<Self, DomainError> {
        if access == AccessMode::ReadOnly && metadata == MetadataStorage::IndexAndSidecar {
            return Err(DomainError::SidecarsNeedWriteAccess);
        }
        Ok(Self { access, metadata })
    }

    #[must_use]
    pub fn access(self) -> AccessMode {
        self.access
    }

    #[must_use]
    pub fn metadata(self) -> MetadataStorage {
        self.metadata
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Library {
    pub id: LibraryId,
    pub name: String,
    /// Absolute root folder in the platform's native form. Only adapters turn it into a file system path.
    pub root: String,
    pub settings: LibrarySettings,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sidecar_files_need_write_access() {
        assert_eq!(
            LibrarySettings::new(AccessMode::ReadOnly, MetadataStorage::IndexAndSidecar),
            Err(DomainError::SidecarsNeedWriteAccess)
        );
        let settings =
            LibrarySettings::new(AccessMode::MetadataWrite, MetadataStorage::IndexAndSidecar).expect("valid");
        assert_eq!(
            (settings.access(), settings.metadata()),
            (AccessMode::MetadataWrite, MetadataStorage::IndexAndSidecar)
        );
        assert_eq!(LibrarySettings::default().access(), AccessMode::ReadOnly, "new libraries start read-only");
    }
}
