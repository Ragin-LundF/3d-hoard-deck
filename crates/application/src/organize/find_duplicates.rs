use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use hoarddeck_domain::{LibraryId, ModelId, RelPath};

use crate::AppError;
use crate::ports::{EntryStore, EventSink, FileHasher, HashCache, LibraryRepository, ModelRepository, SizedFile};

/// Files smaller than this are not compared; tiny files repeat by accident.
const MIN_SIZE: u64 = 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuplicateFile {
    pub library: LibraryId,
    pub path: RelPath,
    pub model: Option<ModelId>,
}

/// Files with exactly the same content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuplicateGroup {
    pub size: u64,
    pub files: Vec<DuplicateFile>,
}

/// Finds identical model files across all libraries: same size, then a quick fingerprint, then the full content.
/// Only reads files; offline libraries are skipped. Fingerprints are cached until a scan sees the file change.
pub struct FindDuplicates {
    pub libraries: Arc<dyn LibraryRepository>,
    pub entries: Arc<dyn EntryStore>,
    pub models: Arc<dyn ModelRepository>,
    pub hasher: Arc<dyn FileHasher>,
    pub cache: Arc<dyn HashCache>,
    pub events: Arc<dyn EventSink>,
    pub cancelled: Arc<AtomicBool>,
}

impl FindDuplicates {
    /// Stops a running search after the group it is comparing.
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }

    /// # Errors
    /// Fails when the index cannot be read, or with [`AppError::Cancelled`]. Unreadable files are skipped.
    pub fn execute(&self) -> Result<Vec<DuplicateGroup>, AppError> {
        self.cancelled.store(false, Ordering::Relaxed);
        let roots: HashMap<LibraryId, String> =
            self.libraries.list()?.into_iter().map(|library| (library.id, library.root)).collect();
        let candidates = self.entries.same_size_files(MIN_SIZE)?;
        let mut groups = Vec::new();
        for (done, same_size) in candidates.iter().enumerate() {
            if self.cancelled.load(Ordering::Relaxed) {
                return Err(AppError::Cancelled);
            }
            self.events.duplicates_progress(done, candidates.len());
            let size = same_size.first().map_or(0, |file| file.2);
            for same_start in split(same_size, |file| self.fingerprint(&roots, file, false)) {
                for same in split(&same_start, |file| self.fingerprint(&roots, file, true)) {
                    let files = same
                        .into_iter()
                        .map(|(library, path, _, _)| {
                            let model = self.models.model_of_file(library, &path)?;
                            Ok(DuplicateFile { library, path, model })
                        })
                        .collect::<Result<Vec<_>, AppError>>()?;
                    groups.push(DuplicateGroup { size, files });
                }
            }
        }
        self.events.duplicates_progress(candidates.len(), candidates.len());
        Ok(groups)
    }

    fn fingerprint(&self, roots: &HashMap<LibraryId, String>, file: &SizedFile, full: bool) -> Option<[u8; 32]> {
        if let Ok(Some(hash)) = self.cache.cached(file, full) {
            return Some(hash);
        }
        let root = roots.get(&file.0)?;
        let hash = if full { self.hasher.full(root, &file.1) } else { self.hasher.quick(root, &file.1) }.ok()?;
        // A cache that cannot be written only means reading the file again next time.
        let _ = self.cache.store(file, full, hash);
        Some(hash)
    }
}

/// Groups files by a fingerprint and keeps groups with more than one file.
fn split(files: &[SizedFile], fingerprint: impl Fn(&SizedFile) -> Option<[u8; 32]>) -> Vec<Vec<SizedFile>> {
    let mut by_print: HashMap<[u8; 32], Vec<SizedFile>> = HashMap::new();
    for file in files {
        if let Some(print) = fingerprint(file) {
            by_print.entry(print).or_default().push(file.clone());
        }
    }
    by_print.into_values().filter(|group| group.len() > 1).collect()
}
