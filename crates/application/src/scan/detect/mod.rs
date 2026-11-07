//! Groups raw entries into models and browsable folders. Pure: works on stored entries, never on disk.

mod folders;
mod images;
mod models;
mod owners;
#[cfg(test)]
mod tests;

use hoarddeck_domain::{Catalog, Entry, EntryKind, FileKind};

use super::CompiledRules;
use images::ImagesByDir;

/// Groups `entries` into a [`Catalog`]:
/// - a folder with model files is one model; variant subfolders (Supported, 75mm, images …) merge into it,
/// - model files in a folder that also has model subfolders (or is marked as a collection) become one model each,
/// - a folder the user marked as a model is one model with everything below it,
/// - a model belongs to the nearest folder marked as a collection above it.
#[must_use]
pub fn detect(entries: &[Entry], rules: &CompiledRules) -> Catalog {
    let files: Vec<(&Entry, FileKind)> = entries
        .iter()
        .filter_map(|entry| match entry.kind {
            EntryKind::File(kind) if !rules.is_ignored(&entry.path) => Some((entry, kind)),
            _ => None,
        })
        .collect();
    let images = ImagesByDir::new(&files);
    let groups = owners::group_by_owner(&files, rules);
    let models = models::build_models(groups, &images, rules);
    let folders = folders::build_folders(&models, &images, rules);
    Catalog { folders, models }
}
