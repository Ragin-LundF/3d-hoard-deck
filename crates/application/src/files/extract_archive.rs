//! Extracting ZIP, RAR and 7z archives into a new folder next to them (Managed libraries only). Files go into a hidden
//! staging folder first and become visible with one rename once all of them are written and verified. The archive is
//! only deleted after the user looked at the result and agreed.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use hoarddeck_domain::{FileKind, ModelId, RelPath};

use super::disk::{check_name, refuse_ignored, require_managed};
use crate::ports::{
    ArchiveExtractor, ArchiveItem, Deleted, EventSink, ExtractControl, ExtractError, FileError, FileJournal,
    FileOperations, JournalEntry, JournalStatus, LibraryRepository, ModelRepository, STAGING_PREFIX, SearchIndex,
    StorageInspector, StorageKind,
};
use crate::scan::{LibraryLocks, ScanLibrary, ScanMode, model_stem};
use crate::{AppError, Refusal};

/// More entries than this, or a better compression than [`MAX_RATIO`]:1, look like a zip bomb.
const MAX_ENTRIES: usize = 100_000;
const MAX_RATIO: u64 = 1000;
/// Free space kept on top of the unpacked size: 5 % plus this.
const SPACE_MARGIN: u64 = 100 * 1024 * 1024;
/// `name (2)` … `name (999)`.
const MAX_COPIES: u32 = 999;

/// What extracting an archive would do, shown before anything is written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractPlan {
    pub id: ModelId,
    pub archive: RelPath,
    /// Every file of the archive (several for a multi-volume RAR).
    pub parts: Vec<RelPath>,
    /// Suggested name of the new folder; ` (2)` is added when the archive's own name is taken.
    pub folder: String,
    /// A folder with the archive's own name exists already.
    pub clash: bool,
    pub files: usize,
    pub bytes: u64,
    /// Content goes straight into the new folder because the archive holds only this one folder.
    pub unwrapped: Option<String>,
    /// A password is needed (the file list may only be known with it).
    pub encrypted: bool,
    /// Free bytes on the drive, when known.
    pub free: Option<u64>,
    /// On a network share: a deleted archive is gone for good.
    pub network: bool,
    /// Why it cannot be extracted (not enough space, unsafe paths, damaged…); `None` when it can.
    pub problem: Option<String>,
}

/// A finished extraction waiting for the user's decision about the archive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Extracted {
    pub journal: i64,
    pub folder: RelPath,
    pub files: usize,
    pub bytes: u64,
    /// `(name in the archive, name on disk)` of entries that had to be renamed.
    pub renamed: Vec<(String, String)>,
    /// Links in the archive, which are never extracted.
    pub skipped: Vec<String>,
    pub unwrapped: Option<String>,
    /// A few pictures among the extracted files, for a quick look.
    pub pictures: Vec<RelPath>,
    pub warning: Option<String>,
}

/// What happens with an extracted archive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArchiveChoice {
    /// Into the trash, or for good with `permanent`; the archive's names, tags and notes move to the new folder.
    Delete { permanent: bool },
    /// The new folder goes to the trash, or is removed for good with `permanent`; the archive stays.
    Discard { permanent: bool },
}

/// How a settled extraction ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finished {
    pub deleted: Deleted,
    /// Set when the files changed but the folder could not be read again.
    pub warning: Option<String>,
}

/// Where planned files go inside the new folder.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Layout {
    pub files: Vec<(usize, RelPath)>,
    pub bytes: u64,
    pub renamed: Vec<(String, String)>,
    pub skipped: Vec<String>,
    pub unwrapped: Option<String>,
}

pub struct ExtractArchives {
    pub libraries: Arc<dyn LibraryRepository>,
    pub models: Arc<dyn ModelRepository>,
    pub files: Arc<dyn FileOperations>,
    pub extractor: Arc<dyn ArchiveExtractor>,
    pub journal: Arc<dyn FileJournal>,
    pub locks: Arc<LibraryLocks>,
    pub scan: Arc<ScanLibrary>,
    pub storage: Arc<dyn StorageInspector>,
    pub search: Arc<dyn SearchIndex>,
    pub events: Arc<dyn EventSink>,
    /// Set by [`Self::cancel`]; one extraction runs at a time.
    pub cancel: AtomicBool,
}

impl ExtractArchives {
    /// What extracting `archive`, a file of model `id`, would do. Problems are reported in the plan, not as errors.
    /// # Errors
    /// Fails for an unknown model, a file that is no archive of it, or when the index cannot be read.
    pub fn plan(&self, id: ModelId, archive: &RelPath, password: Option<&str>) -> Result<ExtractPlan, AppError> {
        let (library, archive) = self.archive_of(id, archive)?;
        let parent = archive.parent().unwrap_or_default();
        let base = folder_name(archive.name());
        let taken = |name: &str| {
            parent.join(name).ok().is_none_or(|path| self.files.exists(&library.root, &path).unwrap_or(true))
        };
        let folder = free_name(&base, taken);
        let drive = absolute(&library.root, &parent);
        let mut plan = ExtractPlan {
            id,
            archive: archive.clone(),
            parts: vec![archive.clone()],
            clash: folder != base,
            folder,
            files: 0,
            bytes: 0,
            unwrapped: None,
            encrypted: false,
            free: self.storage.free_space(&drive),
            network: self.storage.kind(&drive) == StorageKind::Network,
            problem: None,
        };
        if is_later_part(archive.name()) {
            plan.problem = Some("This is a later part of a multi-volume archive; extract its first part.".to_owned());
            return Ok(plan);
        }
        let info = match self.extractor.inspect(&library.root, &archive, password) {
            Ok(info) => info,
            Err(error) => {
                plan.problem = Some(error);
                return Ok(plan);
            }
        };
        plan.parts = info.parts;
        plan.encrypted = info.encrypted;
        match layout(&info.entries) {
            Ok(found) => {
                plan.files = found.files.len();
                plan.bytes = found.bytes;
                plan.unwrapped = found.unwrapped;
                plan.problem = space_problem(found.bytes, plan.free);
            }
            Err(error) => plan.problem = Some(error.to_string()),
        }
        Ok(plan)
    }

    /// Extracts `archive`, a file of model `id`, into a new folder `name` next to it. Runs until done; [`Self::cancel`] stops
    /// it. Nothing is left in the library when it fails, and the archive is never touched.
    /// # Errors
    /// Fails unless the library is in Managed mode, for a bad or taken name, a wrong password, too little space,
    /// unsafe or damaged content, or when cancelled.
    pub fn extract(
        &self,
        id: ModelId,
        archive: &RelPath,
        name: &str,
        password: Option<&str>,
    ) -> Result<Extracted, AppError> {
        let (library, archive) = self.archive_of(id, archive)?;
        require_managed(&library)?;
        let name = name.trim();
        check_name(name)?;
        if name.starts_with(STAGING_PREFIX) {
            return Err(AppError::InvalidInput(format!("\"{name}\" is reserved for the app")));
        }
        let parent = archive.parent().unwrap_or_default();
        let folder = parent.join(name)?;
        refuse_ignored(&self.libraries.scan_rules(library.id)?, &folder)?;
        if self.files.exists(&library.root, &folder).map_err(AppError::InvalidInput)? {
            return Err(Refusal::AlreadyExists(folder.to_string()).into());
        }
        let info = self.extractor.inspect(&library.root, &archive, password).map_err(AppError::InvalidInput)?;
        if info.encrypted && info.entries.is_empty() {
            return Err(Refusal::WrongPassword.into());
        }
        let found = layout(&info.entries)?;
        if let Some(problem) = space_problem(found.bytes, self.storage.free_space(&absolute(&library.root, &parent))) {
            return Err(AppError::InvalidInput(problem));
        }
        let stamp = self.files.stamp(&library.root, &archive).map_err(AppError::InvalidInput)?;
        let journal = self.journal.begin_extract(library.id, &archive, &folder, stamp, &info.parts)?;
        let staging = parent.join(&format!("{STAGING_PREFIX}{journal}"))?;
        self.cancel.store(false, Ordering::SeqCst);
        let progress = |done: u64, total: u64, file: &str| self.events.extract_progress(&archive, done, total, file);
        let control = ExtractControl { progress: &progress, cancel: &self.cancel };
        // The long write runs without the library lock, so scans go on; the staging folder is never scanned.
        let written = self
            .extractor
            .extract(&library.root, &archive, &staging, &found.files, password, &control)
            .map_err(|error| match error {
                ExtractError::WrongPassword => Refusal::WrongPassword.into(),
                ExtractError::Cancelled => AppError::Cancelled,
                ExtractError::Failed(reason) => AppError::InvalidInput(reason),
            })
            .and_then(|()| {
                let _busy = self.locks.lock(library.id)?;
                self.files.rename(&library.root, &staging, &folder).map_err(|error| match error {
                    FileError::AlreadyExists => Refusal::AlreadyExists(folder.to_string()).into(),
                    error => AppError::InvalidInput(error.to_string()),
                })
            });
        if let Err(error) = written {
            let _ = self.files.remove_staging_dir(&library.root, &staging);
            self.journal.set_status(journal, JournalStatus::Failed)?;
            return Err(error);
        }
        self.journal.set_status(journal, JournalStatus::Done)?;
        let pictures = pictures(&folder, &found.files);
        Ok(Extracted {
            journal,
            folder,
            files: found.files.len(),
            bytes: found.bytes,
            renamed: found.renamed,
            skipped: found.skipped,
            unwrapped: found.unwrapped,
            pictures,
            warning: rescan_folder(&self.scan, library.id, &parent),
        })
    }

    /// Stops the running extraction; it then fails with [`AppError::Cancelled`] and leaves nothing behind.
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::SeqCst);
    }

    /// Deletes the archive of a finished extraction, or removes the extracted folder again.
    /// # Errors
    /// Fails for an entry that is no finished extraction, a library not in Managed mode, an archive that changed
    /// since it was extracted, [`Refusal::NoTrash`] when the trash cannot be used (nothing deleted), or when the file
    /// system refuses.
    pub fn finish(&self, journal: i64, choice: ArchiveChoice) -> Result<Finished, AppError> {
        let entry = self.extraction(journal)?;
        let library = self.libraries.get(entry.library)?.ok_or(AppError::LibraryNotFound(entry.library))?;
        require_managed(&library)?;
        // Once the archive is gone the extracted folder is all there is: it is never discarded then.
        if !self.files.exists(&library.root, &entry.from).map_err(AppError::InvalidInput)? {
            return Err(AppError::InvalidInput("this extraction is already settled".to_owned()));
        }
        let deleted = match choice {
            ArchiveChoice::Delete { permanent } => self.delete_archive(&library.root, &entry, permanent)?,
            ArchiveChoice::Discard { permanent } => self.discard(&library.root, &entry, permanent)?,
        };
        // The files are settled; a failed rescan only delays the grid.
        let warning = rescan_folder(&self.scan, library.id, &entry.from.parent().unwrap_or_default());
        if matches!(choice, ArchiveChoice::Delete { .. }) {
            // The folder's search entry was made before the archive's tags and names moved to it.
            let _sync = self.locks.search_sync();
            for (_, id, _) in self.models.models_at(library.id, std::slice::from_ref(&entry.to))? {
                if let Some(document) = self.models.search_document(id)? {
                    self.search.update_documents(&[document])?;
                }
            }
        }
        Ok(Finished { deleted, warning })
    }

    /// Finished extractions of the archives of model `id` whose archive and folder are both still there: journal
    /// entry, archive and folder, for "Extracted to … · Delete archive".
    /// # Errors
    /// Fails when the index cannot be read.
    pub fn pending(&self, id: ModelId) -> Result<Vec<(i64, RelPath, RelPath)>, AppError> {
        let Some(detail) = self.models.get(id)? else { return Ok(Vec::new()) };
        let Some(library) = self.libraries.get(detail.summary.library)? else { return Ok(Vec::new()) };
        let archives: HashSet<&RelPath> =
            detail.files.iter().filter(|file| file.kind == FileKind::Archive).map(|file| &file.path).collect();
        let mut seen = HashSet::new();
        Ok(self
            .journal
            .recent(500)?
            .into_iter()
            .filter(|entry| {
                entry.extract.is_some()
                    && entry.status == JournalStatus::Done
                    && entry.library == library.id
                    && archives.contains(&entry.from)
                    && seen.insert(entry.from.clone())
                    && self.files.exists(&library.root, &entry.from).unwrap_or(false)
                    && self.files.exists(&library.root, &entry.to).unwrap_or(false)
            })
            .map(|entry| (entry.id, entry.from, entry.to))
            .collect())
    }

    fn delete_archive(&self, root: &str, entry: &JournalEntry, permanent: bool) -> Result<Deleted, AppError> {
        let stamp = self.files.stamp(root, &entry.from).map_err(AppError::InvalidInput)?;
        if Some(stamp) != entry.extract {
            return Err(AppError::InvalidInput(
                "The archive changed since it was extracted; it was not deleted.".to_owned(),
            ));
        }
        let parent = absolute(root, &entry.from.parent().unwrap_or_default());
        if !permanent && self.storage.kind(&parent) == StorageKind::Network {
            return Err(Refusal::NoTrash(entry.from.to_string()).into());
        }
        let parts = if entry.parts.is_empty() { vec![entry.from.clone()] } else { entry.parts.clone() };
        let mut deleted = Deleted::Trashed;
        {
            let _busy = self.locks.lock(entry.library)?;
            for part in &parts {
                deleted = self.files.delete_archive(root, part, permanent).map_err(|error| match error {
                    FileError::NoTrash => AppError::from(Refusal::NoTrash(part.to_string())),
                    error => AppError::InvalidInput(error.to_string()),
                })?;
            }
            // Names, tags, notes, marks and collection membership of the archive model go to the new folder.
            self.journal.finish(entry.id, entry.library, &entry.from, &entry.to, JournalStatus::Done)?;
        }
        Ok(deleted)
    }

    /// Into the trash: files added to the folder since are not lost. For good only when the user said so.
    fn discard(&self, root: &str, entry: &JournalEntry, permanent: bool) -> Result<Deleted, AppError> {
        let no_trash = || AppError::from(Refusal::NoTrash(entry.to.to_string()));
        if !permanent {
            if self.storage.kind(&absolute(root, &entry.to)) == StorageKind::Network {
                return Err(no_trash());
            }
            let _busy = self.locks.lock(entry.library)?;
            self.files.trash_dir(root, &entry.to).map_err(|error| match error {
                FileError::NoTrash => no_trash(),
                error => AppError::InvalidInput(error.to_string()),
            })?;
            self.journal.set_status(entry.id, JournalStatus::Undone)?;
            return Ok(Deleted::Trashed);
        }
        let staging = entry.to.parent().unwrap_or_default().join(&format!("{STAGING_PREFIX}{}", entry.id))?;
        let _busy = self.locks.lock(entry.library)?;
        // Hidden first, so a crash in between leaves a folder the startup check removes, not half a model.
        self.journal.set_status(entry.id, JournalStatus::Undoing)?;
        if let Err(error) = self.files.rename(root, &entry.to, &staging) {
            self.journal.set_status(entry.id, JournalStatus::Done)?;
            return Err(AppError::InvalidInput(error.to_string()));
        }
        self.files.remove_staging_dir(root, &staging).map_err(AppError::InvalidInput)?;
        self.journal.set_status(entry.id, JournalStatus::Undone)?;
        Ok(Deleted::Removed)
    }

    fn extraction(&self, journal: i64) -> Result<JournalEntry, AppError> {
        self.journal
            .entry(journal)?
            .filter(|entry| entry.extract.is_some() && entry.status == JournalStatus::Done)
            .ok_or_else(|| AppError::InvalidInput("this extraction is already settled".to_owned()))
    }

    /// Archives are files of a model: a loose archive is its own model, archives alone in a folder belong to the
    /// folder's model.
    fn archive_of(&self, id: ModelId, archive: &RelPath) -> Result<(hoarddeck_domain::Library, RelPath), AppError> {
        let detail = self.models.get(id)?.ok_or_else(|| AppError::InvalidInput("unknown model".to_owned()))?;
        let library =
            self.libraries.get(detail.summary.library)?.ok_or(AppError::LibraryNotFound(detail.summary.library))?;
        let archive = archive.clone();
        if !detail.files.iter().any(|file| file.path == archive && file.kind == FileKind::Archive) {
            return Err(AppError::InvalidInput(format!("{archive} is not an archive")));
        }
        Ok((library, archive))
    }
}

/// Settles extractions a crash interrupted: removes their staging folders.
pub(super) fn reconcile_extraction(
    files: &dyn FileOperations,
    journal: &dyn FileJournal,
    root: &str,
    entry: &JournalEntry,
) -> Result<(), AppError> {
    let Ok(staging) = entry.to.parent().unwrap_or_default().join(&format!("{STAGING_PREFIX}{}", entry.id)) else {
        return Ok(());
    };
    if files.remove_staging_dir(root, &staging).is_err() {
        return Ok(());
    }
    let folder_there = files.exists(root, &entry.to).unwrap_or(false);
    let status = match (entry.status, folder_there) {
        // The rename happened just before the crash: the extraction is complete.
        (JournalStatus::Pending | JournalStatus::Undoing, true) => JournalStatus::Done,
        (JournalStatus::Pending, false) => JournalStatus::Failed,
        (JournalStatus::Undoing, false) => JournalStatus::Undone,
        _ => return Ok(()),
    };
    journal.set_status(entry.id, status)?;
    Ok(())
}

/// Only the archive's folder is read again: a whole-library rescan takes long on a NAS.
fn rescan_folder(scan: &ScanLibrary, library: hoarddeck_domain::LibraryId, folder: &RelPath) -> Option<String> {
    scan.scan_folder(library, folder, ScanMode::Incremental).err().map(|error| {
        format!("The files are in place, but the folder could not be rescanned ({error}). Use \"Scan this folder\".")
    })
}

/// Up to six extracted pictures, by path inside the library.
fn pictures(folder: &RelPath, files: &[(usize, RelPath)]) -> Vec<RelPath> {
    const PICTURES: [&str; 5] = [".png", ".jpg", ".jpeg", ".webp", ".gif"];
    files
        .iter()
        .filter(|(_, path)| {
            let name = path.name().to_ascii_lowercase();
            PICTURES.iter().any(|extension| name.ends_with(extension))
        })
        .filter_map(|(_, path)| RelPath::new(format!("{folder}/{path}")).ok())
        .take(6)
        .collect()
}

/// `Dragon.zip` → `Dragon`, `Orcs.part1.rar` → `Orcs`; a name Windows cannot store is made safe.
fn folder_name(archive: &str) -> String {
    let stem = model_stem(archive);
    let stem = strip_part(stem).unwrap_or(stem);
    safe_segment(stem).0
}

/// `Orcs.part1` → `Orcs`.
fn strip_part(stem: &str) -> Option<&str> {
    let (base, part) = stem.rsplit_once('.')?;
    let number = part.to_ascii_lowercase();
    let digits = number.strip_prefix("part")?;
    (!digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit())).then_some(base)
}

/// `.part2.rar` and later, or the `.r00` volumes of old RAR sets: they belong to the first part.
fn is_later_part(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    if let Some(stem) = lower.strip_suffix(".rar")
        && let Some((_, part)) = stem.rsplit_once(".part")
        && let Ok(number) = part.parse::<u32>()
    {
        return number > 1;
    }
    lower.rsplit_once('.').is_some_and(|(_, extension)| {
        extension.len() == 3 && extension.starts_with('r') && extension[1..].chars().all(|c| c.is_ascii_digit())
    })
}

/// `name`, else `name (2)`, `name (3)`…
fn free_name(name: &str, taken: impl Fn(&str) -> bool) -> String {
    if !taken(name) {
        return name.to_owned();
    }
    (2..=MAX_COPIES).map(|number| format!("{name} ({number})")).find(|candidate| !taken(candidate)).unwrap_or_default()
}

fn space_problem(bytes: u64, free: Option<u64>) -> Option<String> {
    let needed = bytes.saturating_add(bytes / 20).saturating_add(SPACE_MARGIN);
    let free = free?;
    (needed > free).then(|| format!("Not enough space: needs {}, {} free.", size_text(needed), size_text(free)))
}

#[allow(clippy::cast_precision_loss)] // shown with one decimal
fn size_text(bytes: u64) -> String {
    let gigabytes = bytes as f64 / 1_000_000_000.0;
    if gigabytes >= 1.0 { format!("{gigabytes:.1} GB") } else { format!("{:.0} MB", bytes as f64 / 1_000_000.0) }
}

fn absolute(root: &str, path: &RelPath) -> PathBuf {
    let mut absolute = Path::new(root).to_path_buf();
    absolute.extend(path.segments());
    absolute
}

/// Where each file of the archive goes. Refuses unsafe paths (absolute, drive letters, `..`), duplicate names and
/// zip bombs; skips links; renames what Windows and NAS shares cannot store and names that differ only in case.
pub(crate) fn layout(entries: &[ArchiveItem]) -> Result<Layout, AppError> {
    let unsafe_path = |name: &str| AppError::InvalidInput(format!("The archive contains an unsafe path: {name}"));
    if entries.len() > MAX_ENTRIES {
        return Err(AppError::InvalidInput(format!("The archive holds more than {MAX_ENTRIES} entries.")));
    }
    let mut out = Layout::default();
    let mut files: Vec<(usize, Vec<String>, &str)> = Vec::new();
    let mut seen = HashSet::new();
    let (mut bytes, mut packed) = (0_u64, 0_u64);
    for entry in entries {
        if entry.is_link {
            out.skipped.push(entry.name.clone());
            continue;
        }
        let name = entry.name.replace('\\', "/");
        let drive = name.len() >= 2 && name.as_bytes()[1] == b':' && name.as_bytes()[0].is_ascii_alphabetic();
        if name.starts_with('/') || drive {
            return Err(unsafe_path(&entry.name));
        }
        let mut segments = Vec::new();
        for segment in name.split('/').filter(|segment| !segment.is_empty() && *segment != ".") {
            if segment == ".." {
                return Err(unsafe_path(&entry.name));
            }
            segments.push(segment.to_owned());
        }
        if entry.is_dir || segments.is_empty() {
            continue;
        }
        if !seen.insert(segments.join("/")) {
            return Err(AppError::InvalidInput(format!("The archive holds \"{}\" twice.", entry.name)));
        }
        bytes = bytes.saturating_add(entry.size);
        packed = packed.saturating_add(entry.packed);
        files.push((entry.index, segments, entry.name.as_str()));
    }
    if packed > 0 && bytes / packed > MAX_RATIO {
        return Err(AppError::InvalidInput("The archive unpacks to far more than it holds (a zip bomb?).".to_owned()));
    }
    // One top folder and nothing beside it: its content goes straight into the new folder.
    let top = files.first().map(|(_, segments, _)| segments[0].clone());
    if let Some(top) = top
        && files.iter().all(|(_, segments, _)| segments.len() > 1 && segments[0] == top)
    {
        for (_, segments, _) in &mut files {
            segments.remove(0);
        }
        out.unwrapped = Some(top);
    }
    // Lower-cased paths already used, and the spelling chosen for each folder.
    let mut used: HashSet<String> = HashSet::new();
    let mut folders: HashMap<String, String> = HashMap::new();
    for (index, segments, original) in files {
        let mut renamed = false;
        let mut path = String::new();
        let last = segments.len() - 1;
        for (position, segment) in segments.iter().enumerate() {
            let (safe, changed) = safe_segment(segment);
            renamed |= changed;
            let joined = if path.is_empty() { safe } else { format!("{path}/{safe}") };
            if position == last {
                let free = free_name_file(&joined, |candidate| used.contains(&candidate.to_lowercase()));
                renamed |= free != joined;
                path = free;
            } else {
                let key = joined.to_lowercase();
                // Folders that differ only in case end up as one, as they would on macOS, Windows and NAS shares.
                path.clone_from(folders.entry(key.clone()).or_insert(joined));
                used.insert(key);
            }
        }
        used.insert(path.to_lowercase());
        if renamed {
            out.renamed.push((original.to_owned(), path.clone()));
        }
        out.files.push((index, RelPath::new(path)?));
    }
    out.bytes = bytes;
    Ok(out)
}

/// `a/b.stl` taken → `a/b (2).stl`.
fn free_name_file(path: &str, taken: impl Fn(&str) -> bool) -> String {
    if !taken(path) {
        return path.to_owned();
    }
    let (folder, name) = path.rsplit_once('/').map_or(("", path), |(folder, name)| (folder, name));
    let stem = model_stem(name);
    let extension = &name[stem.len()..];
    (2..=MAX_COPIES)
        .map(|number| {
            let name = format!("{stem} ({number}){extension}");
            if folder.is_empty() { name } else { format!("{folder}/{name}") }
        })
        .find(|candidate| !taken(candidate))
        .unwrap_or_default()
}

/// A name Windows and NAS shares can store: forbidden characters become `_`, a trailing dot or space gets a `_`, and
/// device names like `CON` get a leading `_`. Returns whether it changed.
fn safe_segment(name: &str) -> (String, bool) {
    if check_name(name).is_ok() {
        return (name.to_owned(), false);
    }
    let mut safe: String =
        name.chars().map(|c| if c.is_control() || "\\/:*?\"<>|".contains(c) { '_' } else { c }).collect();
    if safe.ends_with(['.', ' ']) {
        safe.push('_');
    }
    if check_name(&safe).is_err() {
        safe.insert(0, '_');
    }
    (safe, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(index: usize, name: &str, size: u64) -> ArchiveItem {
        ArchiveItem { index, name: name.to_owned(), size, packed: size, is_dir: false, is_link: false }
    }

    fn paths(found: &Layout) -> Vec<&str> {
        found.files.iter().map(|(_, path)| path.as_str()).collect()
    }

    #[test]
    fn unsafe_paths_refuse_the_whole_archive() {
        for bad in ["../x.stl", "/etc/passwd", "C:\\x.stl", "c:/x.stl", "a\\..\\..\\x.stl", "a/../../x.stl"] {
            assert!(layout(&[item(0, "ok.stl", 1), item(1, bad, 1)]).is_err(), "{bad} must be refused");
        }
        assert!(layout(&[item(0, "a/x.stl", 1), item(1, "a/./x.stl", 1)]).is_err(), "the same file twice");
    }

    #[test]
    fn links_are_skipped_and_odd_names_made_safe() {
        let mut link = item(2, "evil", 1);
        link.is_link = true;
        let found = layout(&[
            item(0, "Read me?.txt", 1),
            item(1, "Base.STL", 1),
            link,
            item(3, "base.stl", 1),
            item(4, "con.txt", 1),
        ])
        .expect("layout");
        assert_eq!(paths(&found), ["Read me_.txt", "Base.STL", "base (2).stl", "_con.txt"]);
        assert_eq!(found.skipped, ["evil"]);
        assert_eq!(found.renamed.len(), 3);
        assert!(found.renamed.contains(&("Read me?.txt".to_owned(), "Read me_.txt".to_owned())));
    }

    #[test]
    fn a_single_top_folder_is_unwrapped_and_folders_differing_in_case_merge() {
        let found =
            layout(&[item(0, "Dragon/a.stl", 3), item(1, "Dragon/Sub/b.stl", 4), item(2, "Dragon/sub/c.stl", 5)])
                .expect("layout");
        assert_eq!(found.unwrapped.as_deref(), Some("Dragon"));
        assert_eq!(paths(&found), ["a.stl", "Sub/b.stl", "Sub/c.stl"]);
        assert_eq!(found.bytes, 12);
        let mixed = layout(&[item(0, "Dragon/a.stl", 1), item(1, "readme.txt", 1)]).expect("layout");
        assert_eq!(mixed.unwrapped, None, "a loose file beside the folder keeps the folder");
    }

    #[test]
    fn zip_bombs_and_huge_archives_are_refused() {
        let mut bomb = item(0, "big.bin", 10_000_000_000);
        bomb.packed = 1000;
        assert!(layout(&[bomb]).is_err());
    }

    #[test]
    fn folder_names_and_parts() {
        assert_eq!(folder_name("Dragon Lord.zip"), "Dragon Lord");
        assert_eq!(folder_name("Orcs.part01.rar"), "Orcs");
        assert_eq!(folder_name("Odd?.7z"), "Odd_");
        assert!(is_later_part("Orcs.part2.rar") && is_later_part("Orcs.r00"));
        assert!(!is_later_part("Orcs.part1.rar") && !is_later_part("Orcs.rar") && !is_later_part("Orcs.zip"));
        assert_eq!(free_name("Dragon", |name| name == "Dragon"), "Dragon (2)");
        assert!(space_problem(1_000_000_000, Some(500_000_000)).is_some());
        assert!(space_problem(1_000_000, Some(5_000_000_000)).is_none());
        assert!(space_problem(1_000_000, None).is_none());
    }
}
