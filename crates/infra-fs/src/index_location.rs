use std::fs;
use std::io;
use std::path::PathBuf;

use serde::Deserialize;

const APP_DIR: &str = ".3dhoarddeck";

#[derive(Deserialize)]
struct Bootstrap {
    index_dir: PathBuf,
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
