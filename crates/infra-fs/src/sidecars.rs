//! `.3dhoarddeck` files: versioned JSON beside each model. Written only for libraries where the user allowed it.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use hoarddeck_application::ports::SidecarFiles;
use hoarddeck_application::scan::FolderRule;
use hoarddeck_domain::{ModelMeta, RelPath};
use serde_json::{Map, Value};

const VERSION: u64 = 1;
const SIDECAR_NAME: &str = ".3dhoarddeck";
static WRITES: AtomicU64 = AtomicU64::new(0);

pub struct SidecarFileStore;

/// Holds the folder rules in the `.3dhoarddeck` at the library root.
const FOLDER_RULES: &str = "folderRules";

impl SidecarFiles for SidecarFileStore {
    fn write(&self, root: &str, file: &RelPath, meta: &ModelMeta) -> Result<(), String> {
        let Value::Object(known) = serde_json::to_value(meta).map_err(|e| e.to_string())? else {
            return Err("metadata is not an object".to_owned());
        };
        merge_write(&absolute(root, file), known)
    }

    fn read(&self, root: &str, file: &RelPath) -> Result<Option<(ModelMeta, i64)>, String> {
        let Some(value) = read_json(&absolute(root, file))? else { return Ok(None) };
        let written = value.get("updatedAt").and_then(Value::as_i64).unwrap_or(0);
        let meta = serde_json::from_value(value).map_err(|e| format!("{root}/{file}: {e}"))?;
        Ok(Some((meta, written)))
    }

    fn write_folder_rules(&self, root: &str, rules: &[FolderRule]) -> Result<(), String> {
        let rules = serde_json::to_value(rules).map_err(|e| e.to_string())?;
        merge_write(
            &absolute(root, &RelPath::root()).join(SIDECAR_NAME),
            Map::from_iter([(FOLDER_RULES.to_owned(), rules)]),
        )
    }

    fn read_folder_rules(&self, root: &str) -> Result<Option<Vec<FolderRule>>, String> {
        let target = absolute(root, &RelPath::root()).join(SIDECAR_NAME);
        let Some(mut value) = read_json(&target)? else { return Ok(None) };
        value
            .get_mut(FOLDER_RULES)
            .map(Value::take)
            .map(|rules| serde_json::from_value(rules).map_err(|e| format!("{}: {e}", target.display())))
            .transpose()
    }
}

/// Writes atomically and keeps fields a newer version wrote, so an older app never loses them.
fn merge_write(target: &Path, known: Map<String, Value>) -> Result<(), String> {
    let mut fields: Map<String, Value> =
        fs::read(target).ok().and_then(|bytes| serde_json::from_slice(&bytes).ok()).unwrap_or_default();
    fields.extend(known);
    fields.insert("version".to_owned(), Value::from(VERSION));
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
    fields.insert("updatedAt".to_owned(), Value::from(now));
    let json = serde_json::to_vec_pretty(&Value::Object(fields)).map_err(|e| e.to_string())?;
    let write = WRITES.fetch_add(1, Ordering::Relaxed);
    let temporary = target.with_file_name(format!(".3dhoarddeck.{}.{write}.tmp", std::process::id()));
    fs::write(&temporary, json).map_err(|e| format!("{}: {e}", temporary.display()))?;
    fs::rename(&temporary, target).map_err(|e| {
        let _ = fs::remove_file(&temporary);
        format!("{}: {e}", target.display())
    })
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
    use hoarddeck_domain::TagName;

    use super::*;

    #[test]
    fn writes_round_trip_and_keep_unknown_fields() {
        let dir = tempfile::tempdir().expect("temp dir");
        let root = dir.path().to_str().expect("utf-8");
        fs::create_dir_all(dir.path().join("Dragon")).expect("model folder");
        let file = RelPath::new("Dragon/.3dhoarddeck").expect("valid");
        fs::write(dir.path().join("Dragon/.3dhoarddeck"), r#"{"futureField": 42, "tags": ["old"]}"#).expect("seed");

        let meta = ModelMeta {
            display_name: Some("Red Dragon".to_owned()),
            tags: [TagName::new("Resin").expect("valid")].into_iter().collect(),
            favorite: true,
            ..ModelMeta::default()
        };
        SidecarFileStore.write(root, &file, &meta).expect("write");
        let (read, written) = SidecarFileStore.read(root, &file).expect("read").expect("present");
        assert_eq!(read, meta);
        assert!(written > 0, "updatedAt is recorded");
        let raw: Value =
            serde_json::from_slice(&fs::read(dir.path().join("Dragon/.3dhoarddeck")).expect("raw")).expect("json");
        assert_eq!(raw["futureField"], 42);
        assert_eq!(raw["version"], 1);
        assert_eq!(
            SidecarFileStore.read(root, &RelPath::new("None/.3dhoarddeck").expect("valid")).expect("read"),
            None
        );
    }

    #[test]
    fn folder_rules_live_in_the_root_file() {
        use hoarddeck_application::scan::FolderRole;

        let dir = tempfile::tempdir().expect("temp dir");
        let root = dir.path().to_str().expect("utf-8");
        assert_eq!(SidecarFileStore.read_folder_rules(root).expect("read"), None);
        fs::write(dir.path().join(".3dhoarddeck"), r#"{"futureField": 1}"#).expect("seed");
        let rules = vec![
            FolderRule::for_folder(&RelPath::new("Minis/Dragon").expect("valid"), FolderRole::Model),
            FolderRule { pattern: "**/Loot Studios/*/*".to_owned(), role: FolderRole::Collection },
        ];
        SidecarFileStore.write_folder_rules(root, &rules).expect("write");
        assert_eq!(SidecarFileStore.read_folder_rules(root).expect("read"), Some(rules));
        let raw: Value =
            serde_json::from_slice(&fs::read(dir.path().join(".3dhoarddeck")).expect("raw")).expect("json");
        assert_eq!(raw["futureField"], 1, "unknown fields are kept");
    }
}
