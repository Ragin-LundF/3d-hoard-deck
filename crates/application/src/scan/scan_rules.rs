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
        }
    }
}
