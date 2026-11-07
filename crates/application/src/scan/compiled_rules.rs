use std::collections::HashSet;

use globset::{Glob, GlobBuilder, GlobSet, GlobSetBuilder};
use hoarddeck_domain::RelPath;

use super::{FolderRole, ScanRules};
use crate::AppError;

/// [`ScanRules`] ready for fast matching.
#[derive(Debug)]
pub struct CompiledRules {
    ignore: GlobSet,
    models: GlobSet,
    collections: GlobSet,
    variants: HashSet<String>,
}

impl CompiledRules {
    /// # Errors
    /// Returns [`AppError::InvalidInput`] for an invalid glob.
    pub fn compile(rules: &ScanRules) -> Result<Self, AppError> {
        Ok(Self {
            ignore: glob_set(rules.ignore.iter())?,
            models: glob_set(rules.with_role(FolderRole::Model))?,
            collections: glob_set(rules.with_role(FolderRole::Collection))?,
            variants: rules.variant_folders.iter().map(|name| normalize(name)).collect(),
        })
    }

    #[must_use]
    pub fn is_ignored(&self, path: &RelPath) -> bool {
        is_own_file(path.name()) || self.ignore.is_match(path.as_str())
    }

    /// True when a folder is ignored as a whole, e.g. `@eaDir` for the pattern `**/@eaDir/**`.
    #[must_use]
    pub fn is_ignored_dir(&self, dir: &RelPath) -> bool {
        // Probe with a child name no real pattern targets, so `X/**` patterns match the folder itself.
        !dir.is_root() && (self.ignore.is_match(dir.as_str()) || self.ignore.is_match(format!("{dir}/\u{0}")))
    }

    /// Marked as one model by the user. Wins over a collection mark on the same folder.
    #[must_use]
    pub fn is_model(&self, dir: &RelPath) -> bool {
        !dir.is_root() && self.models.is_match(dir.as_str())
    }

    #[must_use]
    pub fn is_collection(&self, dir: &RelPath) -> bool {
        !dir.is_root() && self.collections.is_match(dir.as_str()) && !self.is_model(dir)
    }

    /// Variant folder names from the list, plus scale folders like `75mm` or `120 %`.
    #[must_use]
    pub fn is_variant(&self, folder_name: &str) -> bool {
        let name = normalize(folder_name);
        self.variants.contains(&name) || is_scale(&name)
    }
}

/// The app's metadata files and their temporary copies are never library content, whatever the rules say.
#[allow(clippy::case_sensitive_file_extension_comparisons)] // compared lower-cased; `.3dhoarddeck` has no extension
fn is_own_file(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.ends_with(".3dhoarddeck") || (lower.contains(".3dhoarddeck.") && lower.ends_with(".tmp"))
}

fn glob_set<'a>(patterns: impl Iterator<Item = &'a String>) -> Result<GlobSet, AppError> {
    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        builder.add(compile_glob(pattern)?);
    }
    builder.build().map_err(|error| AppError::InvalidInput(error.to_string()))
}

fn compile_glob(pattern: &str) -> Result<Glob, AppError> {
    GlobBuilder::new(pattern)
        .case_insensitive(true)
        .literal_separator(true)
        .build()
        .map_err(|error| AppError::InvalidInput(format!("{pattern}: {error}")))
}

fn normalize(name: &str) -> String {
    name.to_lowercase().replace(['_', '-'], " ").split_whitespace().collect::<Vec<_>>().join(" ")
}

fn is_scale(name: &str) -> bool {
    let unit = name.strip_suffix("mm").or_else(|| name.strip_suffix('%'));
    unit.map(str::trim_end).is_some_and(|digits| !digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scan::FolderRule;

    fn path(value: &str) -> RelPath {
        RelPath::new(value).expect("valid path")
    }

    #[test]
    fn default_rules_skip_tool_folders_at_any_depth() {
        let rules = CompiledRules::compile(&ScanRules::default()).expect("valid defaults");
        assert!(rules.is_ignored(&path("_orynt3d/backup.json")));
        assert!(rules.is_ignored(&path("Minis/Dragon/config.orynt3d")));
        assert!(rules.is_ignored(&path("Minis/.DS_Store")));
        assert!(!rules.is_ignored(&path("Minis/Dragon/dragon.stl")));
        assert!(rules.is_ignored_dir(&path("Minis/@eaDir")));
        let none = CompiledRules::compile(&ScanRules { ignore: Vec::new(), ..ScanRules::default() }).expect("valid");
        assert!(none.is_ignored(&path("Minis/Dragon/.3dhoarddeck")), "own files even without any rule");
        assert!(none.is_ignored(&path("Makerworld/a.3mf.3dhoarddeck")));
        assert!(none.is_ignored(&path("Minis/Dragon/.3dhoarddeck.12.3.tmp")));
        assert!(rules.is_ignored_dir(&path("_orynt3d")));
        assert!(!rules.is_ignored_dir(&path("Minis/Dragon")));
    }

    #[test]
    fn variant_names_are_matched_loosely() {
        let rules = CompiledRules::compile(&ScanRules::default()).expect("valid defaults");
        for name in ["Supported", "Pre-Supported", "pre_supported", "STL Files", "75mm", "120 %"] {
            assert!(rules.is_variant(name), "{name} should be a variant folder");
        }
        assert!(!rules.is_variant("Dragon"));
        assert!(!rules.is_variant("mm"));
    }

    #[test]
    fn single_folder_rules_escape_glob_characters() {
        let rule = FolderRule::for_folder(&path("Minis/Dragon [v2]"), FolderRole::Model);
        let rules =
            CompiledRules::compile(&ScanRules { folder_rules: vec![rule], ..ScanRules::default() }).expect("valid");
        assert!(rules.is_model(&path("Minis/Dragon [v2]")));
        assert!(!rules.is_model(&path("Minis/Dragon v")));
    }

    #[test]
    fn collection_globs_match_folders_not_their_children() {
        let rules = ScanRules {
            folder_rules: vec![FolderRule { pattern: "**/Loot Studios/*/*".to_owned(), role: FolderRole::Collection }],
            ..ScanRules::default()
        };
        let rules = CompiledRules::compile(&rules).expect("valid rules");
        assert!(rules.is_collection(&path("Minis/Loot Studios/Fantasy/2023-01 - Dragons")));
        assert!(!rules.is_collection(&path("Minis/Loot Studios/Fantasy")));
        assert!(!rules.is_collection(&path("Minis/Loot Studios/Fantasy/2023-01 - Dragons/Supported")));
    }
}
