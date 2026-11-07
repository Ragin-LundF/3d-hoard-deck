//! Model summaries, files and details.

use hoarddeck_application::{ModelDetail, ModelSummary};
use hoarddeck_domain::ModelFile;
use serde::Serialize;
use specta::Type;

use super::id32;

#[derive(Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ModelSummaryDto {
    pub id: u32,
    pub library: u32,
    pub path: String,
    pub folder: String,
    /// `folder` or `file`.
    pub kind: String,
    pub title: String,
    pub released: Option<String>,
    pub cover: Option<String>,
    /// Folder path of the collection the model belongs to.
    pub collection: Option<String>,
    /// The user marked this model's folder as one model; offer "detect automatically".
    pub marked_by_user: bool,
    pub formats: Vec<String>,
    pub file_count: u32,
    pub total_size: f64,
}

impl From<ModelSummary> for ModelSummaryDto {
    #[allow(clippy::cast_precision_loss)] // sizes beyond 2^53 bytes are not a concern
    fn from(model: ModelSummary) -> Self {
        Self {
            id: id32(model.id.0),
            library: id32(model.library.0),
            path: model.path.into(),
            folder: model.folder.into(),
            kind: model.kind.code().to_owned(),
            title: model.title,
            released: model.released.map(|r| r.to_string()),
            cover: model.cover.map(String::from),
            collection: model.collection.map(String::from),
            marked_by_user: model.marked_by_user,
            formats: model.formats.iter().map(|f| f.code().to_owned()).collect(),
            file_count: model.file_count,
            total_size: model.total_size as f64,
        }
    }
}

#[derive(Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ModelFileDto {
    pub path: String,
    /// File kind code: a format (`stl`, `3mf` …) or `slicer`, `image`, `doc`, `archive`, `other`.
    pub kind: String,
    pub size: f64,
    pub variant: Option<String>,
}

impl From<ModelFile> for ModelFileDto {
    #[allow(clippy::cast_precision_loss)]
    fn from(file: ModelFile) -> Self {
        Self {
            path: file.path.into(),
            kind: file.kind.code().to_owned(),
            size: file.size as f64,
            variant: file.variant,
        }
    }
}

#[derive(Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ModelDetailDto {
    pub summary: ModelSummaryDto,
    pub files: Vec<ModelFileDto>,
}

impl From<ModelDetail> for ModelDetailDto {
    fn from(detail: ModelDetail) -> Self {
        Self { summary: detail.summary.into(), files: detail.files.into_iter().map(Into::into).collect() }
    }
}
