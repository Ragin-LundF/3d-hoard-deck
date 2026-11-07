//! Slicer list entries.

use hoarddeck_domain::Slicer;
use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SlicerDto {
    pub name: String,
    /// Executable, or an `.app` bundle on macOS.
    pub program: String,
    /// `{files}` stands for the files to open.
    pub args: Vec<String>,
    /// Lower-case file extensions, e.g. `stl`, `3mf`.
    pub extensions: Vec<String>,
}

impl From<Slicer> for SlicerDto {
    fn from(slicer: Slicer) -> Self {
        Self { name: slicer.name, program: slicer.program, args: slicer.args, extensions: slicer.extensions }
    }
}

impl From<SlicerDto> for Slicer {
    /// Unvalidated; the use case validates every entry before saving.
    fn from(dto: SlicerDto) -> Self {
        Self { name: dto.name, program: dto.program, args: dto.args, extensions: dto.extensions }
    }
}
