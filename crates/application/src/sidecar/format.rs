//! The `.3dhoarddeck` file format, version 2. Additive to version 1: a model's fields stay flat at the top level, new
//! data sits in new keys, so an older app still reads (and keeps) everything it knows.

use std::collections::BTreeMap;

use hoarddeck_domain::{ModelMeta, RelPath};
use serde::{Deserialize, Serialize};

use crate::scan::{FolderRole, FolderRule};
use crate::tagging::TagRule;

/// Format version written into every file.
pub const FORMAT_VERSION: u64 = 2;

/// What the file in a model folder, beside a single-file model, or in any folder with user data holds.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct EntryFile {
    /// Version 1 fields: name, tags, hidden rule tags, marks, notes and the picked cover (library-relative).
    #[serde(flatten)]
    pub meta: ModelMeta,
    /// The picked cover relative to the file's folder, so it is still found after the folder was renamed or moved.
    pub cover_in_folder: Option<String>,
    /// When the model first appeared in the library (seconds since 1970).
    pub added_at: Option<i64>,
    /// `skipped`: the user does not want a rendered preview for this model.
    pub rendered_preview: Option<String>,
    /// What the user decided for this folder; `None` in a single-file model's file.
    pub folder: Option<FolderFile>,
}

/// User decisions about a folder, kept in the folder itself so they follow it when it is renamed outside the app.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct FolderFile {
    /// Set when the user marked exactly this folder as one model or as a collection.
    pub role: Option<FolderRole>,
    /// The folder cover the user picked, relative to this folder.
    pub cover: Option<String>,
    /// A picked folder cover outside the folder (possible in read-only libraries), library-relative.
    pub cover_from_library: Option<String>,
}

impl EntryFile {
    /// Whether the file holds anything a restore would need.
    #[must_use]
    pub fn has_user_data(&self) -> bool {
        !self.meta.is_empty()
            || self.rendered_preview.is_some()
            || self.folder.as_ref().is_some_and(|folder| {
                folder.role.is_some() || folder.cover.is_some() || folder.cover_from_library.is_some()
            })
    }
}

/// The library section of the `.3dhoarddeck` at the library root.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LibraryFile {
    /// Created once; tells "the same drive at another mount path" apart from another library.
    pub uid: String,
    pub name: String,
    /// `index_only` or `index_and_sidecar`; access is never stored, a file must not grant write access.
    pub metadata_storage: Option<String>,
    pub scan_rules: SavedScanRules,
    pub tag_groups: Vec<SavedTagGroup>,
    /// Tag keys pinned in the sidebar.
    pub pinned_tags: Vec<String>,
    pub collections: Vec<SavedCollection>,
    /// A copy of the app-wide tag rules; a restore offers them, and uses them only when the app has none yet.
    pub tag_rules: Vec<TagRule>,
    /// How many folders and models had user data at the last export, for the "saved data found" message.
    pub entries: usize,
    pub exported_at: i64,
    pub app_version: String,
}

/// Scan rules besides the folder rules, which stay in `folderRules` where version 1 put them.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct SavedScanRules {
    pub ignore: Vec<String>,
    pub variant_folders: Vec<String>,
    pub parent_covers: Option<bool>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct SavedTagGroup {
    pub name: String,
    pub color: String,
    pub position: i64,
    /// Tag keys in the group.
    pub tags: Vec<String>,
}

/// A hand-made collection (`members`, in order) or a smart one (`query` with an optional folder `scope`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct SavedCollection {
    pub name: String,
    pub members: Vec<String>,
    pub query: Option<String>,
    pub scope: Option<String>,
    /// A smart collection limited to this library (without a folder scope).
    pub library_only: bool,
}

/// The whole root file: the version 1 folder rules plus the library section.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct RootFile {
    pub folder_rules: Vec<FolderRule>,
    pub library: Option<LibraryFile>,
}

/// One portable file with everything of a library, for libraries that cannot hold metadata files (read-only shares).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct BackupFile {
    pub version: u64,
    #[serde(flatten)]
    pub root: RootFile,
    /// Model and folder entries by the library-relative path their `.3dhoarddeck` file would have.
    pub entries: BTreeMap<String, EntryFile>,
}

/// `cover` relative to `folder`, when it lies inside it.
#[must_use]
pub fn relative_to(cover: &RelPath, folder: &RelPath) -> Option<String> {
    if folder.is_root() {
        return Some(cover.to_string());
    }
    cover.as_str().strip_prefix(folder.as_str()).and_then(|rest| rest.strip_prefix('/')).map(str::to_owned)
}

/// `relative` resolved against `folder`.
#[must_use]
pub fn resolve(relative: &str, folder: &RelPath) -> Option<RelPath> {
    if folder.is_root() { RelPath::new(relative).ok() } else { RelPath::new(format!("{folder}/{relative}")).ok() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_one_files_still_read_and_new_fields_stay_optional() {
        let v1: EntryFile =
            serde_json::from_str(r#"{"version":1,"tags":["Resin"],"favorite":true,"future":1}"#).expect("v1");
        assert!(v1.meta.favorite && v1.added_at.is_none() && v1.folder.is_none());
        let v2 = EntryFile { added_at: Some(5), rendered_preview: Some("skipped".to_owned()), ..v1.clone() };
        let text = serde_json::to_string(&v2).expect("write");
        // A version 1 reader (plain ModelMeta) still reads the flat fields.
        let old: ModelMeta = serde_json::from_str(&text).expect("old reader");
        assert_eq!(old, v1.meta);
        assert_eq!(serde_json::from_str::<EntryFile>(&text).expect("v2"), v2);
    }

    #[test]
    fn covers_are_kept_relative_to_their_folder() {
        let folder = RelPath::new("Minis/Dragon").expect("path");
        let cover = RelPath::new("Minis/Dragon/images/front.jpg").expect("path");
        assert_eq!(relative_to(&cover, &folder).as_deref(), Some("images/front.jpg"));
        assert_eq!(relative_to(&RelPath::new("Other/a.jpg").expect("path"), &folder), None);
        let moved = RelPath::new("Archive/Red Dragon").expect("path");
        assert_eq!(
            resolve("images/front.jpg", &moved).map(String::from).as_deref(),
            Some("Archive/Red Dragon/images/front.jpg")
        );
    }
}
