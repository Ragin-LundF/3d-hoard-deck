use std::fs;
use std::path::Path;

use hoarddeck_application::ports::{ArchiveFormat, ArchiveInfo, ArchiveItem, ExtractError};
use hoarddeck_domain::RelPath;
use unrar::Archive;
use unrar::error::{Code, UnrarError};

use super::{Sink, planned};

/// Unix file types in the attributes of archives made on Unix.
const TYPE_MASK: u32 = 0o170_000;
const SYMLINK: u32 = 0o120_000;
/// Windows reparse points (links).
const REPARSE_POINT: u32 = 0x400;

fn archive<'a>(path: &'a Path, password: Option<&'a str>) -> Archive<'a> {
    match password {
        Some(password) => Archive::with_password(path, password),
        None => Archive::new(path),
    }
}

fn message(error: &UnrarError, archive: &RelPath) -> String {
    match error.code {
        Code::MissingPassword | Code::BadPassword => "wrong password".to_owned(),
        Code::EOpen => format!("{archive}: a part of the archive is missing or cannot be opened"),
        Code::BadData | Code::BadArchive => format!("{archive} is damaged"),
        _ => format!("{archive}: {error}"),
    }
}

pub(super) fn inspect(path: &Path, rel: &RelPath, password: Option<&str>) -> Result<ArchiveInfo, String> {
    let parts = parts(path, rel)?;
    let listing = match archive(path, password).open_for_listing() {
        Ok(listing) => listing,
        Err(error) if password.is_none() && error.code == Code::MissingPassword => {
            // The file list itself is encrypted.
            return Ok(ArchiveInfo { format: ArchiveFormat::Rar, parts, entries: Vec::new(), encrypted: true });
        }
        Err(error) => return Err(message(&error, rel)),
    };
    let mut entries = Vec::new();
    let mut encrypted = false;
    // Listing walks every volume, so a missing or damaged part shows up here, before anything is written.
    for (index, header) in listing.enumerate() {
        let header = header.map_err(|error| message(&error, rel))?;
        encrypted |= header.is_encrypted();
        let attributes = header.file_attr;
        entries.push(ArchiveItem {
            index,
            name: header.filename.to_string_lossy().into_owned(),
            size: header.unpacked_size,
            packed: 0,
            is_dir: header.is_directory(),
            is_link: attributes & TYPE_MASK == SYMLINK || attributes & REPARSE_POINT != 0,
        });
    }
    Ok(ArchiveInfo { format: ArchiveFormat::Rar, parts, entries, encrypted })
}

/// `Orcs.part1.rar` with its siblings `Orcs.part2.rar`…, or `Orcs.rar` with `Orcs.r00`, `Orcs.r01`…
pub(super) fn parts(path: &Path, rel: &RelPath) -> Result<Vec<RelPath>, String> {
    let name = rel.name();
    let lower = name.to_ascii_lowercase();
    let folder = rel.parent().unwrap_or_default();
    let dir = path.parent().ok_or_else(|| format!("{rel} has no folder"))?;
    let stem = &lower[..lower.len() - ".rar".len().min(lower.len())];
    let belongs: Box<dyn Fn(&str) -> bool> = if let Some((base, number)) = stem.rsplit_once(".part")
        && !number.is_empty()
        && number.chars().all(|c| c.is_ascii_digit())
    {
        let base = format!("{base}.part");
        Box::new(move |other: &str| {
            other
                .strip_prefix(&base)
                .and_then(|rest| rest.strip_suffix(".rar"))
                .is_some_and(|digits| !digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit()))
        })
    } else {
        let base = format!("{stem}.r");
        Box::new(move |other: &str| {
            other
                .strip_prefix(&base)
                .is_some_and(|digits| digits.len() == 2 && digits.chars().all(|c| c.is_ascii_digit()))
        })
    };
    let mut found: Vec<String> = fs::read_dir(dir)
        .map_err(|error| format!("{}: {error}", dir.display()))?
        .filter_map(Result::ok)
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|other| other != name && belongs(&other.to_ascii_lowercase()))
        .collect();
    found.sort_by_key(|other| other.to_ascii_lowercase());
    std::iter::once(name.to_owned())
        .chain(found)
        .map(|other| folder.join(&other).map_err(|error| error.to_string()))
        .collect()
}

pub(super) fn extract(
    path: &Path,
    plan: &[(usize, RelPath)],
    password: Option<&str>,
    sink: &mut Sink<'_>,
) -> Result<(), ExtractError> {
    let failed = |error: &UnrarError| match error.code {
        Code::MissingPassword | Code::BadPassword => ExtractError::WrongPassword,
        Code::BadData => ExtractError::Failed("The archive is damaged: checksum mismatch.".to_owned()),
        Code::EOpen => ExtractError::Failed("A part of the archive is missing.".to_owned()),
        _ => ExtractError::Failed(format!("The archive could not be read: {error}")),
    };
    let targets = planned(plan);
    // Headers only, for the progress total.
    for (index, header) in archive(path, password).open_for_listing().map_err(|error| failed(&error))?.enumerate() {
        if targets.contains_key(&index) {
            sink.total += header.map_err(|error| failed(&error))?.unpacked_size;
        }
    }
    let mut open = archive(path, password).open_for_processing().map_err(|error| failed(&error))?;
    let mut index = 0;
    let mut written = 0;
    while let Some(header) = open.read_header().map_err(|error| failed(&error))? {
        sink.check_cancel()?;
        open = match targets.get(&index) {
            Some(target) => {
                let declared = header.entry().unpacked_size;
                let (file, handle) = sink.target.file(target)?;
                drop(handle);
                // unrar writes the file itself and verifies the checksum; the empty file just reserved the name.
                fs::remove_file(&file).map_err(|error| ExtractError::Failed(format!("{target}: {error}")))?;
                let next = header.extract_to(&file).map_err(|error| failed(&error))?;
                // UnRAR never writes more than the size in the file header (every decoder counts it down), and the
                // free-space check covered those sizes; the result is checked against it all the same.
                let size =
                    fs::metadata(&file).map_err(|error| ExtractError::Failed(format!("{target}: {error}")))?.len();
                if size != declared {
                    return Err(ExtractError::Failed(format!("{target} does not match its declared size")));
                }
                sink.done += size;
                (sink.progress)(sink.done, sink.total, target.as_str());
                written += 1;
                next
            }
            None => header.skip().map_err(|error| failed(&error))?,
        };
        index += 1;
    }
    if written != targets.len() {
        return Err(ExtractError::Failed(format!(
            "only {written} of {} files were found in the archive",
            targets.len()
        )));
    }
    Ok(())
}
