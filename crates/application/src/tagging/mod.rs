//! Tags that appear without typing: tag rules on folder and file names.

mod manage_tag_rules;
mod presets;
mod rules;
mod tag_rule_book;

pub use manage_tag_rules::ManageTagRules;
pub use presets::tag_rule_presets;
pub use rules::{CompiledTagRules, RuleKind, TagRule};
pub use tag_rule_book::TagRuleBook;
