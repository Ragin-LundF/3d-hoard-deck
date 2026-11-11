//! `.3dhoarddeck` files: versioned JSON beside each model. Written only for libraries where the user allowed it.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use hoarddeck_application::ports::SidecarFiles;
use hoarddeck_application::sidecar::{BackupFile, EntryFile, FORMAT_VERSION, RootFile};
use hoarddeck_domain::RelPath;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Map, Value};

const SIDECAR_NAME: &str = ".3dhoarddeck";
static WRITES: AtomicU64 = AtomicU64::new(0);

pub struct SidecarFileStore;

impl SidecarFiles for SidecarFileStore {
    fn read_entry(&self, root: &str, file: &RelPath) -> Result<Option<(EntryFile, i64)>, String> {
        read_typed(&absolute(root, file))
    }

    fn write_entry(&self, root: &str, file: &RelPath, entry: &EntryFile) -> Result<bool, String> {
        merge_write(&absolute(root, file), entry)
    }

    fn read_root(&self, root: &str) -> Result<Option<(RootFile, i64)>, String> {
        read_typed(&absolute(root, &RelPath::root()).join(SIDECAR_NAME))
    }

    fn write_root(&self, root: &str, file: &RootFile) -> Result<bool, String> {
        merge_write(&absolute(root, &RelPath::root()).join(SIDECAR_NAME), file)
    }

    fn write_backup(&self, path: &Path, backup: &BackupFile) -> Result<(), String> {
        let json = serde_json::to_vec_pretty(backup).map_err(|e| e.to_string())?;
        write_atomically(path, &json)
    }

    fn read_backup(&self, path: &Path) -> Result<BackupFile, String> {
        let bytes = fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let value: Value = serde_json::from_slice(&bytes).map_err(|e| format!("{}: {e}", path.display()))?;
        // A library's root `.3dhoarddeck` has a library section too; only a backup has the entries.
        let has_entries = value.get("entries").is_some_and(Value::is_object);
        let backup: BackupFile = serde_json::from_value(value).map_err(|e| format!("{}: {e}", path.display()))?;
        if !has_entries || backup.version == 0 || backup.root.library.is_none() {
            return Err(format!("{} is no 3D Hoard Deck backup file", path.display()));
        }
        Ok(backup)
    }
}

/// Merges `content` into the file: fields this version does not know are kept, fields set to `null` are removed (a
/// cleared value). An unchanged file is not written, so its folder keeps its mtime and is not rescanned for nothing.
fn merge_write(target: &Path, content: &impl Serialize) -> Result<bool, String> {
    let Value::Object(known) = serde_json::to_value(content).map_err(|e| e.to_string())? else {
        return Err("metadata is not an object".to_owned());
    };
    let existing: Option<Map<String, Value>> =
        fs::read(target).ok().and_then(|bytes| serde_json::from_slice(&bytes).ok());
    let mut fields = existing.clone().unwrap_or_default();
    fields.extend(known);
    fields.retain(|_, value| !value.is_null());
    let comparable = |map: &Map<String, Value>| {
        let mut map = map.clone();
        map.remove("updatedAt");
        map.remove("version");
        map
    };
    if existing.as_ref().is_some_and(|old| comparable(old) == comparable(&fields)) {
        return Ok(false);
    }
    fields.insert("version".to_owned(), Value::from(FORMAT_VERSION));
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
    fields.insert("updatedAt".to_owned(), Value::from(now));
    let json = serde_json::to_vec_pretty(&Value::Object(fields)).map_err(|e| e.to_string())?;
    write_atomically(target, &json)?;
    Ok(true)
}

/// A temporary file in the same folder, then a rename: a reader never sees half a file.
fn write_atomically(target: &Path, bytes: &[u8]) -> Result<(), String> {
    let write = WRITES.fetch_add(1, Ordering::Relaxed);
    let temporary = target.with_file_name(format!(".3dhoarddeck.{}.{write}.tmp", std::process::id()));
    fs::write(&temporary, bytes).map_err(|e| format!("{}: {e}", temporary.display()))?;
    fs::rename(&temporary, target).map_err(|e| {
        let _ = fs::remove_file(&temporary);
        format!("{}: {e}", target.display())
    })
}

/// The file parsed into `T`, with its `updatedAt`.
fn read_typed<T: DeserializeOwned>(target: &Path) -> Result<Option<(T, i64)>, String> {
    let Some(value) = read_json(target)? else { return Ok(None) };
    let written = value.get("updatedAt").and_then(Value::as_i64).unwrap_or(0);
    let typed = serde_json::from_value(value).map_err(|e| format!("{}: {e}", target.display()))?;
    Ok(Some((typed, written)))
}

fn read_json(target: &Path) -> Result<Option<Value>, String> {
    match fs::read(target) {
        Ok(bytes) => serde_json::from_slice(&bytes).map(Some).map_err(|e| format!("{}: {e}", target.display())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!("{}: {error}", target.display())),
    }
}

fn absolute(root: &str, file: &RelPath) -> PathBuf {
    let mut path = PathBuf::from(root);
    path.extend(file.segments());
    path
}

#[cfg(test)]
mod tests {
    use hoarddeck_application::scan::{FolderRole, FolderRule};
    use hoarddeck_application::sidecar::{FolderFile, LibraryFile};
    use hoarddeck_domain::{ModelMeta, TagName};

    use super::*;

    #[test]
    fn entries_round_trip_keep_unknown_fields_and_skip_unchanged_writes() {
        let dir = tempfile::tempdir().expect("temp dir");
        let root = dir.path().to_str().expect("utf-8");
        fs::create_dir_all(dir.path().join("Dragon")).expect("model folder");
        let file = RelPath::new("Dragon/.3dhoarddeck").expect("valid");
        fs::write(dir.path().join("Dragon/.3dhoarddeck"), r#"{"futureField": 42, "tags": ["old"], "addedAt": 7}"#)
            .expect("seed");
        let entry = EntryFile {
            meta: ModelMeta {
                display_name: Some("Red Dragon".to_owned()),
                tags: [TagName::new("Resin").expect("valid")].into_iter().collect(),
                favorite: true,
                ..ModelMeta::default()
            },
            folder: Some(FolderFile { role: Some(FolderRole::Model), ..FolderFile::default() }),
            ..EntryFile::default()
        };
        assert!(SidecarFileStore.write_entry(root, &file, &entry).expect("write"));
        let (read, written) = SidecarFileStore.read_entry(root, &file).expect("read").expect("present");
        assert_eq!(read, entry, "a cleared addedAt is gone");
        assert!(written > 0, "updatedAt is recorded");
        let raw: Value =
            serde_json::from_slice(&fs::read(dir.path().join("Dragon/.3dhoarddeck")).expect("raw")).expect("json");
        assert_eq!((raw["futureField"].as_i64(), raw["version"].as_u64()), (Some(42), Some(FORMAT_VERSION)));
        assert!(!SidecarFileStore.write_entry(root, &file, &entry).expect("again"), "unchanged: not written");
        assert_eq!(
            SidecarFileStore.read_entry(root, &RelPath::new("None/.3dhoarddeck").expect("valid")).expect("read"),
            None
        );
    }

    #[test]
    fn the_root_file_keeps_version_one_folder_rules_and_backups_round_trip() {
        let dir = tempfile::tempdir().expect("temp dir");
        let root = dir.path().to_str().expect("utf-8");
        fs::write(dir.path().join(".3dhoarddeck"), r#"{"futureField": 1, "folderRules": []}"#).expect("seed");
        let file = RootFile {
            folder_rules: vec![FolderRule { pattern: "**/Loot Studios/*/*".to_owned(), role: FolderRole::Collection }],
            library: Some(LibraryFile { uid: "abc".to_owned(), name: "3D Print".to_owned(), ..LibraryFile::default() }),
        };
        SidecarFileStore.write_root(root, &file).expect("write");
        assert_eq!(SidecarFileStore.read_root(root).expect("read").map(|(read, _)| read), Some(file.clone()));
        let raw: Value =
            serde_json::from_slice(&fs::read(dir.path().join(".3dhoarddeck")).expect("raw")).expect("json");
        assert_eq!(raw["futureField"], 1, "unknown fields are kept");
        assert_eq!(raw["folderRules"][0]["role"], "collection", "where version 1 keeps them");

        let backup = BackupFile {
            version: FORMAT_VERSION,
            root: file,
            entries: [("a.stl.3dhoarddeck".to_owned(), EntryFile::default())].into(),
        };
        let target = dir.path().join("lib.3dhoarddeck-backup.json");
        SidecarFileStore.write_backup(&target, &backup).expect("backup");
        assert_eq!(SidecarFileStore.read_backup(&target).expect("read backup"), backup);
        assert!(SidecarFileStore.read_backup(&dir.path().join(".3dhoarddeck")).is_err(), "no backup file");
    }
}
