use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::thread;

use hoarddeck_domain::{LibraryId, ModelId, RelPath, TagName};

use super::{AiFolders, SuggestTags};
use crate::AppError;

/// Models shown at once in the review; more wait for the next round.
const REVIEW_LIMIT: usize = 1_000;

/// One model with tags waiting for review.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewItem {
    pub id: ModelId,
    pub title: String,
    pub path: RelPath,
    pub cover: Option<RelPath>,
    pub suggested: Vec<TagName>,
    pub own: Vec<TagName>,
}

/// What the user decided for one model: suggested tags to keep, and to drop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decision {
    pub id: ModelId,
    pub keep: Vec<TagName>,
    pub dismiss: Vec<TagName>,
}

/// What a decision changed, so it can be undone: tags added to the model, tags taken out of its suggestions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolution {
    pub path: RelPath,
    pub added: Vec<TagName>,
    pub removed: Vec<TagName>,
}

impl SuggestTags {
    /// Models in `folder` (and below) with suggestions waiting, optionally only those suggested `tag`.
    /// # Errors
    /// Fails when the index cannot be read.
    pub fn review(
        &self,
        library: LibraryId,
        folder: &RelPath,
        tag: Option<&TagName>,
    ) -> Result<Vec<ReviewItem>, AppError> {
        let pending: Vec<(RelPath, Vec<TagName>)> = self
            .store
            .pending_suggestions(library)?
            .into_iter()
            .filter(|(path, tags)| {
                path.is_within(folder) && tag.is_none_or(|tag| tags.iter().any(|t| t.key() == tag.key()))
            })
            .take(REVIEW_LIMIT)
            .collect();
        let ids: HashMap<RelPath, (ModelId, String)> =
            self.models.search_documents(library)?.into_iter().map(|doc| (doc.path, (doc.id, doc.title))).collect();
        let known: Vec<ModelId> = pending.iter().filter_map(|(path, _)| ids.get(path).map(|(id, _)| *id)).collect();
        let summaries: HashMap<ModelId, _> = self.models.summaries(&known)?.into_iter().map(|s| (s.id, s)).collect();
        let mut items = Vec::new();
        for (path, suggested) in pending {
            let Some(summary) = ids.get(&path).and_then(|(id, _)| summaries.get(id)) else { continue };
            let own: Vec<TagName> = self.meta.meta(library, &path)?.tags.into_iter().collect();
            let suggested: Vec<TagName> =
                suggested.into_iter().filter(|tag| !own.iter().any(|known| known.key() == tag.key())).collect();
            if !suggested.is_empty() {
                items.push(ReviewItem {
                    id: summary.id,
                    title: summary.title.clone(),
                    cover: summary.cover.clone(),
                    path,
                    suggested,
                    own,
                });
            }
        }
        Ok(items)
    }

    /// How many models in `folder` (and below) have suggestions waiting.
    /// # Errors
    /// Fails when the index cannot be read.
    pub fn pending_count(&self, library: LibraryId, folder: &RelPath) -> Result<usize, AppError> {
        Ok(self.store.pending_suggestions(library)?.iter().filter(|(path, _)| path.is_within(folder)).count())
    }

    /// Keeps and drops suggested tags on many models at once; the search index is refreshed once.
    /// # Errors
    /// Fails when the index or a metadata file cannot be written.
    pub fn decide(&self, library: LibraryId, decisions: &[Decision]) -> Result<Vec<Resolution>, AppError> {
        let ids: Vec<ModelId> = decisions.iter().map(|decision| decision.id).collect();
        let paths: HashMap<ModelId, RelPath> =
            self.models.summaries(&ids)?.into_iter().filter(|s| s.library == library).map(|s| (s.id, s.path)).collect();
        let mut resolutions = Vec::new();
        for decision in decisions {
            let Some(path) = paths.get(&decision.id) else { continue };
            let own = self.meta.meta(library, path)?.tags;
            let added: Vec<TagName> =
                decision.keep.iter().filter(|tag| !own.iter().any(|known| known.key() == tag.key())).cloned().collect();
            let removed: Vec<TagName> = decision.keep.iter().chain(&decision.dismiss).cloned().collect();
            resolutions.push(Resolution { path: path.clone(), added, removed });
        }
        let additions: HashMap<&RelPath, &Vec<TagName>> = resolutions.iter().map(|r| (&r.path, &r.added)).collect();
        let changed: Vec<RelPath> =
            resolutions.iter().filter(|r| !r.added.is_empty()).map(|r| r.path.clone()).collect();
        self.tags.rewrite_paths(library, &changed, |path, meta| {
            meta.tags.extend(additions.get(path).into_iter().flat_map(|tags| tags.iter().cloned()));
        })?;
        for resolution in &resolutions {
            self.store.remove_suggested(library, std::slice::from_ref(&resolution.path), &resolution.removed)?;
        }
        Ok(resolutions)
    }

    /// Takes back what [`Self::decide`] did.
    /// # Errors
    /// Fails when the index or a metadata file cannot be written.
    pub fn undo(&self, library: LibraryId, resolutions: &[Resolution]) -> Result<(), AppError> {
        let added: HashMap<&RelPath, Vec<String>> =
            resolutions.iter().map(|r| (&r.path, r.added.iter().map(TagName::key).collect())).collect();
        let changed: Vec<RelPath> =
            resolutions.iter().filter(|r| !r.added.is_empty()).map(|r| r.path.clone()).collect();
        self.tags.rewrite_paths(library, &changed, |path, meta| {
            if let Some(keys) = added.get(path) {
                meta.tags.retain(|tag| !keys.contains(&tag.key()));
            }
        })?;
        for resolution in resolutions {
            self.store.restore_suggested(library, &resolution.path, &resolution.removed)?;
        }
        Ok(())
    }

    /// # Errors
    /// Fails when the index cannot be read.
    pub fn folders(&self, library: LibraryId) -> Result<AiFolders, AppError> {
        Ok(self.store.ai_folders(library)?)
    }

    /// # Errors
    /// Fails when the index cannot be written.
    pub fn save_folders(&self, library: LibraryId, folders: &AiFolders) -> Result<(), AppError> {
        Ok(self.store.save_ai_folders(library, folders)?)
    }

    /// How many models a run over the AI folders would ask about now.
    /// # Errors
    /// Fails when the index cannot be read.
    pub fn count_folders(&self, library: LibraryId, folders: &AiFolders) -> Result<usize, AppError> {
        Ok(self.folder_candidates(library, folders)?.len())
    }

    /// Starts a background run over the AI folders of `library`. Returns how many models it asks about.
    /// # Errors
    /// Fails when AI is off, a run is going on already, or the index cannot be read.
    pub fn start_folders(self: &Arc<Self>, library: LibraryId) -> Result<usize, AppError> {
        let folders = self.store.ai_folders(library)?;
        let candidates = self.folder_candidates(library, &folders)?;
        self.start_with(library, candidates)
    }

    /// Called when a scan of `library` ended: asks about new models in the AI folders when the user wants that.
    pub fn after_scan(self: &Arc<Self>, library: LibraryId) {
        let wanted = self.store.ai_folders(library).is_ok_and(|folders| folders.after_scan)
            && self.store.load_ai_settings().is_ok_and(|settings| settings.enabled())
            && !self.running.load(Ordering::SeqCst);
        if wanted {
            let this = Arc::clone(self);
            // The scan holds its library lock until it returns; asking runs apart from it.
            thread::spawn(move || {
                if let Err(error) = this.start_folders(library) {
                    eprintln!("tag suggestions after the scan: {error}");
                }
            });
        }
    }

    fn folder_candidates(&self, library: LibraryId, folders: &AiFolders) -> Result<Vec<ModelId>, AppError> {
        let ids = self.candidates_where(library, |path| folders.wants(path), folders.only_untagged)?;
        if !folders.skip_without_preview {
            return Ok(ids);
        }
        Ok(self.models.summaries(&ids)?.into_iter().filter(|s| s.cover.is_some()).map(|s| s.id).collect())
    }
}
