use std::collections::HashMap;

use hoarddeck_domain::{Entry, LibraryId, RelPath};

/// Counter that marks entries seen by one scan, so entries not seen can be removed afterwards.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Generation(pub i64);

/// What the last scan stored about one folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KnownDir {
    pub mtime: i64,
    /// Names of the direct subfolders.
    pub child_dirs: Vec<String>,
}

/// Snapshot of all folders listed by the last scan, keyed by path. Lets a rescan skip unchanged folders.
pub type KnownDirs = HashMap<RelPath, KnownDir>;

/// One result of walking a folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScanItem {
    /// The folder was read; `children` are all its direct files and subfolders.
    Listed { dir: RelPath, mtime: i64, children: Vec<Entry> },
    /// The folder mtime matched the last scan; its stored files are still valid.
    Unchanged { dir: RelPath },
    /// The folder could not be read; keep whatever is stored below it.
    Failed { dir: RelPath, message: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScanPhase {
    Walking,
    Grouping,
    Done,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanProgress {
    pub library: LibraryId,
    pub phase: ScanPhase,
    pub dirs: u64,
    pub files: u64,
}
