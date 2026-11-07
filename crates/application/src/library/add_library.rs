use std::path::{Path, PathBuf};
use std::sync::Arc;

use hoarddeck_domain::Library;

use crate::AppError;
use crate::ports::LibraryRepository;
use crate::scan::ScanRules;

/// Registers a root folder, read-only and with default scan rules. Does not scan.
pub struct AddLibrary {
    pub libraries: Arc<dyn LibraryRepository>,
    /// The app's own index folder; a library must not contain it, or the app would index its own files.
    pub index_dir: PathBuf,
}

impl AddLibrary {
    /// # Errors
    /// Fails for an empty name or root, a root that contains the index folder, or a root that is already registered.
    pub fn execute(&self, name: &str, root: &str) -> Result<Library, AppError> {
        let name = name.trim();
        let root = normalize_root(root);
        if name.is_empty() || root.is_empty() {
            return Err(AppError::InvalidInput("library name and folder are required".to_owned()));
        }
        if self.index_dir.starts_with(Path::new(root)) {
            return Err(AppError::InvalidInput(format!("{root} contains the index folder; choose another folder")));
        }
        Ok(self.libraries.add(name, root, &ScanRules::default())?)
    }
}

/// `/Volumes/x/` and `/Volumes/x` are the same library; a bare `/` stays.
fn normalize_root(root: &str) -> &str {
    let root = root.trim();
    let trimmed = root.trim_end_matches(['/', '\\']);
    if trimmed.is_empty() { &root[..root.len().min(1)] } else { trimmed }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trailing_separators_do_not_create_a_second_library() {
        assert_eq!(normalize_root(" /Volumes/volume/3D Print/ "), "/Volumes/volume/3D Print");
        assert_eq!(normalize_root("C:\\3D\\"), "C:\\3D");
        assert_eq!(normalize_root("/"), "/");
    }
}
