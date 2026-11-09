use hoarddeck_application::tagging::{RuleKind, TagRule};
use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum RuleKindDto {
    Any,
    All,
    Regex,
    Folder,
}

/// A tag rule: `tag` is added to models whose path matches `pattern` (see `kind`), unless it contains a word of
/// `unless`.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TagRuleDto {
    pub tag: String,
    pub kind: RuleKindDto,
    pub pattern: String,
    pub unless: String,
    pub name_only: bool,
    pub enabled: bool,
}

impl From<TagRule> for TagRuleDto {
    fn from(rule: TagRule) -> Self {
        Self {
            tag: rule.tag,
            kind: match rule.kind {
                RuleKind::Any => RuleKindDto::Any,
                RuleKind::All => RuleKindDto::All,
                RuleKind::Regex => RuleKindDto::Regex,
                RuleKind::Folder => RuleKindDto::Folder,
            },
            pattern: rule.pattern,
            unless: rule.unless,
            name_only: rule.name_only,
            enabled: rule.enabled,
        }
    }
}

impl From<TagRuleDto> for TagRule {
    fn from(dto: TagRuleDto) -> Self {
        Self {
            tag: dto.tag,
            kind: match dto.kind {
                RuleKindDto::Any => RuleKind::Any,
                RuleKindDto::All => RuleKind::All,
                RuleKindDto::Regex => RuleKind::Regex,
                RuleKindDto::Folder => RuleKind::Folder,
            },
            pattern: dto.pattern,
            unless: dto.unless,
            name_only: dto.name_only,
            enabled: dto.enabled,
        }
    }
}
