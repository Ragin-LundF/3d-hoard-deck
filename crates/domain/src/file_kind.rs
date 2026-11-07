use std::fmt;

use serde::{Deserialize, Serialize};

use crate::DomainError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum FileFormat {
    Stl,
    ThreeMf,
    Obj,
    Ply,
    Step,
    Gltf,
    Fbx,
    Blend,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FileKind {
    Mesh(FileFormat),
    SlicerProject,
    Image,
    Document,
    Archive,
    Other,
}

const CODES: [(&str, FileKind); 13] = [
    ("stl", FileKind::Mesh(FileFormat::Stl)),
    ("3mf", FileKind::Mesh(FileFormat::ThreeMf)),
    ("obj", FileKind::Mesh(FileFormat::Obj)),
    ("ply", FileKind::Mesh(FileFormat::Ply)),
    ("step", FileKind::Mesh(FileFormat::Step)),
    ("gltf", FileKind::Mesh(FileFormat::Gltf)),
    ("fbx", FileKind::Mesh(FileFormat::Fbx)),
    ("blend", FileKind::Mesh(FileFormat::Blend)),
    ("slicer", FileKind::SlicerProject),
    ("image", FileKind::Image),
    ("doc", FileKind::Document),
    ("archive", FileKind::Archive),
    ("other", FileKind::Other),
];

impl FileKind {
    /// Stable code used in the index.
    #[must_use]
    pub fn code(self) -> &'static str {
        CODES.iter().find(|(_, kind)| *kind == self).map_or("other", |(code, _)| code)
    }

    /// # Errors
    /// Returns [`DomainError::UnknownCode`] for an unknown code.
    pub fn from_code(code: &str) -> Result<Self, DomainError> {
        CODES
            .iter()
            .find(|(known, _)| *known == code)
            .map(|(_, kind)| *kind)
            .ok_or_else(|| DomainError::UnknownCode { what: "file kind", code: code.to_owned() })
    }

    /// Files that make a folder a model: meshes, slicer projects and archives.
    #[must_use]
    pub fn is_model_file(self) -> bool {
        matches!(self, Self::Mesh(_) | Self::SlicerProject | Self::Archive)
    }
}

impl FileFormat {
    #[must_use]
    pub fn code(self) -> &'static str {
        FileKind::Mesh(self).code()
    }

    /// # Errors
    /// Returns [`DomainError::UnknownCode`] for codes that are not mesh formats.
    pub fn from_code(code: &str) -> Result<Self, DomainError> {
        match FileKind::from_code(code)? {
            FileKind::Mesh(format) => Ok(format),
            _ => Err(DomainError::UnknownCode { what: "file format", code: code.to_owned() }),
        }
    }
}

impl fmt::Display for FileFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_round_trip() {
        for (code, kind) in CODES {
            assert_eq!(FileKind::from_code(code), Ok(kind));
            assert_eq!(kind.code(), code);
        }
    }
}
