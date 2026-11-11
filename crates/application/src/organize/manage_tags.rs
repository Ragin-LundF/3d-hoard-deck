use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;

use hoarddeck_domain::{LibraryId, ModelId, ModelKind, ModelMeta, RelPath, TagName};

use crate::AppError;
use crate::ports::{LibraryRepository, MetaStore, ModelRepository, SearchIndex, TagGroup, TagInfo, TagScope, TagStore};
use crate::scan::LibraryLocks;
use crate::search::{SearchModels, SearchRequest};

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
    pub sync: Arc<crate::sidecar::SidecarSync>,
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
        self.sync.library_changed(library);
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
        self.sync.library_changed(library);
        Ok(changed)
    }

    /// # Errors
    /// Fails when the index cannot be written.
    pub fn set_group(&self, library: LibraryId, tags: &[TagName], group: Option<i64>) -> Result<(), AppError> {
        self.tags.set_tag_group(library, tags, group)?;
        self.sync.library_changed(library);
        Ok(())
    }

    /// # Errors
    /// Fails when the index cannot be written.
    pub fn set_pinned(&self, library: LibraryId, tags: &[TagName], pinned: bool) -> Result<(), AppError> {
        self.tags.set_tag_pinned(library, tags, pinned)?;
        self.sync.library_changed(library);
        Ok(())
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
        let id = self.tags.save_tag_group(library, id, name, color)?;
        self.sync.library_changed(library);
        Ok(id)
    }

    /// # Errors
    /// Fails when the index cannot be written.
    pub fn delete_group(&self, library: LibraryId, id: i64) -> Result<(), AppError> {
        self.tags.delete_tag_group(library, id)?;
        self.sync.library_changed(library);
        Ok(())
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
        let mut metas: Vec<(RelPath, ModelMeta)> = Vec::new();
        for path in paths {
            let mut meta = self.meta.meta(library, path)?;
            change(path, &mut meta);
            let meta = meta.normalized()?;
            changed.extend(models.get(path).map(|(id, _)| *id));
            metas.push((path.clone(), meta));
        }
        // One transaction for all models: a batch is applied whole or not at all, and quickly.
        self.meta.save_metas(library, &metas)?;
        // A file that cannot be written waits in the queue; the index change stands.
        self.sync.paths_changed(found.id, paths);
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
        refreshed?;
        Ok(paths.len())
    }
}
