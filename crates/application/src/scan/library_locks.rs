use std::collections::HashSet;
use std::sync::{Mutex, PoisonError};

use hoarddeck_domain::LibraryId;

use crate::AppError;

/// One writer per library at a time: a scan and a regroup must not interleave their catalog writes.
#[derive(Debug, Default)]
pub struct LibraryLocks(Mutex<HashSet<LibraryId>>);

impl LibraryLocks {
    /// # Errors
    /// Returns [`AppError::LibraryBusy`] when the library is already locked.
    pub fn lock(&self, library: LibraryId) -> Result<LibraryLock<'_>, AppError> {
        if !self.0.lock().unwrap_or_else(PoisonError::into_inner).insert(library) {
            return Err(AppError::LibraryBusy(library));
        }
        Ok(LibraryLock { locks: self, library })
    }
}

/// Releases the library when dropped.
pub struct LibraryLock<'a> {
    locks: &'a LibraryLocks,
    library: LibraryId,
}

impl Drop for LibraryLock<'_> {
    fn drop(&mut self) {
        self.locks.0.lock().unwrap_or_else(PoisonError::into_inner).remove(&self.library);
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
