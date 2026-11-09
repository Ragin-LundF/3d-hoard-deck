use std::collections::HashSet;
use std::sync::{Mutex, MutexGuard, PoisonError};

use hoarddeck_domain::LibraryId;

use crate::AppError;

/// One writer per library at a time: a scan and a regroup must not interleave their catalog writes.
#[derive(Debug, Default)]
pub struct LibraryLocks {
    libraries: Mutex<HashSet<LibraryId>>,
    search: Mutex<()>,
    regroup: Mutex<()>,
}

impl LibraryLocks {
    /// # Errors
    /// Returns [`AppError::LibraryBusy`] when the library is already locked.
    pub fn lock(&self, library: LibraryId) -> Result<LibraryLock<'_>, AppError> {
        if !self.libraries.lock().unwrap_or_else(PoisonError::into_inner).insert(library) {
            return Err(AppError::LibraryBusy(library));
        }
        Ok(LibraryLock { locks: self, library })
    }
}

impl LibraryLocks {
    /// Held while documents are read from the index and written to the search index, so an edit made during a
    /// regroup is never overwritten by the regroup's older copy.
    pub fn search_sync(&self) -> MutexGuard<'_, ()> {
        self.search.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Held for a whole regroup (read entries, write models, update search), so a regroup after a rule edit can run
    /// while a scan walks, and still never interleaves with the scan's own regroup. Taken before [`Self::search_sync`].
    pub fn regroup_sync(&self) -> MutexGuard<'_, ()> {
        self.regroup.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Releases the library when dropped.
pub struct LibraryLock<'a> {
    locks: &'a LibraryLocks,
    library: LibraryId,
}

impl Drop for LibraryLock<'_> {
    fn drop(&mut self) {
        self.locks.libraries.lock().unwrap_or_else(PoisonError::into_inner).remove(&self.library);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_library_has_one_writer_at_a_time() {
        let locks = LibraryLocks::default();
        let first = locks.lock(LibraryId(1)).expect("first lock");
        assert!(matches!(locks.lock(LibraryId(1)), Err(AppError::LibraryBusy(_))));
        assert!(locks.lock(LibraryId(2)).is_ok(), "other libraries are independent");
        drop(first);
        assert!(locks.lock(LibraryId(1)).is_ok(), "released when dropped");
    }
}
