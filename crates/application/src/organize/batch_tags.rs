use std::collections::{BTreeSet, HashMap};
use std::sync::{Mutex, PoisonError};

use hoarddeck_domain::{LibraryId, ModelId, RelPath, TagName};

use super::ManageTags;
use crate::AppError;
use crate::search::SearchRequest;

/// More results than this are not changed in one batch.
const MAX_BATCH: usize = 50_000;
/// Undo steps kept; older batches can no longer be undone.
const UNDO_STEPS: usize = 20;

/// Which models a batch changes: picked ones, or every result of a search (also those not loaded on screen).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BatchTarget {
    Models(Vec<ModelId>),
    Search { text: String, folder: RelPath },
}

/// Where a model's tag comes from: typed (or kept from an AI suggestion), a tag rule, or both.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TagSource {
    Own,
    Rule,
    Both,
}

/// One model carrying the tag being checked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckItem {
    pub id: ModelId,
    pub title: String,
    pub path: RelPath,
    pub folder: RelPath,
    pub cover: Option<RelPath>,
    pub source: TagSource,
}

/// Each model's own and hidden tags before one batch.
type Before = Vec<(RelPath, Tags)>;
/// Own tags and hidden rule tags: all a batch changes.
type Tags = (BTreeSet<TagName>, BTreeSet<TagName>);

/// The tags of each model before a batch, kept so the batch can be undone.
#[derive(Debug, Default)]
pub struct BatchUndo {
    next: u64,
    steps: Vec<(u64, LibraryId, Before)>,
}

impl ManageTags {
    /// Adds and removes tags on many models in one index transaction and one search update. Removing a tag that a
    /// tag rule gives hides it on those models, so the next regroup does not bring it back. Returns the number of
    /// models and a token for [`Self::undo_batch`].
    /// # Errors
    /// Fails for invalid input, too many models, or when the index or a metadata file cannot be written.
    pub fn batch(
        &self,
        library: LibraryId,
        target: &BatchTarget,
        add: &[TagName],
        remove: &[TagName],
    ) -> Result<(usize, u64), AppError> {
        let paths = self.target_paths(library, target)?;
        let added: Vec<String> = add.iter().map(TagName::key).collect();
        let removed: Vec<String> = remove.iter().map(TagName::key).collect();
        let before: Mutex<Before> = Mutex::default();
        let changed = self.rewrite_paths(library, &paths, |path, meta| {
            let tags = (meta.tags.clone(), meta.hidden_tags.clone());
            before.lock().unwrap_or_else(PoisonError::into_inner).push((path.clone(), tags));
            meta.tags.retain(|tag| !removed.contains(&tag.key()));
            meta.hidden_tags.retain(|tag| !added.contains(&tag.key()));
            meta.hidden_tags.extend(remove.iter().filter(|tag| !added.contains(&tag.key())).cloned());
            meta.tags.extend(add.iter().cloned());
        })?;
        let mut undo = self.undo.lock().unwrap_or_else(PoisonError::into_inner);
        undo.next += 1;
        let token = undo.next;
        undo.steps.push((token, library, before.into_inner().unwrap_or_else(PoisonError::into_inner)));
        if undo.steps.len() > UNDO_STEPS {
            undo.steps.remove(0);
        }
        Ok((changed, token))
    }

    /// Puts the tags of a batch's models back as they were before it; other changes made since (favorite, notes, …)
    /// stay. Returns the number of models.
    /// # Errors
    /// Fails for an unknown or too old token, or when the index or a metadata file cannot be written.
    pub fn undo_batch(&self, token: u64) -> Result<usize, AppError> {
        let (library, before) = {
            let mut undo = self.undo.lock().unwrap_or_else(PoisonError::into_inner);
            let index = undo
                .steps
                .iter()
                .position(|(step, _, _)| *step == token)
                .ok_or_else(|| AppError::InvalidInput("this change can no longer be undone".to_owned()))?;
            let (_, library, before) = undo.steps.remove(index);
            (library, before)
        };
        let paths: Vec<RelPath> = before.iter().map(|(path, _)| path.clone()).collect();
        let tags: HashMap<RelPath, Tags> = before.into_iter().collect();
        self.rewrite_paths(library, &paths, |path, meta| {
            if let Some((own, hidden)) = tags.get(path) {
                meta.tags.clone_from(own);
                meta.hidden_tags.clone_from(hidden);
            }
        })
    }

    /// The models in `folder` (and below) carrying `tag`, with where it comes from, for checking it model by model.
    /// # Errors
    /// Fails when the index or the search index cannot be read.
    pub fn check(&self, library: LibraryId, tag: &TagName, folder: &RelPath) -> Result<Vec<CheckItem>, AppError> {
        let request = SearchRequest {
            text: format!("tag:\"{}\"", tag.as_str()),
            library: Some(library),
            scope: (!folder.is_root()).then(|| folder.clone()),
            offset: 0,
            limit: 0,
        };
        let key = tag.key();
        let mut items = Vec::new();
        for model in self.search_models.all(&request, MAX_BATCH)?.models {
            let Some(detail) = self.models.get(model.id)? else { continue };
            let own = detail.meta.tags.iter().any(|known| known.key() == key);
            let hidden = detail.meta.hidden_tags.iter().any(|known| known.key() == key);
            let rule = !hidden && detail.auto_tags.iter().any(|known| known.key() == key);
            let source = match (own, rule) {
                (true, true) => TagSource::Both,
                (false, true) => TagSource::Rule,
                (true, false) => TagSource::Own,
                (false, false) => continue,
            };
            items.push(CheckItem {
                id: model.id,
                title: model.title,
                folder: model.folder,
                cover: model.cover,
                path: model.path,
                source,
            });
        }
        Ok(items)
    }

    /// Tags of the given models with how many carry each (rule tags included, hidden ones not).
    /// # Errors
    /// Fails when the index cannot be read.
    pub fn counts_of(&self, library: LibraryId, ids: Vec<ModelId>) -> Result<Vec<(TagName, u32)>, AppError> {
        Ok(self.tags.tag_counts(library, &crate::ports::TagScope::Models(ids), 200)?)
    }

    fn target_paths(&self, library: LibraryId, target: &BatchTarget) -> Result<Vec<RelPath>, AppError> {
        let summaries = match target {
            BatchTarget::Models(ids) => self.models.summaries(ids)?,
            BatchTarget::Search { text, folder } => {
                let request = SearchRequest {
                    text: text.clone(),
                    library: Some(library),
                    scope: (!folder.is_root()).then(|| folder.clone()),
                    offset: 0,
                    limit: 0,
                };
                self.search_models.all(&request, MAX_BATCH + 1)?.models
            }
        };
        if summaries.len() > MAX_BATCH {
            return Err(AppError::InvalidInput(format!("at most {MAX_BATCH} models can be changed at once")));
        }
        Ok(summaries.into_iter().filter(|summary| summary.library == library).map(|summary| summary.path).collect())
    }
}
