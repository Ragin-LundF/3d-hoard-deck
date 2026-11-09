use std::collections::BTreeSet;

use crate::{DomainError, FileFormat, FileKind, RelPath, TagName, YearMonth};

/// How a model was formed from the library layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ModelKind {
    /// A folder holding mesh files (variant subfolders merged in).
    Folder,
    /// A single loose file next to other model folders.
    File,
}

impl ModelKind {
    #[must_use]
    pub fn code(self) -> &'static str {
        match self {
            Self::Folder => "folder",
            Self::File => "file",
        }
    }

    /// # Errors
    /// Returns [`DomainError::UnknownCode`] for an unknown code.
    pub fn from_code(code: &str) -> Result<Self, DomainError> {
        match code {
            "folder" => Ok(Self::Folder),
            "file" => Ok(Self::File),
            other => Err(DomainError::UnknownCode { what: "model kind", code: other.to_owned() }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelFile {
    pub path: RelPath,
    pub kind: FileKind,
    pub size: u64,
    /// Variant subfolder path below the model, e.g. `Supported` or `Supported/75mm`.
    pub variant: Option<String>,
}

/// One printable thing: a folder, a loose file or a collection, with all its files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Model {
    /// Model folder, or the file itself for [`ModelKind::File`].
    pub path: RelPath,
    /// The browsable folder the model is shown in.
    pub folder: RelPath,
    pub kind: ModelKind,
    pub title: String,
    pub released: Option<YearMonth>,
    pub cover: Option<RelPath>,
    /// The nearest folder marked as a collection at or above [`Model::folder`].
    pub collection: Option<RelPath>,
    /// The user marked this folder as one model; "detect automatically" undoes it.
    pub marked_by_user: bool,
    pub files: Vec<ModelFile>,
    /// Tags from the user's tag rules, derived from the path; kept apart from the tags the user typed.
    pub auto_tags: Vec<TagName>,
    /// Where [`Model::cover`] comes from.
    pub cover_source: CoverSource,
    /// The folder picture to show instead when the cover is a 3MF that turns out to have no preview inside.
    pub fallback_cover: Option<RelPath>,
}

/// Where a model's cover picture comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CoverSource {
    /// One of its own pictures, or the picture named like it next to it.
    #[default]
    Own,
    /// The preview image inside one of its 3MF files (Bambu Studio, `OrcaSlicer`).
    Embedded,
    /// Borrowed from the folder above, because it has no picture of its own.
    Parent,
}

impl CoverSource {
    #[must_use]
    pub fn code(self) -> &'static str {
        match self {
            Self::Own => "own",
            Self::Embedded => "embedded",
            Self::Parent => "parent",
        }
    }

    #[must_use]
    pub fn from_code(code: &str) -> Self {
        match code {
            "embedded" => Self::Embedded,
            "parent" => Self::Parent,
            _ => Self::Own,
        }
    }
}

impl Model {
    #[must_use]
    pub fn formats(&self) -> BTreeSet<FileFormat> {
        self.files
            .iter()
            .filter_map(|file| match file.kind {
                FileKind::Mesh(format) => Some(format),
                _ => None,
            })
            .collect()
    }

    #[must_use]
    pub fn total_size(&self) -> u64 {
        self.files.iter().map(|file| file.size).sum()
    }
}

/// A browsable folder that contains models somewhere below it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Folder {
    pub path: RelPath,
    pub display_name: String,
    pub released: Option<YearMonth>,
    pub direct_models: u32,
    pub total_models: u32,
    pub cover: Option<RelPath>,
    /// Marked as a collection: shown as one card; opening it shows every model inside.
    pub is_collection: bool,
}

/// Result of grouping a library's entries.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Catalog {
    pub folders: Vec<Folder>,
    pub models: Vec<Model>,
}
