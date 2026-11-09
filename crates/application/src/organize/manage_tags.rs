use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;

use hoarddeck_domain::{LibraryId, ModelKind, ModelMeta, RelPath, TagName};

use super::sidecar_path;
use crate::AppError;
use crate::library::writes_sidecars;
use crate::ports::{
    LibraryRepository, MetaStore, ModelRepository, SearchIndex, SidecarFiles, TagGroup, TagInfo, TagScope, TagStore,
};
use crate::scan::LibraryLocks;
use crate::search::{SearchModels, SearchRequest};

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
                limit: FACET_MODELS,
            };
            TagScope::Models(self.search_models.execute(&request)?.models.into_iter().map(|model| model.id).collect())
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

    /// Applies `change` to the metadata of the models at `paths`, then refreshes the search index once, also when a
    /// write failed halfway, so search matches what was changed until then.
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
        let kinds: HashMap<RelPath, ModelKind> =
            self.models.search_documents(library)?.into_iter().map(|document| (document.path, document.kind)).collect();
        let written = paths.iter().try_for_each(|path| -> Result<(), AppError> {
            let mut meta = self.meta.meta(library, path)?;
            change(path, &mut meta);
            let meta = meta.normalized()?;
            self.meta.save_meta(library, path, &meta)?;
            if writes_sidecars(found.settings)
                && let Some(file) = kinds.get(path).and_then(|kind| sidecar_path(path, *kind))
            {
                self.sidecars
                    .write(&found.root, &file, &meta)
                    .map_err(|error| AppError::InvalidInput(format!("metadata file: {error}")))?;
            }
            Ok(())
        });
        let _sync = self.locks.search_sync();
        let refreshed = self.search.replace_library(library, &self.models.search_documents(library)?);
        written?;
        refreshed?;
        Ok(paths.len())
    }
}
