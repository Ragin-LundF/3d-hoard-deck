//! Browsable folders with precomputed model counts and covers.

use std::collections::BTreeMap;
use std::iter::once;

use hoarddeck_domain::{Folder, Model, RelPath, parse_folder_name};

use super::images::ImagesByDir;
use crate::scan::CompiledRules;

pub(super) fn build_folders(models: &[Model], images: &ImagesByDir<'_>, rules: &CompiledRules) -> Vec<Folder> {
    #[derive(Default)]
    struct Counts {
        direct: u32,
        total: u32,
        cover: Option<RelPath>,
    }
    let mut counts: BTreeMap<RelPath, Counts> = BTreeMap::new();
    for model in models {
        if !model.folder.is_root() {
            counts.entry(model.folder.clone()).or_default().direct += 1;
        }
        for folder in model.folder.ancestors().into_iter().chain(once(model.folder.clone())).filter(|f| !f.is_root()) {
            let entry = counts.entry(folder).or_default();
            entry.total += 1;
            if entry.cover.is_none() {
                entry.cover.clone_from(&model.cover);
            }
        }
    }
    // Like a movie library: a folder still without a cover takes a picture from somewhere below it, the nearest first.
    let mut below: BTreeMap<RelPath, (usize, RelPath)> = BTreeMap::new();
    for (dir, best) in images.best_per_dir() {
        for (distance, folder) in once(dir.clone()).chain(dir.ancestors().into_iter().rev()).enumerate() {
            if counts.contains_key(&folder) && below.get(&folder).is_none_or(|(nearest, _)| distance < *nearest) {
                below.insert(folder, (distance, best.clone()));
            }
        }
    }
    counts
        .into_iter()
        .map(|(path, counts)| {
            let parsed = parse_folder_name(path.name());
            Folder {
                // `Release.jpg` next to it, else its own pictures (an overview beside model subfolders), else a model's.
                cover: images
                    .sibling_of(&path)
                    .or_else(|| images.best_in(&path))
                    .or(counts.cover)
                    .or_else(|| below.get(&path).map(|(_, picture)| picture.clone())),
                display_name: parsed.display,
                released: parsed.released,
                direct_models: counts.direct,
                total_models: counts.total,
                is_collection: rules.is_collection(&path),
                path,
            }
        })
        .collect()
}
