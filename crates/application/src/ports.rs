//! Ports: what the use cases need from the outside world. Adapters live in the `infra-*` crates.

use std::sync::mpsc::SyncSender;

use hoarddeck_domain::{Catalog, Entry, Folder, Library, LibraryId, ModelId, RelPath};

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

pub trait EventSink: Send + Sync {
    fn scan_progress(&self, progress: &ScanProgress);
}
