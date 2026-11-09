use hoarddeck_domain::{FileFormat, LibraryId, ModelFile, ModelId, ModelKind, ModelMeta, RelPath, TagName, YearMonth};

/// What a grid tile or list row needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelSummary {
    pub id: ModelId,
    pub library: LibraryId,
    pub path: RelPath,
    pub folder: RelPath,
    pub kind: ModelKind,
    /// The user's display name if set, else the name derived from the folder or file.
    pub title: String,
    pub favorite: bool,
    pub printed: bool,
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
    /// The name derived from the folder or file, shown next to a display name.
    pub original_title: String,
    pub meta: ModelMeta,
    /// Tags from tag rules, including those the user hid (`meta.hidden_tags`).
    pub auto_tags: Vec<TagName>,
}
