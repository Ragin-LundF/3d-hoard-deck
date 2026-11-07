use crate::{FileKind, RelPath};

/// One raw item found by a library walk. Grouping into models happens later, from stored entries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub path: RelPath,
    pub kind: EntryKind,
    pub size: u64,
    /// Milliseconds since the Unix epoch.
    pub mtime: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind {
    Dir,
    File(FileKind),
}
