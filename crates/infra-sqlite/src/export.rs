use std::fs;
use std::path::Path;
use std::time::UNIX_EPOCH;

use hoarddeck_application::IndexError;
use hoarddeck_application::ports::IndexExport;

use crate::SqliteIndex;
use crate::database::FILE_NAME;

impl IndexExport for SqliteIndex {
    fn existing_index(&self, dir: &Path) -> Option<i64> {
        let modified = fs::metadata(dir.join(FILE_NAME)).ok()?.modified().ok()?;
        let seconds = modified.duration_since(UNIX_EPOCH).ok()?.as_secs();
        Some(i64::try_from(seconds).unwrap_or(i64::MAX))
    }

    /// `VACUUM INTO` writes a consistent, compacted copy while the app keeps running.
    fn export_to(&self, dir: &Path) -> Result<(), IndexError> {
        let io = |error: std::io::Error| IndexError(format!("{}: {error}", dir.display()));
        fs::create_dir_all(dir).map_err(io)?;
        let target = dir.join(FILE_NAME);
        // Replacing was confirmed by the user; VACUUM INTO refuses to overwrite.
        for suffix in ["", "-wal", "-shm"] {
            let file = dir.join(format!("{FILE_NAME}{suffix}"));
            if file.exists() {
                fs::remove_file(file).map_err(io)?;
            }
        }
        let target = target.to_str().ok_or_else(|| IndexError(format!("{} is not valid UTF-8", target.display())))?;
        self.write(|connection| connection.execute("VACUUM INTO ?1", [target]).map(|_| ()))
    }
}
