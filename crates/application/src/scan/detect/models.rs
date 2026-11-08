//! Turns owned file groups into models: one per folder, or one per loose file.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::iter::once;

use hoarddeck_domain::{
    Entry, FileKind, Model, ModelFile, ModelKind, RelPath, YearMonth, file_title, parse_folder_name,
};

use super::images::{ImagesByDir, best_image, model_stem, stem};
use super::owners::{Group, Owned};
use crate::scan::CompiledRules;

pub(super) fn build_models(
    groups: BTreeMap<RelPath, Group<'_>>,
    images: &ImagesByDir<'_>,
    rules: &CompiledRules,
) -> Vec<Model> {
    let has_nested_models: HashSet<RelPath> = groups.keys().flat_map(RelPath::ancestors).collect();
    // Names of the model folders inside each folder: `Release.jpg` belongs to `Release/`, not to a loose file.
    let mut model_folders: HashMap<RelPath, HashSet<String>> = HashMap::new();
    for path in groups.keys() {
        if let Some(parent) = path.parent() {
            model_folders.entry(parent).or_default().insert(path.name().to_lowercase());
        }
    }
    let mut models = Vec::with_capacity(groups.len());
    for (path, group) in groups {
        let splits = path.is_root() || has_nested_models.contains(&path) || rules.is_collection(&path);
        if splits && !group.marked {
            let loose: Vec<&Owned<'_>> = group.files.iter().filter(|file| file.kind.is_model_file()).collect();
            // A single loose model file next to model folders gets the folder's other pictures, so they are not lost.
            let pictures: Vec<&Owned<'_>> = match loose[..] {
                [_] => group
                    .files
                    .iter()
                    .filter(|file| file.kind == FileKind::Image)
                    .filter(|file| {
                        let taken = model_folders.get(&path);
                        !taken.is_some_and(|names| names.contains(&stem(file.entry.path.name()).to_lowercase()))
                    })
                    .collect(),
                _ => Vec::new(),
            };
            models.extend(loose.into_iter().map(|file| file_model(file, &pictures, images)));
        } else {
            models.push(folder_model(path, group.marked, group.files, images));
        }
    }
    let mut collections = HashMap::new();
    for model in &mut models {
        model.collection = nearest_collection(&model.folder, rules, &mut collections);
    }
    models.sort_by(|a, b| a.path.cmp(&b.path));
    models
}

/// The deepest folder marked as a collection at or above `folder`, cached per folder.
fn nearest_collection(
    folder: &RelPath,
    rules: &CompiledRules,
    cache: &mut HashMap<RelPath, Option<RelPath>>,
) -> Option<RelPath> {
    if let Some(found) = cache.get(folder) {
        return found.clone();
    }
    let found = if rules.is_collection(folder) {
        Some(folder.clone())
    } else {
        folder.parent().and_then(|parent| nearest_collection(&parent, rules, cache))
    };
    cache.insert(folder.clone(), found.clone());
    found
}

fn folder_model(path: RelPath, marked_by_user: bool, owned: Vec<Owned<'_>>, images: &ImagesByDir<'_>) -> Model {
    let cover = images.sibling_of(&path).or_else(|| cover_from(&owned));
    let mut files: Vec<ModelFile> = owned
        .into_iter()
        .map(|file| ModelFile {
            path: file.entry.path.clone(),
            kind: file.kind,
            size: file.entry.size,
            variant: file.variant,
        })
        .collect();
    files.sort_by(|a, b| a.path.cmp(&b.path));
    Model {
        folder: path.parent().unwrap_or_default(),
        title: parse_folder_name(path.name()).display,
        released: released_at(&path),
        cover,
        collection: None,
        marked_by_user,
        files,
        kind: ModelKind::Folder,
        path,
    }
}

/// One loose model file; `pictures` are further images that belong to it.
fn file_model(file: &Owned<'_>, pictures: &[&Owned<'_>], images: &ImagesByDir<'_>) -> Model {
    let path = file.entry.path.clone();
    let folder = path.parent().unwrap_or_default();
    let named = images.with_stem(&folder, model_stem(path.name()));
    let mut files: Vec<ModelFile> = once(file)
        .chain(pictures.iter().copied())
        .map(|owned| ModelFile {
            path: owned.entry.path.clone(),
            kind: owned.kind,
            size: owned.entry.size,
            variant: None,
        })
        .collect();
    files.sort_by(|a, b| a.path.cmp(&b.path));
    files.dedup_by(|a, b| a.path == b.path);
    let owned_pictures: Vec<Owned<'_>> =
        pictures.iter().map(|owned| Owned { entry: owned.entry, kind: owned.kind, variant: None }).collect();
    Model {
        title: file_title(path.name()),
        released: released_at(&folder),
        cover: named.or_else(|| cover_from(&owned_pictures)),
        files,
        collection: None,
        marked_by_user: false,
        kind: ModelKind::File,
        folder,
        path,
    }
}

/// An image named like cover/preview/render wins, else the largest image.
fn cover_from(owned: &[Owned<'_>]) -> Option<RelPath> {
    let images: Vec<&Entry> = owned.iter().filter(|file| file.kind == FileKind::Image).map(|file| file.entry).collect();
    best_image(&images)
}

/// Release date from the nearest folder named like `2019-05 - X`, at or above `path`.
pub(super) fn released_at(path: &RelPath) -> Option<YearMonth> {
    once(path.clone()).chain(path.ancestors().into_iter().rev()).find_map(|dir| parse_folder_name(dir.name()).released)
}
