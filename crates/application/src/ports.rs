//! Ports: what the use cases need from the outside world. Adapters live in the `infra-*` crates.

use std::path::Path;
use std::sync::mpsc::SyncSender;

use hoarddeck_domain::{
    Catalog, Collection, CollectionId, Entry, Folder, Library, LibraryId, LibrarySettings, ModelId, ModelMeta, RelPath,
    Slicer, SlicerList, TagName,
};

use crate::browse::{BrowsePage, BrowseRequest};
use crate::scan::{CompiledRules, Generation, KnownDirs, ScanItem, ScanProgress, ScanRules};
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
}

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
    /// All stored folder paths of a library (the root excluded).
    /// # Errors
    /// Fails when the index cannot be read.
    fn folders(&self, library: LibraryId) -> Result<Vec<RelPath>, IndexError>;
    /// Model files (meshes, slicer projects, archives) of at least `min_size` bytes that share their size with another
    /// such file in any library, grouped by size, largest first.
    /// # Errors
    /// Fails when the index cannot be read.
    fn same_size_files(&self, min_size: u64) -> Result<Vec<Vec<(LibraryId, RelPath, u64)>>, IndexError>;
    /// All stored file entries of a library (directories excluded).
    /// # Errors
    /// Fails when the index cannot be read.
    fn files(&self, library: LibraryId) -> Result<Vec<Entry>, IndexError>;
}

pub trait ModelRepository: Send + Sync {
    /// Replaces folders and models of a library, keeping ids of models whose path did not change.
    /// # Errors
    /// Fails when the index cannot be written.
    fn replace_catalog(&self, library: LibraryId, catalog: &Catalog) -> Result<(), IndexError>;
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
    /// # Errors
    /// Fails when the search index cannot be read.
    fn search(&self, query: &SearchQuery) -> Result<SearchHits, SearchError>;
}

/// Walks a library folder. Implementations must only read, never write into the library.
pub trait LibraryScanner: Send + Sync {
    /// Sends one item per folder. Folders whose mtime matches `known` are reported as unchanged without listing them.
    /// Files and folders matching the ignore rules are neither reported nor descended into.
    /// Stops early when the receiver is gone.
    /// # Errors
    /// Returns [`ScanError::Offline`] when the root is not reachable; nothing must be deleted then.
    fn scan(
        &self,
        root: &str,
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
    /// Every tag in use with the number of models carrying it, most used first.
    /// # Errors
    /// Fails when the index cannot be read.
    fn tags(&self, library: LibraryId) -> Result<Vec<(TagName, u32)>, IndexError>;
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
    fn create_collection(&self, name: &str, query: Option<&str>) -> Result<Collection, IndexError>;
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
    /// Replaces the ordered member list.
    /// # Errors
    /// Fails for unknown models or when the index cannot be written.
    fn set_collection_members(&self, id: CollectionId, members: &[ModelId]) -> Result<(), IndexError>;
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
}
