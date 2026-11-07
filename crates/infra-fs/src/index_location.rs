use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use hoarddeck_application::IndexError;
use hoarddeck_application::ports::IndexLocationStore;
use serde::{Deserialize, Serialize};

const APP_DIR: &str = ".3dhoarddeck";

#[derive(Serialize, Deserialize)]
struct Bootstrap {
    index_dir: PathBuf,
}

/// Writes `~/.3dhoarddeck/config.json` so the next start opens the index in `dir`.
pub struct BootstrapFile;

impl IndexLocationStore for BootstrapFile {
    fn save(&self, dir: &Path) -> Result<(), IndexError> {
        let to_error = |error: io::Error| IndexError(format!("index location: {error}"));
        let home = app_home().map_err(to_error)?;
        fs::create_dir_all(&home).map_err(to_error)?;
        let json = serde_json::to_vec_pretty(&Bootstrap { index_dir: dir.to_path_buf() })
            .map_err(|e| IndexError(e.to_string()))?;
        // Write next to the target and rename, so a crash never leaves a half-written file behind.
        let temporary = home.join("config.json.tmp");
        fs::write(&temporary, json).map_err(to_error)?;
        fs::rename(&temporary, home.join("config.json")).map_err(to_error)
    }
}

/// `~/.3dhoarddeck` (`%USERPROFILE%\.3dhoarddeck` on Windows). Holds `config.json` and, by default, the index.
///
/// # Errors
/// Fails when no home folder is known.
pub fn app_home() -> io::Result<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(|home| PathBuf::from(home).join(APP_DIR))
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no home folder (HOME or USERPROFILE)"))
}

/// The index folder: `HOARDDECK_INDEX_DIR` when set (tests, demos), else `index_dir` from
/// `~/.3dhoarddeck/config.json`, else `~/.3dhoarddeck`.
///
/// # Errors
/// Fails when no home folder is known or `config.json` exists but is invalid.
pub fn resolve_index_dir() -> io::Result<PathBuf> {
    if let Some(dir) = std::env::var_os("HOARDDECK_INDEX_DIR") {
        return Ok(PathBuf::from(dir));
    }
    let home = app_home()?;
    match fs::read(home.join("config.json")) {
        Ok(bytes) => {
            serde_json::from_slice::<Bootstrap>(&bytes).map(|config| config.index_dir).map_err(io::Error::other)
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(home),
        Err(error) => Err(error),
    }
}
