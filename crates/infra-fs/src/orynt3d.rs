//! Reads `Orynt3D`'s per-folder `config.orynt3d` files (read-only). Only `modelmeta` matters here.

use std::fs;
use std::path::PathBuf;

use hoarddeck_application::ports::{ImportedMeta, Orynt3dReader};
use hoarddeck_domain::RelPath;
use serde::Deserialize;
use serde_json::Value;

const CONFIG: &str = "config.orynt3d";

pub struct Orynt3dFiles;

#[derive(Deserialize)]
struct Config {
    #[serde(default)]
    modelmeta: ModelMeta,
}

#[derive(Deserialize, Default)]
struct ModelMeta {
    name: Option<String>,
    #[serde(default)]
    notes: String,
    #[serde(default)]
    tags: Vec<Value>,
}

impl Orynt3dReader for Orynt3dFiles {
    fn read(&self, root: &str, folder: &RelPath) -> Result<Option<ImportedMeta>, String> {
        let mut path = PathBuf::from(root);
        path.extend(folder.segments());
        path.push(CONFIG);
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(format!("{}: {error}", path.display())),
        };
        let config: Config = serde_json::from_slice(&bytes).map_err(|error| format!("{}: {error}", path.display()))?;
        Ok(Some(ImportedMeta {
            name: config.modelmeta.name.filter(|name| !name.trim().is_empty()),
            notes: config.modelmeta.notes,
            tags: config.modelmeta.tags.iter().filter_map(tag_text).collect(),
        }))
    }
}

/// Tags are plain strings in current files; objects with a `name` or `value` are accepted too.
fn tag_text(tag: &Value) -> Option<String> {
    match tag {
        Value::String(text) => Some(text.clone()),
        Value::Object(fields) => {
            fields.get("name").or_else(|| fields.get("value")).and_then(Value::as_str).map(str::to_owned)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_name_notes_and_tags_in_both_shapes() {
        let dir = tempfile::tempdir().expect("temp dir");
        fs::create_dir_all(dir.path().join("Dragon")).expect("folder");
        let config = r#"{"version":5,"scancfg":{"fileMode":0},"modelmeta":{"name":"Red Dragon","notes":"0.05 mm",
            "tags":["Resin",{"name":"Fantasy"},{"value":"Big"},42],"cover":null,"collections":[],"attributes":[]}}"#;
        fs::write(dir.path().join("Dragon/config.orynt3d"), config).expect("write");
        let root = dir.path().to_str().expect("utf-8");
        let imported =
            Orynt3dFiles.read(root, &RelPath::new("Dragon").expect("valid")).expect("read").expect("present");
        assert_eq!(imported.name.as_deref(), Some("Red Dragon"));
        assert_eq!(imported.notes, "0.05 mm");
        assert_eq!(imported.tags, vec!["Resin", "Fantasy", "Big"]);
        assert_eq!(Orynt3dFiles.read(root, &RelPath::new("Other").expect("valid")).expect("read"), None);
    }

    #[test]
    fn empty_configs_as_orynt3d_writes_them_import_nothing() {
        let dir = tempfile::tempdir().expect("temp dir");
        let config = r#"{"version":5,"modelmeta":{"name":null,"notes":"","tags":[],"cover":null,"collections":[],"attributes":[]}}"#;
        fs::write(dir.path().join("config.orynt3d"), config).expect("write");
        let imported =
            Orynt3dFiles.read(dir.path().to_str().expect("utf-8"), &RelPath::root()).expect("read").expect("present");
        assert!(imported.name.is_none() && imported.notes.is_empty() && imported.tags.is_empty());
    }
}
