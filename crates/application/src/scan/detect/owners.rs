//! Which folder owns a file: a folder the user marked as a model, else the nearest non-variant folder.

use std::collections::{BTreeMap, HashMap};

use hoarddeck_domain::{Entry, FileKind, RelPath};

use crate::scan::CompiledRules;

pub(super) struct Owned<'a> {
    pub entry: &'a Entry,
    pub kind: FileKind,
    pub variant: Option<String>,
}

pub(super) struct Group<'a> {
    pub marked: bool,
    pub files: Vec<Owned<'a>>,
}

#[derive(Clone)]
struct Owner {
    path: RelPath,
    /// The user marked this folder as one model.
    marked: bool,
}

pub(super) fn group_by_owner<'a>(
    files: &[(&'a Entry, FileKind)],
    rules: &CompiledRules,
) -> BTreeMap<RelPath, Group<'a>> {
    let mut resolver = OwnerResolver { rules, owners: HashMap::new(), model_roots: HashMap::new() };
    let mut groups: BTreeMap<RelPath, Group<'a>> = BTreeMap::new();
    for &(entry, kind) in files {
        let Some(dir) = entry.path.parent() else { continue };
        let owner = resolver.owner(&dir);
        let variant = variant_label(&owner.path, &dir);
        groups
            .entry(owner.path)
            .or_insert_with(|| Group { marked: owner.marked, files: Vec::new() })
            .files
            .push(Owned { entry, kind, variant });
    }
    groups.retain(|_, group| group.files.iter().any(|file| file.kind.is_model_file()));
    groups
}

/// Finds the folder a file belongs to, with per-folder caches because files share folders.
struct OwnerResolver<'r> {
    rules: &'r CompiledRules,
    owners: HashMap<RelPath, Owner>,
    model_roots: HashMap<RelPath, Option<RelPath>>,
}

impl OwnerResolver<'_> {
    fn owner(&mut self, dir: &RelPath) -> Owner {
        if let Some(owner) = self.owners.get(dir) {
            return owner.clone();
        }
        let owner = match self.model_root(dir) {
            Some(path) => Owner { path, marked: true },
            None => Owner { path: self.variant_owner(dir), marked: false },
        };
        self.owners.insert(dir.clone(), owner.clone());
        owner
    }

    /// The shallowest folder at or above `dir` that the user marked as one model.
    fn model_root(&mut self, dir: &RelPath) -> Option<RelPath> {
        if let Some(root) = self.model_roots.get(dir) {
            return root.clone();
        }
        let from_parent = dir.parent().and_then(|parent| self.model_root(&parent));
        let root = from_parent.or_else(|| self.rules.is_model(dir).then(|| dir.clone()));
        self.model_roots.insert(dir.clone(), root.clone());
        root
    }

    /// Climbs out of variant folders, but never up to the library root.
    fn variant_owner(&self, dir: &RelPath) -> RelPath {
        let mut current = dir.clone();
        while self.rules.is_variant(current.name()) {
            match current.parent() {
                Some(parent) if !parent.is_root() => current = parent,
                _ => break,
            }
        }
        current
    }
}

fn variant_label(owner: &RelPath, dir: &RelPath) -> Option<String> {
    if owner == dir {
        return None;
    }
    let below = if owner.is_root() { dir.as_str() } else { &dir.as_str()[owner.as_str().len() + 1..] };
    Some(below.to_owned())
}
