use hoarddeck_domain::RelPath;
use serde::{Deserialize, Serialize};

/// Per-library rules for turning folders into models. Stored with the library, applied to stored entries.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ScanRules {
    /// Globs relative to the library root; matching files and folders are skipped.
    pub ignore: Vec<String>,
    /// Subfolder names that are merged into their parent model as variants (case-insensitive).
    pub variant_folders: Vec<String>,
    /// What the user decided for folders the automatic detection gets wrong.
    pub folder_rules: Vec<FolderRule>,
    /// Models without a picture of their own borrow one from the folder above (one level, two if that has none).
    pub parent_covers: bool,
}

/// Gives every folder matching `pattern` a role chosen by the user.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FolderRule {
    /// Glob relative to the library root: a single folder (escaped path) or a pattern like `**/Loot Studios/*/*`.
    pub pattern: String,
    pub role: FolderRole,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FolderRole {
    /// All files in the folder and its subfolders form one model.
    Model,
    /// The folder groups every model below it, folder models and loose files alike.
    Collection,
}

impl FolderRule {
    /// A rule for exactly this folder; glob characters in its name are escaped.
    #[must_use]
    pub fn for_folder(folder: &RelPath, role: FolderRole) -> Self {
        Self { pattern: globset::escape(folder.as_str()), role }
    }
}

impl ScanRules {
    /// The same rules after the folder `from` was renamed or moved to `to`: rules for it or anything below follow it.
    #[must_use]
    pub fn moved(&self, from: &RelPath, to: &RelPath) -> Self {
        // Escaping keeps `/`, so an escaped path is a prefix of the escaped paths below it.
        let (old, new) = (globset::escape(from.as_str()), globset::escape(to.as_str()));
        let folder_rules = self
            .folder_rules
            .iter()
            .map(|rule| {
                let pattern = match rule.pattern.strip_prefix(&old) {
                    Some(rest) if rest.is_empty() || rest.starts_with('/') => format!("{new}{rest}"),
                    _ => rule.pattern.clone(),
                };
                FolderRule { pattern, role: rule.role }
            })
            .collect();
        Self { folder_rules, ..self.clone() }
    }

    pub(super) fn with_role(&self, role: FolderRole) -> impl Iterator<Item = &String> {
        self.folder_rules.iter().filter(move |rule| rule.role == role).map(|rule| &rule.pattern)
    }
}

impl Default for ScanRules {
    fn default() -> Self {
        let ignore = [
            "**/_orynt3d/**",
            "**/config.orynt3d",
            "**/.DS_Store",
            "**/._*",
            "**/Thumbs.db",
            "**/@eaDir/**",
            "**/#recycle/**",
            "**/.3dhoarddeck",
        ];
        let variants = [
            "supported",
            "pre supported",
            "presupported",
            "unsupported",
            "supports",
            "no supports",
            "resin",
            "fdm",
            "stl",
            "stls",
            "stl files",
            "step",
            "step files",
            "3mf",
            "obj",
            "files",
            "parts",
            "images",
            "image",
            "renders",
            "photos",
            "pics",
            "pictures",
            "lychee",
            "chitubox",
        ];
        Self {
            ignore: ignore.map(str::to_owned).to_vec(),
            variant_folders: variants.map(str::to_owned).to_vec(),
            folder_rules: Vec::new(),
            parent_covers: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn path(value: &str) -> RelPath {
        RelPath::new(value).expect("valid")
    }

    #[test]
    fn rules_follow_a_moved_folder_and_leave_others_alone() {
        let rules = ScanRules {
            folder_rules: vec![
                FolderRule::for_folder(&path("Minis/Dragon [v2]"), FolderRole::Model),
                FolderRule::for_folder(&path("Minis/Dragon [v2]/Parts"), FolderRole::Collection),
                FolderRule::for_folder(&path("Minis/Dragon [v2] old"), FolderRole::Model),
                FolderRule { pattern: "**/Loot Studios/*/*".to_owned(), role: FolderRole::Collection },
            ],
            ..ScanRules::default()
        };
        let moved = rules.moved(&path("Minis/Dragon [v2]"), &path("Archive/Dragon"));
        let patterns: Vec<&str> = moved.folder_rules.iter().map(|rule| rule.pattern.as_str()).collect();
        assert_eq!(
            patterns,
            ["Archive/Dragon", "Archive/Dragon/Parts", "Minis/Dragon [[]v2[]] old", "**/Loot Studios/*/*"]
        );
    }
}
