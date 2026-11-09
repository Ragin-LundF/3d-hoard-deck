//! Image lookup per folder, for covers.

use std::collections::HashMap;

use hoarddeck_domain::{Entry, FileKind, RelPath};

pub(super) struct ImagesByDir<'a>(HashMap<RelPath, Vec<&'a Entry>>, HashMap<RelPath, Vec<RelPath>>);

impl<'a> ImagesByDir<'a> {
    pub(super) fn new(files: &[(&'a Entry, FileKind)]) -> Self {
        let mut by_dir: HashMap<RelPath, Vec<&Entry>> = HashMap::new();
        for &(entry, kind) in files {
            if let (FileKind::Image, Some(dir)) = (kind, entry.path.parent()) {
                by_dir.entry(dir).or_default().push(entry);
            }
        }
        // Picture folders by their parent, so `Pack/images/` is found from `Pack`.
        let mut children: HashMap<RelPath, Vec<RelPath>> = HashMap::new();
        for dir in by_dir.keys() {
            if let Some(parent) = dir.parent() {
                children.entry(parent).or_default().push(dir.clone());
            }
        }
        Self(by_dir, children)
    }

    /// The best picture of a folder for models below it: lying in `dir` or in its picture subfolders (`images/`,
    /// `renders/` …), leaving out pictures `free` refuses (named after another model, or someone's own cover).
    pub(super) fn best_for_folder(
        &self,
        dir: &RelPath,
        is_variant: impl Fn(&str) -> bool,
        free: impl Fn(&Entry) -> bool,
    ) -> Option<RelPath> {
        let direct = self.0.get(dir).into_iter().flatten();
        let below = self.1.get(dir).into_iter().flatten().filter(|child| is_variant(child.name()));
        let candidates: Vec<&Entry> = direct
            .chain(below.flat_map(|child| self.0.get(child).into_iter().flatten()))
            .copied()
            .filter(|entry| free(entry))
            .collect();
        best_image(&candidates)
    }

    /// Every folder that holds pictures, with the best of them.
    pub(super) fn best_per_dir(&self) -> impl Iterator<Item = (&RelPath, RelPath)> {
        self.0.iter().filter_map(|(dir, images)| best_image(images).map(|best| (dir, best)))
    }

    /// The best of the pictures lying directly in `dir`.
    pub(super) fn best_in(&self, dir: &RelPath) -> Option<RelPath> {
        best_image(self.0.get(dir)?)
    }

    /// `Release.jpg` next to the folder `Release/`.
    pub(super) fn sibling_of(&self, folder: &RelPath) -> Option<RelPath> {
        self.with_stem(&folder.parent()?, folder.name())
    }

    /// `dragon.png` for `dragon.stl`, else a screenshot the app saved as `preview_dragon.png`.
    pub(super) fn with_stem(&self, dir: &RelPath, wanted: &str) -> Option<RelPath> {
        let images = self.0.get(dir)?;
        let named = |name: &str| images.iter().find(|image| stem(image.path.name()).eq_ignore_ascii_case(name));
        named(wanted).or_else(|| named(&format!("preview_{wanted}"))).map(|image| image.path.clone())
    }
}

/// Image names that mark the picture meant as the cover.
/// `folder` and `poster` are what movie libraries use, and what a copied folder cover is named.
const COVER_WORDS: [&str; 6] = ["cover", "preview", "main", "render", "folder", "poster"];

/// An image named like cover/preview/render wins, else the largest image.
pub(super) fn best_image(images: &[&Entry]) -> Option<RelPath> {
    let named = images.iter().find(|image| {
        let lower = stem(image.path.name()).to_lowercase();
        COVER_WORDS.iter().any(|word| lower.starts_with(word))
    });
    named.or_else(|| images.iter().max_by_key(|image| image.size)).map(|image| image.path.clone())
}

/// `dragon.stl.txt` → `dragon`, so `dragon.jpg` is found as its cover.
pub(crate) fn model_stem(name: &str) -> &str {
    name.to_ascii_lowercase().find(".stl.").map_or_else(|| stem(name), |index| &name[..index])
}

pub(super) fn stem(name: &str) -> &str {
    name.rsplit_once('.').map_or(name, |(stem, _)| stem)
}
