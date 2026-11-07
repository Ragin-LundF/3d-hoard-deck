//! Turns owned file groups into models: one per folder, or one per loose file.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::iter::once;

use hoarddeck_domain::{
    Entry, FileKind, Model, ModelFile, ModelKind, RelPath, YearMonth, file_title, parse_folder_name,
};

use super::images::{ImagesByDir, model_stem, stem};
use super::owners::{Group, Owned};
use crate::scan::CompiledRules;

const COVER_WORDS: [&str; 4] = ["cover", "preview", "main", "render"];

pub(super) fn build_models(
    groups: BTreeMap<RelPath, Group<'_>>,
    images: &ImagesByDir<'_>,
    rules: &CompiledRules,
) -> Vec<Model> {
    let has_nested_models: HashSet<RelPath> = groups.keys().flat_map(RelPath::ancestors).collect();
    let mut models = Vec::with_capacity(groups.len());
    for (path, group) in groups {
        let splits = path.is_root() || has_nested_models.contains(&path) || rules.is_collection(&path);
        if splits && !group.marked {
            models.extend(
                group.files.iter().filter(|file| file.kind.is_model_file()).map(|file| file_model(file, images)),
            );
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

fn file_model(file: &Owned<'_>, images: &ImagesByDir<'_>) -> Model {
    let path = file.entry.path.clone();
    let folder = path.parent().unwrap_or_default();
    Model {
        title: file_title(path.name()),
        released: released_at(&folder),
        cover: images.with_stem(&folder, model_stem(path.name())),
        files: vec![ModelFile { path: path.clone(), kind: file.kind, size: file.entry.size, variant: None }],
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
    let named = images.iter().find(|image| {
        let lower = stem(image.path.name()).to_lowercase();
        COVER_WORDS.iter().any(|word| lower.starts_with(word))
    });
    named.or_else(|| images.iter().max_by_key(|image| image.size)).map(|image| image.path.clone())
}

/// Release date from the nearest folder named like `2019-05 - X`, at or above `path`.
pub(super) fn released_at(path: &RelPath) -> Option<YearMonth> {
    once(path.clone()).chain(path.ancestors().into_iter().rev()).find_map(|dir| parse_folder_name(dir.name()).released)
}
