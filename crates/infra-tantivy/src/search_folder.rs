use std::fs;
use std::path::Path;

use hoarddeck_application::IndexError;
use hoarddeck_application::ports::DerivedIndexes;

/// The `search/` folder inside an index folder.
pub struct SearchFolder;

impl DerivedIndexes for SearchFolder {
    fn invalidate(&self, index_dir: &Path) -> Result<(), IndexError> {
        let search = index_dir.join("search");
        match fs::remove_dir_all(&search) {
            Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
                Err(IndexError(format!("{}: {error}", search.display())))
            }
            _ => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removes_the_search_folder_and_tolerates_a_missing_one() {
        let dir = tempfile::tempdir().expect("temp dir");
        fs::create_dir_all(dir.path().join("search")).expect("create");
        fs::write(dir.path().join("search/hoarddeck-schema"), "2").expect("write");
        SearchFolder.invalidate(dir.path()).expect("invalidate");
        assert!(!dir.path().join("search").exists());
        SearchFolder.invalidate(dir.path()).expect("nothing to remove is fine");
    }
}
