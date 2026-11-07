use hoarddeck_domain::{FileFormat, LibraryId, ModelFile, ModelId, ModelKind, RelPath, YearMonth};

/// What a grid tile or list row needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelSummary {
    pub id: ModelId,
    pub library: LibraryId,
    pub path: RelPath,
    pub folder: RelPath,
    pub kind: ModelKind,
    pub title: String,
    pub released: Option<YearMonth>,
    pub cover: Option<RelPath>,
    pub collection: Option<RelPath>,
    pub marked_by_user: bool,
    pub formats: Vec<FileFormat>,
    pub file_count: u32,
    pub total_size: u64,
}

/// What the model page needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelDetail {
    pub summary: ModelSummary,
    pub files: Vec<ModelFile>,
}
