//! Search requests and results.

use hoarddeck_application::search::SearchResult;
use serde::{Deserialize, Serialize};
use specta::Type;

use super::model::ModelSummaryDto;

#[derive(Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SearchRequestDto {
    pub text: String,
    pub library: Option<u32>,
    pub scope: Option<String>,
    pub offset: u32,
    pub limit: u32,
}

#[derive(Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SearchResultDto {
    pub models: Vec<ModelSummaryDto>,
    pub total: u32,
}

impl From<SearchResult> for SearchResultDto {
    fn from(result: SearchResult) -> Self {
        Self {
            models: result.models.into_iter().map(Into::into).collect(),
            total: u32::try_from(result.total).unwrap_or(u32::MAX),
        }
    }
}
