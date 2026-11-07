//! Image lookup per folder, for covers.

use std::collections::HashMap;

use hoarddeck_domain::{Entry, FileKind, RelPath};

pub(super) struct ImagesByDir<'a>(HashMap<RelPath, Vec<&'a Entry>>);

impl<'a> ImagesByDir<'a> {
    pub(super) fn new(files: &[(&'a Entry, FileKind)]) -> Self {
        let mut by_dir: HashMap<RelPath, Vec<&Entry>> = HashMap::new();
        for &(entry, kind) in files {
            if let (FileKind::Image, Some(dir)) = (kind, entry.path.parent()) {
                by_dir.entry(dir).or_default().push(entry);
            }
        }
        Self(by_dir)
    }

    /// `Release.jpg` next to the folder `Release/`.
    pub(super) fn sibling_of(&self, folder: &RelPath) -> Option<RelPath> {
        self.with_stem(&folder.parent()?, folder.name())
    }

    pub(super) fn with_stem(&self, dir: &RelPath, wanted: &str) -> Option<RelPath> {
        self.0
            .get(dir)?
            .iter()
            .find(|image| stem(image.path.name()).eq_ignore_ascii_case(wanted))
            .map(|image| image.path.clone())
    }
}

/// `dragon.stl.txt` → `dragon`, so `dragon.jpg` is found as its cover.
pub(crate) fn model_stem(name: &str) -> &str {
    name.to_ascii_lowercase().find(".stl.").map_or_else(|| stem(name), |index| &name[..index])
}

pub(super) fn stem(name: &str) -> &str {
    name.rsplit_once('.').map_or(name, |(stem, _)| stem)
}
