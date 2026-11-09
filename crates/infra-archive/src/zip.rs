use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use hoarddeck_application::ports::{ArchiveFormat, ArchiveInfo, ArchiveItem, ExtractError};
use hoarddeck_domain::RelPath;
use zip::ZipArchive;
use zip::result::ZipError;

use super::{Sink, planned};

fn open(path: &Path) -> Result<ZipArchive<BufReader<File>>, String> {
    let file = File::open(path).map_err(|error| format!("{}: {error}", path.display()))?;
    ZipArchive::new(BufReader::new(file)).map_err(|error| format!("{}: {error}", path.display()))
}

pub(super) fn inspect(path: &Path, archive: &RelPath) -> Result<ArchiveInfo, String> {
    // Split archives keep their data in `.z01`, `.z02`… beside the `.zip`.
    if path.with_extension("z01").exists() || path.with_extension("Z01").exists() {
        return Err("Split ZIP archives (.z01) are not supported.".to_owned());
    }
    let mut zip = open(path)?;
    let mut entries = Vec::with_capacity(zip.len());
    let mut encrypted = false;
    for index in 0..zip.len() {
        let file = zip.by_index_raw(index).map_err(|error| format!("{archive}: {error}"))?;
        encrypted |= file.encrypted();
        entries.push(ArchiveItem {
            index,
            name: file.name().map_err(|error| format!("{archive}: {error}"))?.into_owned(),
            size: file.size(),
            packed: file.compressed_size(),
            is_dir: file.is_dir(),
            is_link: file.is_symlink(),
        });
    }
    Ok(ArchiveInfo { format: ArchiveFormat::Zip, parts: vec![archive.clone()], entries, encrypted })
}

pub(super) fn extract(
    path: &Path,
    plan: &[(usize, RelPath)],
    password: Option<&str>,
    sink: &mut Sink<'_>,
) -> Result<(), ExtractError> {
    let mut zip = open(path).map_err(ExtractError::Failed)?;
    let targets = planned(plan);
    for index in 0..zip.len() {
        if targets.contains_key(&index) {
            sink.total += zip.by_index_raw(index).map_err(|error| ExtractError::Failed(error.to_string()))?.size();
        }
    }
    for (index, target) in plan {
        let file = match password {
            Some(password) => zip.by_index_decrypt(*index, password.as_bytes()),
            None => zip.by_index(*index),
        };
        let mut file = file.map_err(|error| match error {
            ZipError::InvalidPassword => ExtractError::WrongPassword,
            ZipError::UnsupportedArchive(message) if message == ZipError::PASSWORD_REQUIRED => {
                ExtractError::WrongPassword
            }
            error => ExtractError::Failed(format!("{target}: {error}")),
        })?;
        let declared = file.size();
        sink.write(target, &mut file, declared)?;
    }
    Ok(())
}
