//! Ports: what the use cases need from the outside world. Adapters live in the `infra-*` crates.

use std::path::Path;
use std::sync::mpsc::SyncSender;
use std::time::Duration;

use hoarddeck_domain::{
    Catalog, Collection, CollectionId, Entry, Folder, Library, LibraryId, LibrarySettings, ModelId, ModelKind,
    ModelMeta, RelPath, Slicer, SlicerList, SmartSearch, TagName,
};

use crate::browse::{BrowsePage, BrowseRequest};
use crate::scan::{CompiledRules, FolderRule, Generation, KnownDirs, ScanItem, ScanProgress, ScanRules};
use crate::search::{SearchDocument, SearchHits, SearchQuery};
use crate::{IndexError, ModelDetail, ModelSummary, ScanError, SearchError};

pub trait LibraryRepository: Send + Sync {
    /// # Errors
    /// Fails when the root is already registered or the index cannot be written.
    fn add(&self, name: &str, root: &str, rules: &ScanRules) -> Result<Library, IndexError>;
    /// # Errors
    /// Fails when the index cannot be read.
    fn list(&self) -> Result<Vec<Library>, IndexError>;
    /// # Errors
    /// Fails when the index cannot be read.
    fn get(&self, id: LibraryId) -> Result<Option<Library>, IndexError>;
    /// # Errors
    /// Fails when the index cannot be read or the stored rules are corrupt.
    fn scan_rules(&self, id: LibraryId) -> Result<ScanRules, IndexError>;
    /// # Errors
    /// Fails when the index cannot be written.
    fn save_settings(&self, id: LibraryId, settings: LibrarySettings) -> Result<(), IndexError>;
    /// # Errors
    /// Fails when the index cannot be written.
    fn save_scan_rules(&self, id: LibraryId, rules: &ScanRules) -> Result<(), IndexError>;
    /// When the last complete scan finished, in seconds since 1970.
    /// # Errors
    /// Fails when the index cannot be read.
    fn last_scan_at(&self, id: LibraryId) -> Result<Option<i64>, IndexError>;
}

/// A file of a library with its size in bytes and modification time (seconds since 1970) from the last scan.
pub type SizedFile = (LibraryId, RelPath, u64, i64);

/// Raw walk results. Only the scan use case writes here, from one thread.
pub trait EntryStore: Send + Sync {
    /// Starts a scan generation and returns what is known about the folders from the last scan.
    /// # Errors
    /// Fails when the index cannot be read or written.
    fn begin_scan(&self, library: LibraryId) -> Result<(Generation, KnownDirs), IndexError>;
    /// Stores a batch of walk results in one transaction.
    /// # Errors
    /// Fails when the index cannot be written.
    fn apply(&self, library: LibraryId, generation: Generation, items: &[ScanItem]) -> Result<(), IndexError>;
    /// Removes entries not seen in this generation. Returns how many were removed.
    /// # Errors
    /// Fails when the index cannot be written.
    fn finish_scan(&self, library: LibraryId, generation: Generation) -> Result<u64, IndexError>;
    /// Like [`Self::finish_scan`] after scanning only `folder` and what is below it: removes unseen entries there and
    /// leaves the rest of the library alone. `visited` is false when the scan reported nothing for `folder` itself,
    /// because it no longer exists; then it is removed with everything below it.
    /// # Errors
    /// Fails when the index cannot be written.
    fn finish_folder_scan(
        &self,
        library: LibraryId,
        generation: Generation,
        folder: &RelPath,
        visited: bool,
    ) -> Result<u64, IndexError>;
    /// All stored folder paths of a library (the root excluded).
    /// # Errors
    /// Fails when the index cannot be read.
    fn folders(&self, library: LibraryId) -> Result<Vec<RelPath>, IndexError>;
    /// Model files (meshes, slicer projects, archives) of at least `min_size` bytes that share their size with another
    /// such file in any library, grouped by size, largest first.
    /// # Errors
    /// Fails when the index cannot be read.
    fn same_size_files(&self, min_size: u64) -> Result<Vec<Vec<SizedFile>>, IndexError>;
    /// All stored file entries of a library (directories excluded).
    /// # Errors
    /// Fails when the index cannot be read.
    fn files(&self, library: LibraryId) -> Result<Vec<Entry>, IndexError>;
}

pub trait ModelRepository: Send + Sync {
    /// Replaces folders and models of a library, keeping ids of models whose path did not change. Only what differs
    /// is written; the result names the models that are new or changed, and those that are gone.
    /// # Errors
    /// Fails when the index cannot be written.
    fn replace_catalog(&self, library: LibraryId, catalog: &Catalog) -> Result<CatalogChange, IndexError>;
    /// # Errors
    /// Fails when the index cannot be read.
    fn search_documents(&self, library: LibraryId) -> Result<Vec<SearchDocument>, IndexError>;
    /// # Errors
    /// Fails when the index cannot be read.
    fn search_document(&self, id: ModelId) -> Result<Option<SearchDocument>, IndexError>;
    /// Summaries in the order of `ids`; unknown ids are skipped.
    /// # Errors
    /// Fails when the index cannot be read.
    fn summaries(&self, ids: &[ModelId]) -> Result<Vec<ModelSummary>, IndexError>;
    /// # Errors
    /// Fails when the index cannot be read.
    fn get(&self, id: ModelId) -> Result<Option<ModelDetail>, IndexError>;
    /// # Errors
    /// Fails when the index cannot be read.
    fn browse(&self, request: &BrowseRequest) -> Result<BrowsePage, IndexError>;
    /// The model a file belongs to.
    /// # Errors
    /// Fails when the index cannot be read.
    fn model_of_file(&self, library: LibraryId, file: &RelPath) -> Result<Option<ModelId>, IndexError>;
    /// Direct child folders of `folder`, sorted by display name.
    /// # Errors
    /// Fails when the index cannot be read.
    fn child_folders(&self, library: LibraryId, folder: &RelPath) -> Result<Vec<Folder>, IndexError>;
    /// Images directly in `folder` that belong to no model, sorted by path.
    /// # Errors
    /// Fails when the index cannot be read.
    fn loose_images(&self, library: LibraryId, folder: &RelPath) -> Result<Vec<RelPath>, IndexError>;
}

/// What a [`ModelRepository::replace_catalog`] changed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CatalogChange {
    /// New models and models whose files, title, cover or grouping changed.
    pub changed: Vec<ModelId>,
    pub removed: Vec<ModelId>,
}

pub trait SearchIndex: Send + Sync {
    /// Replaces all documents of a library and makes them searchable.
    /// # Errors
    /// Fails when the search index cannot be written.
    fn replace_library(&self, library: LibraryId, docs: &[SearchDocument]) -> Result<(), SearchError>;
    /// Replaces single documents (after the user edited a model).
    /// # Errors
    /// Fails when the search index cannot be written.
    fn update_documents(&self, docs: &[SearchDocument]) -> Result<(), SearchError>;
    /// Replaces `docs` and drops the `removed` models, in one commit.
    /// # Errors
    /// Fails when the search index cannot be written.
    fn apply_changes(&self, docs: &[SearchDocument], removed: &[ModelId]) -> Result<(), SearchError>;
    /// # Errors
    /// Fails when the search index cannot be read.
    fn search(&self, query: &SearchQuery) -> Result<SearchHits, SearchError>;
}

/// Walks a library folder. Implementations must only read, never write into the library.
pub trait LibraryScanner: Send + Sync {
    /// Sends one item per folder at and below `from` (the root for a whole library). Folders whose mtime matches
    /// `known` are reported as unchanged without listing them.
    /// Files and folders matching the ignore rules are neither reported nor descended into.
    /// Stops early when the receiver is gone.
    /// # Errors
    /// Returns [`ScanError::Offline`] when the root is not reachable; nothing must be deleted then.
    fn scan(
        &self,
        root: &str,
        from: &RelPath,
        known: &KnownDirs,
        rules: &CompiledRules,
        out: &SyncSender<ScanItem>,
    ) -> Result<(), ScanError>;
}

/// Tags, names, marks and notes, kept per model path so they survive regrouping.
pub trait MetaStore: Send + Sync {
    /// Default (empty) metadata when nothing was saved.
    /// # Errors
    /// Fails when the index cannot be read.
    fn meta(&self, library: LibraryId, path: &RelPath) -> Result<ModelMeta, IndexError>;
    /// Empty metadata removes the entry.
    /// # Errors
    /// Fails when the index cannot be written.
    fn save_meta(&self, library: LibraryId, path: &RelPath, meta: &ModelMeta) -> Result<(), IndexError>;
    /// When the metadata was last saved (seconds since 1970); `None` when there is none.
    /// # Errors
    /// Fails when the index cannot be read.
    fn meta_updated_at(&self, library: LibraryId, path: &RelPath) -> Result<Option<i64>, IndexError>;
}

/// A tag in use, with how it is shown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagInfo {
    pub name: TagName,
    /// Models carrying it.
    pub count: u32,
    pub group: Option<i64>,
    pub pinned: bool,
    /// When a model carrying it was last changed (seconds since 1970), for "recently used".
    pub recent: i64,
}

/// A user-made group of tags, such as Scale or Genre.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagGroup {
    pub id: i64,
    pub name: String,
    /// A CSS colour, such as `#4f8cff`.
    pub color: String,
}

/// Which models a tag count covers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TagScope {
    /// Models in the folder and below it (the root: the whole library).
    Folder(RelPath),
    Models(Vec<ModelId>),
}

/// Tag groups, pins and counts. The tags themselves live in each model's metadata.
pub trait TagStore: Send + Sync {
    /// Every tag in use, most used first.
    /// # Errors
    /// Fails when the index cannot be read.
    fn tag_infos(&self, library: LibraryId) -> Result<Vec<TagInfo>, IndexError>;
    /// The most used tags among the models in `scope`.
    /// # Errors
    /// Fails when the index cannot be read.
    fn tag_counts(&self, library: LibraryId, scope: &TagScope, limit: usize)
    -> Result<Vec<(TagName, u32)>, IndexError>;
    /// Model paths carrying any of the tags (compared without case).
    /// # Errors
    /// Fails when the index cannot be read.
    fn paths_with_tags(&self, library: LibraryId, tags: &[TagName]) -> Result<Vec<RelPath>, IndexError>;
    /// After a rename or merge: the group and pin of the first old tag move to `to`, unless it has its own.
    /// # Errors
    /// Fails when the index cannot be written.
    fn move_tag_info(&self, library: LibraryId, from: &[TagName], to: &TagName) -> Result<(), IndexError>;
    /// # Errors
    /// Fails when the index cannot be written.
    fn forget_tags(&self, library: LibraryId, tags: &[TagName]) -> Result<(), IndexError>;
    /// # Errors
    /// Fails when the index cannot be written.
    fn set_tag_group(&self, library: LibraryId, tags: &[TagName], group: Option<i64>) -> Result<(), IndexError>;
    /// # Errors
    /// Fails when the index cannot be written.
    fn set_tag_pinned(&self, library: LibraryId, tags: &[TagName], pinned: bool) -> Result<(), IndexError>;
    /// # Errors
    /// Fails when the index cannot be read.
    fn tag_groups(&self, library: LibraryId) -> Result<Vec<TagGroup>, IndexError>;
    /// Creates a group (`id` = `None`) or renames and recolours one. Returns its id.
    /// # Errors
    /// Fails when the index cannot be written.
    fn save_tag_group(&self, library: LibraryId, id: Option<i64>, name: &str, color: &str) -> Result<i64, IndexError>;
    /// Its tags become ungrouped.
    /// # Errors
    /// Fails when the index cannot be written.
    fn delete_tag_group(&self, library: LibraryId, id: i64) -> Result<(), IndexError>;
}

/// Hand-made and smart collections. Members are kept by library and path, so they survive regrouping.
pub trait CollectionStore: Send + Sync {
    /// Every collection with its member count (hand-made ones only; smart ones report `None`).
    /// # Errors
    /// Fails when the index cannot be read.
    fn list_collections(&self) -> Result<Vec<(Collection, Option<u32>)>, IndexError>;
    /// # Errors
    /// Fails when the index cannot be read.
    fn collection(&self, id: CollectionId) -> Result<Option<Collection>, IndexError>;
    /// # Errors
    /// Fails when the index cannot be written.
    fn create_collection(&self, name: &str, smart: Option<&SmartSearch>) -> Result<Collection, IndexError>;
    /// # Errors
    /// Fails when the index cannot be written.
    fn rename_collection(&self, id: CollectionId, name: &str) -> Result<(), IndexError>;
    /// # Errors
    /// Fails when the index cannot be written.
    fn delete_collection(&self, id: CollectionId) -> Result<(), IndexError>;
    /// Members in their order; members whose model no longer exists are left out.
    /// # Errors
    /// Fails when the index cannot be read.
    fn collection_members(&self, id: CollectionId) -> Result<Vec<ModelId>, IndexError>;
    /// Appends models that are not members yet, in one transaction.
    /// # Errors
    /// Fails for unknown models or when the index cannot be written.
    fn add_members(&self, id: CollectionId, models: &[ModelId]) -> Result<(), IndexError>;
    /// # Errors
    /// Fails when the index cannot be written.
    fn remove_members(&self, id: CollectionId, models: &[ModelId]) -> Result<(), IndexError>;
    /// Moves a member before the member now shown at `index` (0 = first), in one transaction. Members whose model
    /// is missing at the moment keep their place.
    /// # Errors
    /// Fails when the model is not a member or the index cannot be written.
    fn move_member(&self, id: CollectionId, model: ModelId, index: usize) -> Result<(), IndexError>;
}

/// Fingerprints of library files, for finding duplicates. Reads only.
pub trait FileHasher: Send + Sync {
    /// From the size and the first and last bytes: equal files always agree, different ones rarely do.
    /// # Errors
    /// Fails when the file cannot be read.
    fn quick(&self, root: &str, file: &RelPath) -> Result<[u8; 32], String>;
    /// Of the whole file.
    /// # Errors
    /// Fails when the file cannot be read.
    fn full(&self, root: &str, file: &RelPath) -> Result<[u8; 32], String>;
}

/// Remembered fingerprints, valid while a file keeps its size and modification time.
pub trait HashCache: Send + Sync {
    /// # Errors
    /// Fails when the index cannot be read.
    fn cached(&self, file: &SizedFile, full: bool) -> Result<Option<[u8; 32]>, IndexError>;
    /// # Errors
    /// Fails when the index cannot be written.
    fn store(&self, file: &SizedFile, full: bool, hash: [u8; 32]) -> Result<(), IndexError>;
}

/// Why a rename did not happen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileError {
    /// Something is already at the target; nothing is ever overwritten.
    AlreadyExists,
    Failed(String),
}

impl std::fmt::Display for FileError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AlreadyExists => formatter.write_str("the target already exists"),
            Self::Failed(reason) => formatter.write_str(reason),
        }
    }
}

/// The only port that changes files inside a library. Used only for libraries in Managed mode.
pub trait FileOperations: Send + Sync {
    /// Renames or moves a file or folder inside one library. Never overwrites: fails when `to` exists.
    /// # Errors
    /// Fails when `from` is missing, `to` exists, or the file system refuses.
    fn rename(&self, root: &str, from: &RelPath, to: &RelPath) -> Result<(), FileError>;
    /// # Errors
    /// Fails when the check cannot be made.
    fn exists(&self, root: &str, path: &RelPath) -> Result<bool, String>;
    /// Writes a new file; fails with [`FileError::AlreadyExists`] instead of replacing one.
    /// # Errors
    /// Fails when the file exists, the folder leads outside the library, or the file system refuses.
    fn create_file(&self, root: &str, path: &RelPath, bytes: &[u8]) -> Result<(), FileError>;
    /// Copies a file inside the library; fails with [`FileError::AlreadyExists`] instead of replacing one. Returns
    /// the size of the copy.
    /// # Errors
    /// Fails when the target exists, a path leads outside the library, or the file system refuses.
    fn copy_file(&self, root: &str, from: &RelPath, to: &RelPath) -> Result<u64, FileError>;
    /// Removes `dir` only when it is empty.
    /// # Errors
    /// Fails when it is not empty or cannot be removed.
    fn remove_empty_dir(&self, root: &str, dir: &RelPath) -> Result<(), String>;
    /// Finishes a case-only rename a crash left at its temporary name. Returns whether one was found.
    /// # Errors
    /// Fails when the file system refuses.
    fn recover(&self, root: &str, to: &RelPath) -> Result<bool, String>;
}

/// Where a journaled rename or move stands. `Pending` and `Undoing` are only seen after a crash mid-change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JournalStatus {
    Pending,
    Done,
    Failed,
    Undoing,
    Undone,
}

impl JournalStatus {
    #[must_use]
    pub fn code(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Done => "done",
            Self::Failed => "failed",
            Self::Undoing => "undoing",
            Self::Undone => "undone",
        }
    }

    #[must_use]
    pub fn from_code(code: &str) -> Option<Self> {
        [Self::Pending, Self::Done, Self::Failed, Self::Undoing, Self::Undone].into_iter().find(|s| s.code() == code)
    }
}

/// One rename or move in a library, kept so it can be undone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JournalEntry {
    pub id: i64,
    pub library: LibraryId,
    pub from: RelPath,
    pub to: RelPath,
    /// Folder or single-file model; a single file carries its `.3dhoarddeck` and cover image along.
    pub kind: ModelKind,
    /// Seconds since 1970.
    pub done_at: i64,
    pub status: JournalStatus,
}

/// The journal of renames and moves. Written before the files move, completed in one transaction with the index.
pub trait FileJournal: Send + Sync {
    /// Records a change about to happen, as [`JournalStatus::Pending`].
    /// # Errors
    /// Fails when the index cannot be written.
    fn begin(&self, library: LibraryId, from: &RelPath, to: &RelPath, kind: ModelKind) -> Result<i64, IndexError>;
    /// # Errors
    /// Fails when the index cannot be written.
    fn set_status(&self, id: i64, status: JournalStatus) -> Result<(), IndexError>;
    /// In one transaction: moves names, tags, marks, notes, picked cover, collection membership and smart-collection
    /// scopes from `from` to `to`, saves `rules` (already rewritten for the new path) and sets `status`.
    /// # Errors
    /// Fails when the index cannot be written; then nothing is changed.
    fn finish(
        &self,
        id: i64,
        library: LibraryId,
        from: &RelPath,
        to: &RelPath,
        rules: &ScanRules,
        status: JournalStatus,
    ) -> Result<(), IndexError>;
    /// # Errors
    /// Fails when the index cannot be read.
    fn entry(&self, id: i64) -> Result<Option<JournalEntry>, IndexError>;
    /// Newest first.
    /// # Errors
    /// Fails when the index cannot be read.
    fn recent(&self, limit: usize) -> Result<Vec<JournalEntry>, IndexError>;
    /// Entries left `Pending` or `Undoing` by a crash.
    /// # Errors
    /// Fails when the index cannot be read.
    fn unfinished(&self) -> Result<Vec<JournalEntry>, IndexError>;
}

/// `.3dhoarddeck` files beside the models, for libraries where the user allowed them.
pub trait SidecarFiles: Send + Sync {
    /// Writes atomically and keeps fields this version does not know.
    /// # Errors
    /// Fails when the file cannot be written.
    fn write(&self, root: &str, file: &RelPath, meta: &ModelMeta) -> Result<(), String>;
    /// The metadata and when it was last written (seconds since 1970); `None` when there is no file.
    /// # Errors
    /// Fails when the file exists but cannot be read or parsed.
    fn read(&self, root: &str, file: &RelPath) -> Result<Option<(ModelMeta, i64)>, String>;
    /// The library's folder rules (one model, collection, accepted suggestions), in the `.3dhoarddeck` at its root.
    /// # Errors
    /// Fails when the file cannot be written.
    fn write_folder_rules(&self, root: &str, rules: &[FolderRule]) -> Result<(), String>;
    /// `None` when the root has no file or the file holds no rules.
    /// # Errors
    /// Fails when the file exists but cannot be read or parsed.
    fn read_folder_rules(&self, root: &str) -> Result<Option<Vec<FolderRule>>, String>;
}

/// What `Orynt3D` stored for a model folder.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ImportedMeta {
    pub name: Option<String>,
    pub notes: String,
    pub tags: Vec<String>,
}

/// Reads `Orynt3D`'s `config.orynt3d` files. Never writes.
pub trait Orynt3dReader: Send + Sync {
    /// `None` when the folder has no `Orynt3D` file.
    /// # Errors
    /// Fails when the file exists but cannot be read or parsed.
    fn read(&self, root: &str, folder: &RelPath) -> Result<Option<ImportedMeta>, String>;
}

/// Copies the index of record to another folder (used when the user moves the index).
pub trait IndexExport: Send + Sync {
    /// When `dir` already holds an index: its last change, in seconds since the Unix epoch.
    fn existing_index(&self, dir: &Path) -> Option<i64>;
    /// Writes a consistent copy of the index into `dir`, replacing an index already there.
    /// # Errors
    /// Fails when the folder cannot be created or written.
    fn export_to(&self, dir: &Path) -> Result<(), IndexError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageKind {
    Local,
    /// SMB, NFS, AFP or similar: SQLite's locking is unreliable there.
    Network,
    /// Cloud-sync folders (Dropbox, `OneDrive`, iCloud and Google Drive): syncing half-written files corrupts SQLite.
    CloudSync,
}

/// A file inside an archive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchiveEntry {
    /// Path inside the archive, with `/`.
    pub name: String,
    /// Unpacked size in bytes.
    pub size: u64,
}

/// Reads archives in a library without unpacking them.
pub trait ArchiveReader: Send + Sync {
    /// The files in `archive`, sorted by name.
    /// # Errors
    /// Fails when the archive cannot be read or a symlink leads outside the library.
    fn list(&self, root: &str, archive: &RelPath) -> Result<Vec<ArchiveEntry>, String>;
}

/// Folder covers the user picked, kept in the index so they work in read-only libraries too.
pub trait FolderCoverStore: Send + Sync {
    /// `None` goes back to the detected cover.
    /// # Errors
    /// Fails when the index cannot be written.
    fn set_folder_cover(&self, library: LibraryId, folder: &RelPath, cover: Option<&RelPath>)
    -> Result<(), IndexError>;
}

/// Changes below one watched root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettledChange {
    pub root: String,
    /// Folders whose entries changed, relative to the root.
    pub dirs: Vec<RelPath>,
    /// The system lost events (overflow) or did not say where: only a full scan is safe.
    pub everything: bool,
}

/// Reports changes below watched library roots. Reads only.
pub trait ChangeWatcher: Send + Sync {
    /// # Errors
    /// Fails when the system cannot watch `root`; it is then rescanned on a timer instead.
    fn watch(&self, root: &str) -> Result<(), String>;
    fn unwatch(&self, root: &str);
    /// Roots that changed and then stayed quiet for `quiet`. Each change is reported once.
    fn settled(&self, quiet: Duration) -> Vec<SettledChange>;
}

pub trait StorageInspector: Send + Sync {
    fn kind(&self, dir: &Path) -> StorageKind;
}

/// Data derived from the index (the search index), kept next to it.
pub trait DerivedIndexes: Send + Sync {
    /// Discards what is derived in `index_dir`, so it is rebuilt from the index on the next start.
    /// # Errors
    /// Fails when the files cannot be removed.
    fn invalidate(&self, index_dir: &Path) -> Result<(), IndexError>;
}

/// Remembers which folder holds the index, so the next start finds it.
pub trait IndexLocationStore: Send + Sync {
    /// # Errors
    /// Fails when the bootstrap file cannot be written.
    fn save(&self, dir: &Path) -> Result<(), IndexError>;
}

/// The user's slicer list, kept with the index.
/// The tag rules, kept with the app settings (they apply to every library).
pub trait TagRuleStore: Send + Sync {
    /// # Errors
    /// Fails when the index cannot be read.
    fn load_tag_rules(&self) -> Result<Vec<crate::tagging::TagRule>, IndexError>;
    /// # Errors
    /// Fails when the index cannot be written.
    fn save_tag_rules(&self, rules: &[crate::tagging::TagRule]) -> Result<(), IndexError>;
}

/// One question to an AI: instructions, the model's name and folder, and its preview picture.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SuggestRequest {
    pub system: String,
    pub text: String,
    pub image_jpeg: Option<Vec<u8>>,
}

/// The tags as the AI wrote them, and the model that answered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiAnswer {
    pub tags: Vec<String>,
    pub model: String,
}

/// An AI server: `OpenAI`-compatible (LM Studio, Ollama, `OpenAI`, others) or Anthropic.
pub trait TagSuggester: Send + Sync {
    /// The models the server offers, for choosing one.
    /// # Errors
    /// A readable reason when the server cannot be reached or refuses.
    fn models(&self, settings: &crate::ai::AiSettings, key: Option<&str>) -> Result<Vec<String>, String>;
    /// # Errors
    /// A readable reason when the server cannot be reached, refuses, or answers with no tags.
    fn suggest(
        &self,
        settings: &crate::ai::AiSettings,
        key: Option<&str>,
        request: &SuggestRequest,
    ) -> Result<AiAnswer, String>;
}

/// Secrets in the system's keychain (macOS Keychain, Windows Credential Manager, Secret Service on Linux).
pub trait SecretStore: Send + Sync {
    /// # Errors
    /// A readable reason when the keychain cannot be used.
    fn secret(&self, name: &str) -> Result<Option<String>, String>;
    /// `None` removes it.
    /// # Errors
    /// A readable reason when the keychain cannot be used.
    fn set_secret(&self, name: &str, value: Option<&str>) -> Result<(), String>;
}

/// A small JPEG of an image file, from the thumbnail cache.
pub trait PreviewImages: Send + Sync {
    /// # Errors
    /// A readable reason when the image cannot be read.
    fn preview_jpeg(&self, file: &Path) -> Result<Vec<u8>, String>;
}

/// AI settings and the tags an AI suggested, per model path, until the user keeps or dismisses them.
pub trait AiStore: Send + Sync {
    /// # Errors
    /// Fails when the index cannot be read.
    fn load_ai_settings(&self) -> Result<crate::ai::AiSettings, IndexError>;
    /// # Errors
    /// Fails when the index cannot be written.
    fn save_ai_settings(&self, settings: &crate::ai::AiSettings) -> Result<(), IndexError>;
    /// The suggested tags and which AI suggested them (`provider:model`).
    /// # Errors
    /// Fails when the index cannot be read.
    fn suggestion(&self, library: LibraryId, path: &RelPath) -> Result<Option<(Vec<TagName>, String)>, IndexError>;
    /// Also stores an empty answer, so a run that resumes skips the model.
    /// # Errors
    /// Fails when the index cannot be written.
    fn save_suggestion(
        &self,
        library: LibraryId,
        path: &RelPath,
        tags: &[TagName],
        source: &str,
    ) -> Result<(), IndexError>;
    /// Takes `tags` out of the suggestions of `paths`.
    /// # Errors
    /// Fails when the index cannot be written.
    fn remove_suggested(&self, library: LibraryId, paths: &[RelPath], tags: &[TagName]) -> Result<(), IndexError>;
    /// Every suggested tag with the number of models it is suggested for, most first.
    /// # Errors
    /// Fails when the index cannot be read.
    fn suggestion_counts(&self, library: LibraryId) -> Result<Vec<(TagName, u32)>, IndexError>;
    /// # Errors
    /// Fails when the index cannot be read.
    fn paths_with_suggestion(&self, library: LibraryId, tag: &TagName) -> Result<Vec<RelPath>, IndexError>;
}

pub trait SlicerStore: Send + Sync {
    /// `None` until the list was saved once (first start: detect installed slicers).
    /// # Errors
    /// Fails when the index cannot be read.
    fn load_slicers(&self) -> Result<Option<SlicerList>, IndexError>;
    /// # Errors
    /// Fails when the index cannot be written.
    fn save_slicers(&self, list: &SlicerList) -> Result<(), IndexError>;
}

/// Finds slicers installed at their usual places on this computer.
pub trait SlicerDetector: Send + Sync {
    fn installed(&self) -> Vec<Slicer>;
}

/// Starts a slicer with files, without a shell, so paths with spaces or quotes are safe.
pub trait AppLauncher: Send + Sync {
    /// # Errors
    /// Fails when the program cannot be started.
    fn launch(&self, slicer: &Slicer, files: &[String]) -> Result<(), String>;
}

pub trait EventSink: Send + Sync {
    fn scan_progress(&self, progress: &ScanProgress);
    /// `done` of `total` groups of same-size files compared.
    fn duplicates_progress(&self, _done: usize, _total: usize) {}
    /// Tag suggestions: `done` of `total` models asked, `failed` of them without an answer; `running` is false at the end.
    fn suggest_progress(&self, _done: usize, _total: usize, _failed: usize, _running: bool) {}
}
