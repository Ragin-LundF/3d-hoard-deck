use std::collections::HashMap;
use std::sync::Arc;

use hoarddeck_domain::{LibraryId, ModelId, RelPath};

use crate::AppError;
use crate::ports::{EntryStore, FileHasher, LibraryRepository, ModelRepository};

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
/// Only reads files; offline libraries are skipped.
pub struct FindDuplicates {
    pub libraries: Arc<dyn LibraryRepository>,
    pub entries: Arc<dyn EntryStore>,
    pub models: Arc<dyn ModelRepository>,
    pub hasher: Arc<dyn FileHasher>,
}

impl FindDuplicates {
    /// # Errors
    /// Fails when the index cannot be read. Unreadable files are skipped.
    pub fn execute(&self) -> Result<Vec<DuplicateGroup>, AppError> {
        let roots: HashMap<LibraryId, String> =
            self.libraries.list()?.into_iter().map(|library| (library.id, library.root)).collect();
        let mut groups = Vec::new();
        for candidates in self.entries.same_size_files(MIN_SIZE)? {
            let size = candidates.first().map_or(0, |(_, _, size)| *size);
            let quick = self.split(&candidates, &roots, |root, path| self.hasher.quick(root, path));
            for same_start in quick {
                for same in self.split(&same_start, &roots, |root, path| self.hasher.full(root, path)) {
                    let files = same
                        .into_iter()
                        .map(|(library, path, _)| {
                            let model = self.models.model_of_file(library, &path)?;
                            Ok(DuplicateFile { library, path, model })
                        })
                        .collect::<Result<Vec<_>, AppError>>()?;
                    groups.push(DuplicateGroup { size, files });
                }
            }
        }
        Ok(groups)
    }

    /// Groups files by a fingerprint and keeps groups with more than one file.
    fn split(
        &self,
        files: &[(LibraryId, RelPath, u64)],
        roots: &HashMap<LibraryId, String>,
        fingerprint: impl Fn(&str, &RelPath) -> Result<[u8; 32], String>,
    ) -> Vec<Vec<(LibraryId, RelPath, u64)>> {
        let mut by_print: HashMap<[u8; 32], Vec<(LibraryId, RelPath, u64)>> = HashMap::new();
        for file in files {
            let Some(root) = roots.get(&file.0) else { continue };
            if let Ok(print) = fingerprint(root, &file.1) {
                by_print.entry(print).or_default().push(file.clone());
            }
        }
        by_print.into_values().filter(|group| group.len() > 1).collect()
    }
}
