//! Turns owned file groups into models: one per folder, or one per loose file.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::iter::once;

use hoarddeck_domain::{
    CoverSource, Entry, FileFormat, FileKind, Model, ModelFile, ModelKind, RelPath, YearMonth, file_title,
    parse_folder_name,
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
    fill_missing_covers(&mut models, images, rules);
    models.sort_by(|a, b| a.path.cmp(&b.path));
    models
}

/// Models without a picture take the preview inside their 3MF, else one from the folder above: the model's own
/// folder for a loose file, the parent folder for a folder model, one more level only if that has none, never the
/// library root. Pictures named after another model and other models' own covers stay theirs. A borrowed picture is
/// only the cover, never one of the model's files.
fn fill_missing_covers(models: &mut [Model], images: &ImagesByDir<'_>, rules: &CompiledRules) {
    let mut taken_names: HashMap<RelPath, HashSet<String>> = HashMap::new();
    for model in models.iter() {
        let name = match model.kind {
            ModelKind::Folder => model.path.name().to_lowercase(),
            ModelKind::File => model_stem(model.path.name()).to_lowercase(),
        };
        taken_names.entry(model.folder.clone()).or_default().insert(name);
    }
    let own_covers: HashSet<RelPath> = models.iter().filter_map(|model| model.cover.clone()).collect();
    for model in models.iter_mut().filter(|model| model.cover.is_none()) {
        let parent =
            if rules.parent_covers { parent_picture(model, images, rules, &taken_names, &own_covers) } else { None };
        // Whether the 3MF really holds a preview is only known when it is opened, so the folder picture stays ready.
        let embedded = model.files.iter().find(|file| file.kind == FileKind::Mesh(FileFormat::ThreeMf));
        if let Some(file) = embedded {
            model.cover = Some(file.path.clone());
            model.cover_source = CoverSource::Embedded;
            model.fallback_cover = parent;
        } else if parent.is_some() {
            model.cover = parent;
            model.cover_source = CoverSource::Parent;
        }
    }
}

/// The best free picture one folder above the model, or two if the first has none; never the library root.
fn parent_picture(
    model: &Model,
    images: &ImagesByDir<'_>,
    rules: &CompiledRules,
    taken_names: &HashMap<RelPath, HashSet<String>>,
    own_covers: &HashSet<RelPath>,
) -> Option<RelPath> {
    let first = match model.kind {
        ModelKind::Folder => model.path.parent(),
        ModelKind::File => Some(model.folder.clone()),
    };
    let levels = first.iter().cloned().chain(first.as_ref().and_then(RelPath::parent)).filter(|dir| !dir.is_root());
    for dir in levels {
        let names = taken_names.get(&dir);
        let free = |entry: &Entry| {
            let name = stem(entry.path.name()).to_lowercase();
            !own_covers.contains(&entry.path) && !names.is_some_and(|taken| taken.contains(&name))
        };
        if let Some(picture) = images.best_for_folder(&dir, |name| rules.is_variant(name), free) {
            return Some(picture);
        }
    }
    None
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
        auto_tags: Vec::new(),
        cover_source: hoarddeck_domain::CoverSource::Own,
        fallback_cover: None,
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
        auto_tags: Vec::new(),
        cover_source: hoarddeck_domain::CoverSource::Own,
        fallback_cover: None,
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
