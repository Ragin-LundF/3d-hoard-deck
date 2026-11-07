//! Folder rules and collection preset proposals.

use hoarddeck_application::scan::{FolderRole, FolderRule, PresetProposal};
use serde::Serialize;
use specta::Type;

use super::FolderRoleDto;

#[derive(Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct FolderRuleDto {
    /// Glob relative to the library root; a single folder for marks made by right-click.
    pub pattern: String,
    pub role: FolderRoleDto,
}

impl From<FolderRule> for FolderRuleDto {
    fn from(rule: FolderRule) -> Self {
        let mapped = match rule.role {
            FolderRole::Model => FolderRoleDto::Model,
            FolderRole::Collection => FolderRoleDto::Collection,
        };
        Self { pattern: rule.pattern, role: mapped }
    }
}

#[derive(Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PresetProposalDto {
    pub id: String,
    pub name: String,
    pub pattern: String,
    pub matches: u32,
    pub samples: Vec<String>,
}

impl From<PresetProposal> for PresetProposalDto {
    fn from(proposal: PresetProposal) -> Self {
        Self {
            id: proposal.id,
            name: proposal.name,
            pattern: proposal.pattern,
            matches: u32::try_from(proposal.matches).unwrap_or(u32::MAX),
            samples: proposal.samples,
        }
    }
}
