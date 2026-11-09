use std::collections::HashMap;
use std::path::Path;

use hoarddeck_application::ports::{ArchiveFormat, ArchiveInfo, ArchiveItem, ExtractError};
use hoarddeck_domain::RelPath;
use sevenz_rust2::{Archive, ArchiveReader, Error, Password};

use super::Sink;

/// Windows reparse points (links) in the entry attributes; Unix modes sit in the upper 16 bits.
const REPARSE_POINT: u32 = 0x400;
const UNIX_EXTENSION: u32 = 0x8000;

fn password_of(password: Option<&str>) -> Password {
    password.map_or_else(Password::empty, Password::from)
}

pub(super) fn inspect(path: &Path, archive: &RelPath, password: Option<&str>) -> Result<ArchiveInfo, String> {
    let opened = match password {
        Some(password) => Archive::open_with_password(path, &Password::from(password)),
        None => Archive::open(path),
    };
    let found = match opened {
        Ok(found) => found,
        // The file list itself is encrypted: nothing to show until the password is given.
        Err(Error::PasswordRequired) if password.is_none() => {
            return Ok(ArchiveInfo {
                format: ArchiveFormat::SevenZip,
                parts: vec![archive.clone()],
                entries: Vec::new(),
                encrypted: true,
            });
        }
        Err(Error::PasswordRequired | Error::MaybeBadPassword(_)) => return Err("wrong password".to_owned()),
        Err(error) => return Err(format!("{archive}: {error}")),
    };
    let entries = found
        .files
        .iter()
        .enumerate()
        .map(|(index, entry)| ArchiveItem {
            index,
            name: entry.name.clone(),
            size: entry.size,
            // Solid archives do not store sizes per file; no ratio check then.
            packed: 0,
            is_dir: entry.is_directory,
            is_link: entry.has_windows_attributes && is_link(entry.windows_attributes),
        })
        .collect();
    Ok(ArchiveInfo { format: ArchiveFormat::SevenZip, parts: vec![archive.clone()], entries, encrypted: false })
}

fn is_link(attributes: u32) -> bool {
    attributes & REPARSE_POINT != 0 || (attributes & UNIX_EXTENSION != 0 && (attributes >> 16) & 0o170_000 == 0o120_000)
}

pub(super) fn extract(
    path: &Path,
    plan: &[(usize, RelPath)],
    password: Option<&str>,
    sink: &mut Sink<'_>,
) -> Result<(), ExtractError> {
    let mut reader = ArchiveReader::open(path, password_of(password)).map_err(|error| map(&error))?;
    let files = &reader.archive().files;
    // Entries come in archive order; names are unique (checked when planning).
    let targets: HashMap<String, (&RelPath, u64)> = plan
        .iter()
        .filter_map(|(index, target)| files.get(*index).map(|entry| (entry.name.clone(), (target, entry.size))))
        .collect();
    sink.total = targets.values().map(|(_, size)| size).sum();
    let mut failure = None;
    let mut written = 0;
    let result = reader.for_each_entries(|entry, data| {
        if let Some((target, size)) = targets.get(&entry.name) {
            if let Err(error) = sink.write(target, data, *size) {
                failure = Some(error);
                return Err(Error::Other("stopped".into()));
            }
            written += 1;
        }
        Ok(true)
    });
    if let Some(error) = failure {
        return Err(error);
    }
    result.map_err(|error| map(&error))?;
    if written != targets.len() {
        return Err(ExtractError::Failed(format!(
            "only {written} of {} files were found in the archive",
            targets.len()
        )));
    }
    Ok(())
}

fn map(error: &Error) -> ExtractError {
    match error {
        Error::PasswordRequired | Error::MaybeBadPassword(_) => ExtractError::WrongPassword,
        Error::ChecksumVerificationFailed => {
            ExtractError::Failed("The archive is damaged: checksum mismatch.".to_owned())
        }
        error => ExtractError::Failed(format!("The archive could not be read: {error}")),
    }
}
