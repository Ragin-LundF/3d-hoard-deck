use std::sync::Arc;

use hoarddeck_domain::RelPath;

use super::{CompiledTagRules, TagRule, TagRuleBook, tag_rule_presets};
use crate::AppError;
use crate::ports::{EntryStore, LibraryRepository};
use crate::scan::{CompiledRules, RuleEditor, detect};

/// Tag rules in Settings: list, suggested rules, trying a path, counting matches, and saving. Saving regroups every
/// library in the background from the index alone, so it never reads or writes a library.
pub struct ManageTagRules {
    pub book: Arc<TagRuleBook>,
    pub libraries: Arc<dyn LibraryRepository>,
    pub entries: Arc<dyn EntryStore>,
    pub editor: Arc<RuleEditor>,
}

impl ManageTagRules {
    /// # Errors
    /// Fails when the index cannot be read.
    pub fn rules(&self) -> Result<Vec<TagRule>, AppError> {
        self.book.rules()
    }

    #[must_use]
    pub fn presets(&self) -> Vec<TagRule> {
        tag_rule_presets()
    }

    /// # Errors
    /// Fails for invalid rules or when the index cannot be written.
    pub fn save(&self, rules: &[TagRule]) -> Result<(), AppError> {
        self.book.save(rules)?;
        for library in self.libraries.list()? {
            self.editor.regroup_soon(library.id);
        }
        Ok(())
    }

    /// The tags `rules` (not yet saved) give a typed path.
    /// # Errors
    /// Fails for invalid rules or an invalid path.
    pub fn test(&self, rules: &[TagRule], path: &str) -> Result<Vec<String>, AppError> {
        let path = RelPath::new(path.trim().trim_matches('/').replace('\\', "/"))?;
        Ok(CompiledTagRules::compile(rules)?.tags_for_path(&path))
    }

    /// For each of `rules` (not yet saved, switched on or not), how many models in all libraries it would tag.
    /// # Errors
    /// Fails for invalid rules or when the index cannot be read.
    pub fn counts(&self, rules: &[TagRule]) -> Result<Vec<u32>, AppError> {
        let compiled = CompiledTagRules::compile(rules)?;
        let mut counts = vec![0_u32; rules.len()];
        for library in self.libraries.list()? {
            let folder_rules = CompiledRules::compile(&self.libraries.scan_rules(library.id)?)?;
            let models = detect(&self.entries.files(library.id)?, &folder_rules).models;
            for (count, found) in counts.iter_mut().zip(compiled.count_matches(&models)) {
                *count += found;
            }
        }
        Ok(counts)
    }
}
