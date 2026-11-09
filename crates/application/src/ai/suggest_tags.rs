use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Instant;

use hoarddeck_domain::{LibraryId, ModelId, RelPath, TagName};

use super::prompt::{self, VOCABULARY};
use super::{AiProvider, AiSettings};
use crate::organize::{ManageTags, UpdateModelMeta};
use crate::ports::{
    AiStore, EventSink, LibraryRepository, MetaStore, ModelRepository, PreviewImages, SecretStore, TagSuggester,
};
use crate::{AppError, ModelDetail};

const MAX_PROMPT: usize = 10_000;

/// What to do with the stored API key when saving the settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyChange {
    Keep,
    Set(String),
    Remove,
}

/// The settings as the UI shows them. The key itself never leaves the backend.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiView {
    pub settings: AiSettings,
    pub has_key: bool,
    pub key_from_environment: bool,
    pub remote: bool,
}

/// What "Test connection" found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiTest {
    pub models: Vec<String>,
    pub model: String,
    pub tags: Vec<String>,
    pub millis: u64,
}

/// Tag suggestions from an AI: settings, one model at a time or a background run over a folder, and keeping or
/// dismissing what it suggested. Suggestions are never applied without the user. Only the preview picture (a small
/// JPEG from the thumbnail cache), the name and the folder are sent; model files never are.
pub struct SuggestTags {
    pub libraries: Arc<dyn LibraryRepository>,
    pub models: Arc<dyn ModelRepository>,
    pub meta: Arc<dyn MetaStore>,
    pub store: Arc<dyn AiStore>,
    pub secrets: Arc<dyn SecretStore>,
    pub suggester: Arc<dyn TagSuggester>,
    pub previews: Arc<dyn PreviewImages>,
    pub tags: Arc<ManageTags>,
    pub update: Arc<UpdateModelMeta>,
    pub events: Arc<dyn EventSink>,
    pub running: AtomicBool,
    pub cancelled: AtomicBool,
}

impl SuggestTags {
    /// # Errors
    /// Fails when the index cannot be read.
    pub fn settings(&self) -> Result<AiView, AppError> {
        let settings = self.store.load_ai_settings()?;
        let stored = self.secrets.secret(settings.provider.code()).ok().flatten().is_some();
        let environment =
            settings.provider.key_variable().is_some_and(|name| std::env::var(name).is_ok_and(|v| !v.is_empty()));
        let remote = settings.provider.is_remote(&settings.url());
        Ok(AiView { has_key: stored, key_from_environment: !stored && environment, remote, settings })
    }

    /// # Errors
    /// Fails for invalid settings, or when the index or the keychain cannot be written.
    pub fn save_settings(&self, settings: &AiSettings, key: KeyChange) -> Result<(), AppError> {
        validate(settings)?;
        let name = settings.provider.code();
        match key {
            KeyChange::Keep => {}
            KeyChange::Set(key) if !key.trim().is_empty() => {
                self.secrets.set_secret(name, Some(key.trim())).map_err(keychain)?;
            }
            KeyChange::Set(_) | KeyChange::Remove => self.secrets.set_secret(name, None).map_err(keychain)?,
        }
        Ok(self.store.save_ai_settings(settings)?)
    }

    /// Lists the server's models and asks one question without a picture, with `key` or the stored key.
    /// # Errors
    /// Fails for invalid settings or when the server cannot be used, with the server's reason.
    pub fn test(&self, settings: &AiSettings, key: Option<String>) -> Result<AiTest, AppError> {
        validate(settings)?;
        let key = key.filter(|key| !key.trim().is_empty()).or_else(|| self.key(settings.provider));
        let models = self.suggester.models(settings, key.as_deref()).map_err(AppError::Ai)?;
        let started = Instant::now();
        let request = prompt::request(settings, "Red dragon bust", "Miniatures/Dragons", &[], None);
        let answer = self.suggester.suggest(settings, key.as_deref(), &request).map_err(AppError::Ai)?;
        let tags = prompt::clean(&answer.tags, &[], &[], usize::from(settings.max_tags));
        Ok(AiTest {
            models,
            model: answer.model,
            tags: tags.into_iter().map(String::from).collect(),
            millis: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
        })
    }

    /// Asks the AI about one model now and stores the answer.
    /// # Errors
    /// Fails when AI is off, the model is unknown, or the server gives no answer.
    pub fn suggest_model(&self, id: ModelId) -> Result<Vec<TagName>, AppError> {
        let settings = self.enabled_settings()?;
        let detail = self.models.get(id)?.ok_or(AppError::InvalidInput("unknown model".to_owned()))?;
        let library =
            self.libraries.get(detail.summary.library)?.ok_or(AppError::LibraryNotFound(detail.summary.library))?;
        let vocabulary = self.vocabulary(library.id)?;
        let key = self.key(settings.provider);
        let (tags, source) =
            self.ask(&settings, key.as_deref(), &library.root, &detail, &vocabulary).map_err(AppError::Ai)?;
        self.store.save_suggestion(library.id, &detail.summary.path, &tags, &source)?;
        Ok(tags)
    }

    /// The suggestions for a model that it does not carry yet.
    /// # Errors
    /// Fails for an unknown model or when the index cannot be read.
    pub fn suggestions(&self, id: ModelId) -> Result<Vec<TagName>, AppError> {
        let detail = self.models.get(id)?.ok_or(AppError::InvalidInput("unknown model".to_owned()))?;
        let Some((tags, _)) = self.store.suggestion(detail.summary.library, &detail.summary.path)? else {
            return Ok(Vec::new());
        };
        let own: Vec<String> = detail.meta.tags.iter().map(TagName::key).collect();
        Ok(tags.into_iter().filter(|tag| !own.contains(&tag.key())).collect())
    }

    /// Keeps a suggested tag as a normal tag, or dismisses it.
    /// # Errors
    /// Fails for an unknown model or when the index cannot be written.
    pub fn resolve(&self, id: ModelId, tag: &TagName, keep: bool) -> Result<(), AppError> {
        let detail = self.models.get(id)?.ok_or(AppError::InvalidInput("unknown model".to_owned()))?;
        if keep {
            let mut meta = detail.meta;
            meta.tags.insert(tag.clone());
            self.update.execute(id, meta)?;
        }
        self.store.remove_suggested(detail.summary.library, &[detail.summary.path], std::slice::from_ref(tag))?;
        Ok(())
    }

    /// Starts a background run over the models in `folder` (and below) that have no suggestions yet; for the whole
    /// library only models without tags of their own. Returns how many models it will ask about.
    /// # Errors
    /// Fails when AI is off, a run is going on already, or the index cannot be read.
    pub fn start(self: &Arc<Self>, library: LibraryId, folder: &RelPath) -> Result<usize, AppError> {
        let settings = self.enabled_settings()?;
        let found = self.libraries.get(library)?.ok_or(AppError::LibraryNotFound(library))?;
        if self.running.swap(true, Ordering::SeqCst) {
            return Err(AppError::InvalidInput("tags are being suggested already".to_owned()));
        }
        let candidates = match self.candidates(library, folder) {
            Ok(candidates) => candidates,
            Err(error) => {
                self.running.store(false, Ordering::SeqCst);
                return Err(error);
            }
        };
        let total = candidates.len();
        self.cancelled.store(false, Ordering::SeqCst);
        let this = Arc::clone(self);
        thread::spawn(move || this.run(&settings, &found.root, library, &candidates));
        Ok(total)
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }

    /// Every suggested tag in a library with the number of models, for the review.
    /// # Errors
    /// Fails when the index cannot be read.
    pub fn counts(&self, library: LibraryId) -> Result<Vec<(TagName, u32)>, AppError> {
        Ok(self.store.suggestion_counts(library)?)
    }

    /// Keeps one suggested tag on every model it was suggested for. Returns the number of models.
    /// # Errors
    /// Fails when the index or a metadata file cannot be written.
    pub fn accept_all(&self, library: LibraryId, tag: &TagName) -> Result<usize, AppError> {
        let paths = self.store.paths_with_suggestion(library, tag)?;
        let changed = self.tags.rewrite_paths(library, &paths, |tags| {
            if !tags.iter().any(|known| known.key() == tag.key()) {
                tags.insert(tag.clone());
            }
        })?;
        self.store.remove_suggested(library, &paths, std::slice::from_ref(tag))?;
        Ok(changed)
    }

    /// Dismisses one suggested tag everywhere. Returns the number of models.
    /// # Errors
    /// Fails when the index cannot be written.
    pub fn dismiss_all(&self, library: LibraryId, tag: &TagName) -> Result<usize, AppError> {
        let paths = self.store.paths_with_suggestion(library, tag)?;
        self.store.remove_suggested(library, &paths, std::slice::from_ref(tag))?;
        Ok(paths.len())
    }

    fn run(&self, settings: &AiSettings, root: &str, library: LibraryId, candidates: &[ModelId]) {
        let key = self.key(settings.provider);
        let vocabulary = self.vocabulary(library).unwrap_or_default();
        let (total, mut failed) = (candidates.len(), 0);
        self.events.suggest_progress(0, total, 0, true);
        for (done, id) in candidates.iter().enumerate() {
            if self.cancelled.load(Ordering::SeqCst) {
                break;
            }
            let answer = self
                .models
                .get(*id)
                .map_err(|error| error.to_string())
                .and_then(|detail| detail.ok_or_else(|| "model is gone".to_owned()))
                .and_then(|detail| {
                    let (tags, source) = self.ask(settings, key.as_deref(), root, &detail, &vocabulary)?;
                    self.store.save_suggestion(library, &detail.summary.path, &tags, &source).map_err(|e| e.to_string())
                });
            if let Err(error) = answer {
                // shortcut: reasons are only logged; the UI shows how many failed.
                eprintln!("no tag suggestion for model {}: {error}", id.0);
                failed += 1;
            }
            self.events.suggest_progress(done + 1, total, failed, true);
        }
        self.running.store(false, Ordering::SeqCst);
        self.events.suggest_progress(total, total, failed, false);
    }

    fn candidates(&self, library: LibraryId, folder: &RelPath) -> Result<Vec<ModelId>, AppError> {
        let mut documents: Vec<_> =
            self.models.search_documents(library)?.into_iter().filter(|doc| doc.path.is_within(folder)).collect();
        documents.sort_by(|a, b| a.path.cmp(&b.path));
        let mut ids = Vec::new();
        for document in documents {
            if self.store.suggestion(library, &document.path)?.is_some() {
                continue;
            }
            if folder.is_root() && !self.meta.meta(library, &document.path)?.tags.is_empty() {
                continue;
            }
            ids.push(document.id);
        }
        Ok(ids)
    }

    fn ask(
        &self,
        settings: &AiSettings,
        key: Option<&str>,
        root: &str,
        detail: &ModelDetail,
        vocabulary: &[TagName],
    ) -> Result<(Vec<TagName>, String), String> {
        let summary = &detail.summary;
        let image = summary
            .cover
            .as_ref()
            .filter(|_| settings.send_image)
            .and_then(|cover| self.previews.preview_jpeg(&Path::new(root).join(cover.as_str())).ok());
        let request = prompt::request(settings, &summary.title, summary.folder.as_str(), vocabulary, image);
        let answer = self.suggester.suggest(settings, key, &request)?;
        let existing: Vec<TagName> = detail.meta.tags.iter().chain(&detail.auto_tags).cloned().collect();
        let tags = prompt::clean(&answer.tags, vocabulary, &existing, usize::from(settings.max_tags));
        Ok((tags, format!("{}:{}", settings.provider.code(), answer.model)))
    }

    fn vocabulary(&self, library: LibraryId) -> Result<Vec<TagName>, AppError> {
        Ok(self.tags.list(library)?.into_iter().take(VOCABULARY).map(|info| info.name).collect())
    }

    fn enabled_settings(&self) -> Result<AiSettings, AppError> {
        let settings = self.store.load_ai_settings()?;
        if settings.enabled() {
            Ok(settings)
        } else {
            Err(AppError::InvalidInput("AI is switched off in Settings".to_owned()))
        }
    }

    /// The stored key, else the provider's environment variable.
    fn key(&self, provider: AiProvider) -> Option<String> {
        self.secrets
            .secret(provider.code())
            .ok()
            .flatten()
            .or_else(|| provider.key_variable().and_then(|name| std::env::var(name).ok()))
            .filter(|key| !key.is_empty())
    }
}

#[allow(clippy::needless_pass_by_value)] // used as `map_err(keychain)`
fn keychain(reason: String) -> AppError {
    AppError::InvalidInput(format!("the keychain could not be used: {reason}"))
}

fn validate(settings: &AiSettings) -> Result<(), AppError> {
    let invalid = |reason: &str| Err(AppError::InvalidInput(reason.to_owned()));
    if !(1..=10).contains(&settings.max_tags) {
        return invalid("tags per model must be 1 to 10");
    }
    if !(5..=600).contains(&settings.timeout_seconds) {
        return invalid("the timeout must be 5 to 600 seconds");
    }
    if settings.prompt.len() > MAX_PROMPT {
        return invalid("the instructions are too long");
    }
    let url = settings.url();
    if settings.enabled() && !(url.starts_with("http://") || url.starts_with("https://")) {
        return invalid("the server address must start with http:// or https://");
    }
    Ok(())
}
