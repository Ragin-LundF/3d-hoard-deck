use hoarddeck_domain::{ModelKind, RelPath};

/// Name of the metadata file: inside a model folder, or next to a single-file model as `<file>.3dhoarddeck`.
const SIDECAR: &str = ".3dhoarddeck";

/// Where a model's metadata file lives.
#[must_use]
pub fn sidecar_path(model: &RelPath, kind: ModelKind) -> Option<RelPath> {
    match kind {
        ModelKind::Folder => model.join(SIDECAR).ok(),
        ModelKind::File => model.parent()?.join(&format!("{}{SIDECAR}", model.name())).ok(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folders_hold_their_file_and_single_files_get_a_neighbour() {
        let folder = RelPath::new("Minis/Dragon").expect("valid");
        assert_eq!(
            sidecar_path(&folder, ModelKind::Folder).map(String::from),
            Some("Minis/Dragon/.3dhoarddeck".to_owned())
        );
        let file = RelPath::new("Makerworld/13661.3mf").expect("valid");
        assert_eq!(
            sidecar_path(&file, ModelKind::File).map(String::from),
            Some("Makerworld/13661.3mf.3dhoarddeck".to_owned())
        );
    }
}
