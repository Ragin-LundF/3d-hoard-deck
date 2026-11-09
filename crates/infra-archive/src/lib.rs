//! Unpacks ZIP, RAR (feature `rar`) and 7z (feature `sevenz`) archives into a staging folder inside a library.
//! Every file is written as a new file (never over an existing one), only below the staging folder, never larger than
//! its declared size, and read to its end so the format's checksum is verified.

use std::fs::{self, File};
use std::io::{BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use hoarddeck_application::ports::{
    ArchiveExtractor, ArchiveInfo, ExtractControl, ExtractError, ExtractProgress, STAGING_PREFIX,
};
use hoarddeck_domain::RelPath;

#[cfg(feature = "rar")]
mod rar;
#[cfg(feature = "sevenz")]
mod sevenz;
mod zip;

/// Written in chunks of this size: few round trips to a NAS, little memory.
const CHUNK: usize = 1024 * 1024;

pub struct Archives;

impl ArchiveExtractor for Archives {
    fn inspect(&self, root: &str, archive: &RelPath, password: Option<&str>) -> Result<ArchiveInfo, String> {
        let path = inside(root, archive)?;
        match extension(archive).as_str() {
            "zip" => zip::inspect(&path, archive),
            #[cfg(feature = "rar")]
            "rar" => rar::inspect(&path, archive, password),
            #[cfg(feature = "sevenz")]
            "7z" => sevenz::inspect(&path, archive, password),
            _ => {
                let _ = password;
                Err(format!("{} archives cannot be extracted", extension(archive).to_uppercase()))
            }
        }
    }

    fn parts(&self, root: &str, archive: &RelPath) -> Result<Vec<RelPath>, String> {
        #[cfg(feature = "rar")]
        if extension(archive) == "rar" {
            return rar::parts(&inside(root, archive)?, archive);
        }
        let _ = root;
        Ok(vec![archive.clone()])
    }

    fn extract(
        &self,
        root: &str,
        archive: &RelPath,
        staging: &RelPath,
        plan: &[(usize, RelPath)],
        password: Option<&str>,
        control: &ExtractControl<'_>,
    ) -> Result<(), ExtractError> {
        let source = inside(root, archive).map_err(ExtractError::Failed)?;
        let target = Target::create(root, staging)?;
        let mut sink = Sink { target, done: 0, total: 0, progress: control.progress, cancel: control.cancel };
        match extension(archive).as_str() {
            "zip" => zip::extract(&source, plan, password, &mut sink),
            #[cfg(feature = "rar")]
            "rar" => rar::extract(&source, plan, password, &mut sink),
            #[cfg(feature = "sevenz")]
            "7z" => sevenz::extract(&source, plan, password, &mut sink),
            _ => Err(ExtractError::Failed(format!("{archive} cannot be extracted"))),
        }
    }
}

/// The new staging folder, canonical, so every written path can be checked against it.
struct Target {
    dir: PathBuf,
}

impl Target {
    fn create(root: &str, staging: &RelPath) -> Result<Self, ExtractError> {
        let failed = |message: String| ExtractError::Failed(message);
        if !staging.name().starts_with(STAGING_PREFIX) {
            return Err(failed(format!("{staging} is not a staging folder")));
        }
        let root = fs::canonicalize(root).map_err(|error| failed(format!("{root}: {error}")))?;
        let dir = absolute(&root, staging);
        let parent = dir.parent().ok_or_else(|| failed(format!("{staging} has no parent")))?;
        let real_parent = fs::canonicalize(parent).map_err(|error| failed(format!("{}: {error}", parent.display())))?;
        if !real_parent.starts_with(&root) {
            return Err(failed(format!("{staging} leads outside the library")));
        }
        // `create_dir` fails when it exists: a staging folder is always new.
        fs::create_dir(&dir).map_err(|error| failed(format!("{staging}: {error}")))?;
        Ok(Self { dir: real_parent.join(dir.file_name().unwrap_or_default()) })
    }

    /// A new file at `path` below the staging folder; its folder is checked to stay inside after creating it.
    fn file(&self, path: &RelPath) -> Result<(PathBuf, File), ExtractError> {
        let failed = |message: String| ExtractError::Failed(message);
        let target = absolute(&self.dir, path);
        let parent = target.parent().ok_or_else(|| failed(format!("{path} has no folder")))?;
        fs::create_dir_all(parent).map_err(|error| failed(format!("{path}: {error}")))?;
        let real = fs::canonicalize(parent).map_err(|error| failed(format!("{path}: {error}")))?;
        if !real.starts_with(&self.dir) {
            return Err(failed(format!("{path} leads outside the new folder")));
        }
        let file = File::create_new(&target).map_err(|error| failed(format!("{path}: {error}")))?;
        Ok((target, file))
    }
}

/// Writes entries, counts bytes and reports progress.
struct Sink<'a> {
    target: Target,
    done: u64,
    total: u64,
    progress: ExtractProgress<'a>,
    cancel: &'a AtomicBool,
}

impl Sink<'_> {
    fn check_cancel(&self) -> Result<(), ExtractError> {
        if self.cancel.load(Ordering::SeqCst) { Err(ExtractError::Cancelled) } else { Ok(()) }
    }

    /// Copies `reader` to a new file. Reads to the end (which makes the decoder verify the checksum) and refuses an
    /// entry that turns out larger or smaller than `declared`.
    fn write(&mut self, path: &RelPath, reader: &mut dyn Read, declared: u64) -> Result<(), ExtractError> {
        self.check_cancel()?;
        let (_, file) = self.target.file(path)?;
        let mut out = BufWriter::with_capacity(CHUNK, file);
        let mut buffer = vec![0_u8; CHUNK.min(usize::try_from(declared).unwrap_or(CHUNK)).max(64 * 1024)];
        let mut written = 0_u64;
        loop {
            let read = reader.read(&mut buffer).map_err(|error| read_error(path, &error))?;
            if read == 0 {
                break;
            }
            written += read as u64;
            if written > declared {
                return Err(ExtractError::Failed(format!("{path} unpacks to more than the archive declares")));
            }
            out.write_all(&buffer[..read]).map_err(|error| ExtractError::Failed(format!("{path}: {error}")))?;
            self.done += read as u64;
            (self.progress)(self.done, self.total, path.as_str());
            self.check_cancel()?;
        }
        out.flush().map_err(|error| ExtractError::Failed(format!("{path}: {error}")))?;
        if written != declared {
            return Err(ExtractError::Failed(format!("{path} is shorter than the archive declares (damaged?)")));
        }
        Ok(())
    }
}

fn read_error(path: &RelPath, error: &std::io::Error) -> ExtractError {
    let text = error.to_string();
    if text.to_lowercase().contains("password") {
        ExtractError::WrongPassword
    } else {
        ExtractError::Failed(format!("The archive is damaged at {path}: {text}"))
    }
}

fn extension(archive: &RelPath) -> String {
    archive.name().rsplit_once('.').map(|(_, extension)| extension.to_ascii_lowercase()).unwrap_or_default()
}

fn absolute(root: &Path, path: &RelPath) -> PathBuf {
    let mut absolute = root.to_path_buf();
    absolute.extend(path.segments());
    absolute
}

/// The archive's absolute path; a symlink must not lead out of the library.
fn inside(root: &str, archive: &RelPath) -> Result<PathBuf, String> {
    let root = fs::canonicalize(root).map_err(|error| format!("{root}: {error}"))?;
    let path = fs::canonicalize(absolute(&root, archive)).map_err(|error| format!("{archive}: {error}"))?;
    if path.starts_with(&root) { Ok(path) } else { Err(format!("{archive} leads outside the library")) }
}

/// The planned path of each archive entry, by index.
fn planned(plan: &[(usize, RelPath)]) -> std::collections::HashMap<usize, &RelPath> {
    plan.iter().map(|(index, path)| (*index, path)).collect()
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn a_link_inside_the_staging_folder_cannot_lead_writes_outside() {
        let dir = tempfile::tempdir().expect("dir");
        let outside = tempfile::tempdir().expect("outside");
        let root = dir.path().to_str().expect("utf-8");
        let target = Target::create(root, &RelPath::new(".hoarddeck-extract-9").expect("path")).expect("staging");
        std::os::unix::fs::symlink(outside.path(), target.dir.join("evil")).expect("link");
        assert!(target.file(&RelPath::new("evil/x.stl").expect("path")).is_err());
        assert!(fs::read_dir(outside.path()).expect("list").next().is_none(), "nothing written outside");
        assert!(target.file(&RelPath::new("ok/x.stl").expect("path")).is_ok());
    }
}
