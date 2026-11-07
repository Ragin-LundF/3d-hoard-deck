//! `.3dhoarddeck` files: versioned JSON beside each model. Written only for libraries where the user allowed it.

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use hoarddeck_application::ports::SidecarFiles;
use hoarddeck_domain::{ModelMeta, RelPath};
use serde_json::{Map, Value};

const VERSION: u64 = 1;
static WRITES: AtomicU64 = AtomicU64::new(0);

pub struct SidecarFileStore;

impl SidecarFiles for SidecarFileStore {
    fn write(&self, root: &str, file: &RelPath, meta: &ModelMeta) -> Result<(), String> {
        let target = absolute(root, file);
        // Keep fields a newer version wrote, so an older app never loses them.
        let mut fields: Map<String, Value> =
            fs::read(&target).ok().and_then(|bytes| serde_json::from_slice(&bytes).ok()).unwrap_or_default();
        let Value::Object(known) = serde_json::to_value(meta).map_err(|e| e.to_string())? else {
            return Err("metadata is not an object".to_owned());
        };
        fields.extend(known);
        fields.insert("version".to_owned(), Value::from(VERSION));
        let now = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
        fields.insert("updatedAt".to_owned(), Value::from(now));
        let json = serde_json::to_vec_pretty(&Value::Object(fields)).map_err(|e| e.to_string())?;
        let write = WRITES.fetch_add(1, Ordering::Relaxed);
        let temporary = target.with_file_name(format!(".3dhoarddeck.{}.{write}.tmp", std::process::id()));
        fs::write(&temporary, json).map_err(|e| format!("{}: {e}", temporary.display()))?;
        fs::rename(&temporary, &target).map_err(|e| {
            let _ = fs::remove_file(&temporary);
            format!("{}: {e}", target.display())
        })
    }

    fn read(&self, root: &str, file: &RelPath) -> Result<Option<(ModelMeta, i64)>, String> {
        let target = absolute(root, file);
        match fs::read(&target) {
            Ok(bytes) => {
                let value: Value = serde_json::from_slice(&bytes).map_err(|e| format!("{}: {e}", target.display()))?;
                let written = value.get("updatedAt").and_then(Value::as_i64).unwrap_or(0);
                let meta = serde_json::from_value(value).map_err(|e| format!("{}: {e}", target.display()))?;
                Ok(Some((meta, written)))
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(format!("{}: {error}", target.display())),
        }
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
}
