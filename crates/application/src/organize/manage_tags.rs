use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;
use std::thread;

use hoarddeck_domain::{LibraryId, ModelId, ModelKind, ModelMeta, RelPath, TagName};

use super::sidecar_path;
use crate::AppError;
use crate::library::writes_sidecars;
use crate::ports::{
    LibraryRepository, MetaStore, ModelRepository, SearchIndex, SidecarFiles, TagGroup, TagInfo, TagScope, TagStore,
};
use crate::scan::LibraryLocks;
use crate::search::{SearchModels, SearchRequest};

/// `.3dhoarddeck` files written at the same time; modest, so a NAS stays usable for others.
const SIDECAR_WRITERS: usize = 4;
/// More changed models than this rebuild the library's search index instead of updating them one by one.
const FULL_REFRESH_ABOVE: usize = 2000;
/// Counting tags of a search looks at this many hits; enough to rank the chips above the grid.
const FACET_MODELS: usize = 5000;

/// Tags as a whole: counts for the tag bar, the tag browser, renaming and merging, deleting, groups and pins.
/// Renames and deletes change each model's metadata (and its `.3dhoarddeck` file where enabled), never the models.
pub struct ManageTags {
    pub libraries: Arc<dyn LibraryRepository>,
    pub models: Arc<dyn ModelRepository>,
    pub meta: Arc<dyn MetaStore>,
    pub tags: Arc<dyn TagStore>,
    pub sidecars: Arc<dyn SidecarFiles>,
    pub search: Arc<dyn SearchIndex>,
    pub search_models: Arc<SearchModels>,
    pub locks: Arc<LibraryLocks>,
    /// The last batches, so they can be undone.
    pub undo: std::sync::Mutex<super::BatchUndo>,
}

impl ManageTags {
    /// Every tag in use, most used first, with group and pin.
    /// # Errors
    /// Fails when the index cannot be read.
    pub fn list(&self, library: LibraryId) -> Result<Vec<TagInfo>, AppError> {
        Ok(self.tags.tag_infos(library)?)
    }

    /// The most used tags of what the user looks at: a folder (and below), narrowed by a search when given.
    /// # Errors
    /// Fails when the index or the search index cannot be read.
    pub fn facets(
        &self,
        library: LibraryId,
        folder: &RelPath,
        query: &str,
        limit: usize,
    ) -> Result<Vec<(TagName, u32)>, AppError> {
        let scope = if query.trim().is_empty() {
            TagScope::Folder(folder.clone())
        } else {
            let request = SearchRequest {
                text: query.to_owned(),
                library: Some(library),
                scope: (!folder.is_root()).then(|| folder.clone()),
                offset: 0,
                limit: 0,
            };
            TagScope::Models(
                self.search_models.all(&request, FACET_MODELS)?.models.into_iter().map(|model| model.id).collect(),
            )
        };
        Ok(self.tags.tag_counts(library, &scope, limit)?)
    }

    /// Renames tags, or merges several into one (`Sci-Fi` and `SciFi` into `Sci-Fi`). Returns the changed models.
    /// # Errors
    /// Fails for an unknown library, or when the index or a metadata file cannot be written.
    pub fn rename(&self, library: LibraryId, from: &[TagName], to: &TagName) -> Result<usize, AppError> {
        let keys: BTreeSet<String> = from.iter().map(TagName::key).collect();
        let changed = self.rewrite(library, from, |meta| {
            meta.tags.retain(|tag| !keys.contains(&tag.key()));
            meta.tags.insert(to.clone());
            // Rule tags cannot be renamed, so the old name is hidden on these models and the new one is their own tag.
            meta.hidden_tags.extend(from.iter().filter(|tag| tag.key() != to.key()).cloned());
            meta.hidden_tags.retain(|tag| tag.key() != to.key());
        })?;
        self.tags.move_tag_info(library, from, to)?;
        Ok(changed)
    }

    /// Removes tags from every model. Returns the changed models.
    /// # Errors
    /// Fails for an unknown library, or when the index or a metadata file cannot be written.
    pub fn delete(&self, library: LibraryId, tags: &[TagName]) -> Result<usize, AppError> {
        let keys: BTreeSet<String> = tags.iter().map(TagName::key).collect();
        let changed = self.rewrite(library, tags, |meta| {
            meta.tags.retain(|tag| !keys.contains(&tag.key()));
            // A tag from a tag rule would come back with the next regroup; hiding it removes it for good.
            meta.hidden_tags.extend(tags.iter().cloned());
        })?;
        self.tags.forget_tags(library, tags)?;
        Ok(changed)
    }

    /// # Errors
    /// Fails when the index cannot be written.
    pub fn set_group(&self, library: LibraryId, tags: &[TagName], group: Option<i64>) -> Result<(), AppError> {
        Ok(self.tags.set_tag_group(library, tags, group)?)
    }

    /// # Errors
    /// Fails when the index cannot be written.
    pub fn set_pinned(&self, library: LibraryId, tags: &[TagName], pinned: bool) -> Result<(), AppError> {
        Ok(self.tags.set_tag_pinned(library, tags, pinned)?)
    }

    /// Writes the `.3dhoarddeck` files a few at a time: each is its own file in its own folder, and on a network share
    /// one write after another spends most of the time waiting. Every file is tried; the first failure is returned.
    fn write_sidecars(&self, root: &str, files: &[(RelPath, ModelMeta)]) -> Result<(), AppError> {
        if files.is_empty() {
            return Ok(());
        }
        let chunk = files.len().div_ceil(SIDECAR_WRITERS);
        let failures: Vec<String> = thread::scope(|scope| {
            let workers: Vec<_> = files
                .chunks(chunk)
                .map(|part| {
                    scope.spawn(move || {
                        part.iter()
                            .filter_map(|(file, meta)| self.sidecars.write(root, file, meta).err())
                            .collect::<Vec<_>>()
                    })
                })
                .collect();
            workers
                .into_iter()
                .flat_map(|worker| worker.join().unwrap_or_else(|_| vec!["writer stopped".to_owned()]))
                .collect()
        });
        match failures.first() {
            None => Ok(()),
            Some(first) => {
                Err(AppError::InvalidInput(format!("metadata file: {first} ({} files failed)", failures.len())))
            }
        }
    }

    /// # Errors
    /// Fails when the index cannot be read.
    pub fn groups(&self, library: LibraryId) -> Result<Vec<TagGroup>, AppError> {
        Ok(self.tags.tag_groups(library)?)
    }

    /// Creates or changes a group. Returns its id.
    /// # Errors
    /// Fails for an empty name, or when the index cannot be written.
    pub fn save_group(&self, library: LibraryId, id: Option<i64>, name: &str, color: &str) -> Result<i64, AppError> {
        let name = name.trim();
        if name.is_empty() {
            return Err(AppError::InvalidInput("a group needs a name".to_owned()));
        }
        Ok(self.tags.save_tag_group(library, id, name, color)?)
    }

    /// # Errors
    /// Fails when the index cannot be written.
    pub fn delete_group(&self, library: LibraryId, id: i64) -> Result<(), AppError> {
        Ok(self.tags.delete_tag_group(library, id)?)
    }

    /// Applies `change` to the tags of every model carrying one of `tags`, then refreshes the search index once.
    fn rewrite(
        &self,
        library: LibraryId,
        tags: &[TagName],
        change: impl Fn(&mut ModelMeta),
    ) -> Result<usize, AppError> {
        let paths = self.tags.paths_with_tags(library, tags)?;
        self.rewrite_paths(library, &paths, |_, meta| change(meta))
    }

    /// Applies `change` to the metadata of the models at `paths` in one index transaction, writes their metadata files,
    /// then refreshes the search index once, also when a metadata file failed, so search matches the index.
    pub(crate) fn rewrite_paths(
        &self,
        library: LibraryId,
        paths: &[RelPath],
        change: impl Fn(&RelPath, &mut ModelMeta),
    ) -> Result<usize, AppError> {
        let found = self.libraries.get(library)?.ok_or(AppError::LibraryNotFound(library))?;
        if paths.is_empty() {
            return Ok(0);
        }
        let models: HashMap<RelPath, (ModelId, ModelKind)> =
            self.models.models_at(library, paths)?.into_iter().map(|(path, id, kind)| (path, (id, kind))).collect();
        let mut changed: Vec<ModelId> = Vec::new();
        let mut files: Vec<(RelPath, ModelMeta)> = Vec::new();
        let mut metas: Vec<(RelPath, ModelMeta)> = Vec::new();
        for path in paths {
            let mut meta = self.meta.meta(library, path)?;
            change(path, &mut meta);
            let meta = meta.normalized()?;
            changed.extend(models.get(path).map(|(id, _)| *id));
            if writes_sidecars(found.settings)
                && let Some(file) = models.get(path).and_then(|(_, kind)| sidecar_path(path, *kind))
            {
                files.push((file, meta.clone()));
            }
            metas.push((path.clone(), meta));
        }
        // One transaction for all models: a batch is applied whole or not at all, and quickly.
        self.meta.save_metas(library, &metas)?;
        let written = self.write_sidecars(&found.root, &files);
        let _sync = self.locks.search_sync();
        // Only the changed models go to search; rebuilding all of a big library took seconds per click.
        let refreshed: Result<(), AppError> = if changed.len() > FULL_REFRESH_ABOVE {
            self.search.replace_library(library, &self.models.search_documents(library)?).map_err(Into::into)
        } else {
            changed
                .iter()
                .map(|id| self.models.search_document(*id))
                .filter_map(Result::transpose)
                .collect::<Result<Vec<_>, _>>()
                .map_err(AppError::from)
                .and_then(|documents| self.search.update_documents(&documents).map_err(Into::into))
        };
        written?;
        refreshed?;
        Ok(paths.len())
    }
}
