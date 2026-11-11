use std::collections::BTreeSet;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

use hoarddeck_domain::{Library, LibraryId, ModelKind, RelPath};

use super::format::{
    BackupFile, EntryFile, FORMAT_VERSION, FolderFile, LibraryFile, RootFile, SavedScanRules, relative_to,
};
use crate::AppError;
use crate::library::writes_sidecars;
use crate::organize::sidecar_path;
use crate::ports::{EntryStore, EventSink, LibraryRepository, SavedEntry, SidecarFiles, UserDataStore};
use crate::scan::{FolderRule, ScanRules};
use crate::tagging::TagRuleBook;

/// Metadata files written at the same time: each is its own small file in its own folder, and on a network share one
/// write after another spends most of the time waiting.
const WRITERS: usize = 4;
/// Name of the file in a folder.
const FOLDER_FILE: &str = ".3dhoarddeck";

/// What "Write metadata files" did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExportReport {
    pub written: usize,
    pub unchanged: usize,
    /// `(file, reason)` of files that could not be written; they are tried again later.
    pub failed: Vec<(String, String)>,
}

/// For the status line in Settings.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SidecarStatus {
    pub exported_at: Option<i64>,
    pub pending: usize,
}

/// Keeps a library's `.3dhoarddeck` files in step with the index. Every use case that changes user data tells it
/// which paths changed; a failed write never blocks the change in the index: the file goes into a queue and is
/// written on the next change, at start, or by "Write metadata files".
pub struct SidecarSync {
    pub libraries: Arc<dyn LibraryRepository>,
    pub store: Arc<dyn UserDataStore>,
    pub entries: Arc<dyn EntryStore>,
    pub files: Arc<dyn SidecarFiles>,
    pub tag_rules: Arc<TagRuleBook>,
    pub events: Arc<dyn EventSink>,
    /// Stops a running export.
    pub cancel: AtomicBool,
}

impl SidecarSync {
    /// The user data of these model or folder paths changed. Never fails: problems go into the queue.
    pub fn paths_changed(&self, library: LibraryId, paths: &[RelPath]) {
        if let Some(found) = self.writing(library) {
            self.write_paths(&found, paths);
            self.retry(&found);
        }
    }

    /// Library-wide data changed (scan rules, tag groups, pins, a name): the root file.
    pub fn library_changed(&self, library: LibraryId) {
        if let Some(found) = self.writing(library) {
            self.write_root(&found);
            self.retry(&found);
        }
    }

    /// Data shared by all libraries changed (collections, tag rules).
    pub fn all_libraries_changed(&self) {
        for library in self.libraries.list().unwrap_or_default() {
            self.library_changed(library.id);
        }
    }

    /// Folder rules changed: the root file, and the files of folders whose own mark was added or removed.
    pub fn rules_changed(&self, library: LibraryId, before: &ScanRules, after: &ScanRules) {
        let Some(found) = self.writing(library) else { return };
        let marked = |rules: &ScanRules| rules.folder_rules.iter().filter_map(exact_folder).collect::<BTreeSet<_>>();
        let (old, new) = (marked(before), marked(after));
        let changed: Vec<RelPath> = old.symmetric_difference(&new).cloned().collect();
        self.write_root(&found);
        self.write_paths(&found, &changed);
        self.retry(&found);
    }

    /// Files that could not be written before, tried again for every library that keeps metadata files.
    pub fn retry_all(&self) {
        for library in self.libraries.list().unwrap_or_default() {
            if writes_sidecars(library.settings) {
                self.retry(&library);
            }
        }
    }

    /// Writes the root file and a file for every model and folder with user data, plus every metadata file the
    /// scan found (so a file that still holds data cleared meanwhile is corrected). Unchanged files are not touched.
    /// # Errors
    /// Fails for an unknown library, one that does not keep metadata files, or when the index cannot be read.
    pub fn export(&self, library: LibraryId) -> Result<ExportReport, AppError> {
        let found = self.libraries.get(library)?.ok_or(AppError::LibraryNotFound(library))?;
        if !writes_sidecars(found.settings) {
            return Err(AppError::InvalidInput(
                "this library keeps no metadata files; allow them under Access & storage, or export a backup file"
                    .to_owned(),
            ));
        }
        self.cancel.store(false, Ordering::SeqCst);
        let rules = self.libraries.scan_rules(library)?;
        let pending = self.store.pending(library)?;
        let mut paths: BTreeSet<RelPath> = self.data_paths(library, &rules)?.into_iter().collect();
        paths.extend(self.entries.metadata_files(library)?.iter().filter_map(entry_path));
        paths.extend(pending.iter().filter(|path| !path.is_root()).cloned());
        let mut failed: BTreeSet<RelPath> = BTreeSet::new();
        let paths: Vec<RelPath> = paths.into_iter().collect();
        let saved = self.store.saved_entries(library, Some(&paths))?;
        let mut report = ExportReport::default();
        let total = saved.len() + 1;
        for (done, batch) in saved.chunks(WRITERS * 16).enumerate() {
            if self.cancel.load(Ordering::SeqCst) {
                return Err(AppError::Cancelled);
            }
            let jobs: Vec<(RelPath, EntryFile)> = batch.iter().filter_map(|entry| entry_file(&rules, entry)).collect();
            for (file, result) in self.write_all(&found.root, &jobs) {
                match result {
                    Ok(true) => report.written += 1,
                    Ok(false) => report.unchanged += 1,
                    Err(error) => {
                        report.failed.push((file.to_string(), error));
                        failed.extend(entry_path(&file));
                    }
                }
            }
            self.events.metadata_progress(library, ((done + 1) * WRITERS * 16).min(total), total);
        }
        let now = now();
        self.store.set_exported_at(library, now)?;
        match self.files.write_root(&found.root, &self.root_file(&found, &rules, Some(now))?) {
            Ok(true) => report.written += 1,
            Ok(false) => report.unchanged += 1,
            Err(error) => {
                report.failed.push((FOLDER_FILE.to_owned(), error));
                failed.insert(RelPath::root());
            }
        }
        let done: Vec<RelPath> = pending.into_iter().filter(|path| !failed.contains(path)).collect();
        self.store.clear_pending(library, &done)?;
        self.store.mark_pending(library, &failed.into_iter().collect::<Vec<_>>())?;
        self.events.metadata_progress(library, total, total);
        Ok(report)
    }

    /// Stops a running export.
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::SeqCst);
    }

    /// Writes one file with everything of the library to `path`, anywhere outside it. Works for read-only libraries.
    /// Returns the number of model and folder entries.
    /// # Errors
    /// Fails for an unknown library, or when the index cannot be read or the file cannot be written.
    pub fn backup(&self, library: LibraryId, path: &Path) -> Result<usize, AppError> {
        let found = self.libraries.get(library)?.ok_or(AppError::LibraryNotFound(library))?;
        // Never into a library: that would write into a library that may be read-only.
        let folder = path.parent().map(|parent| std::fs::canonicalize(parent).unwrap_or_else(|_| parent.to_path_buf()));
        if let Some(folder) = folder
            && let Some(inside) = self.libraries.list()?.into_iter().find(|other| {
                folder.starts_with(
                    std::fs::canonicalize(&other.root).unwrap_or_else(|_| Path::new(&other.root).to_path_buf()),
                )
            })
        {
            return Err(AppError::InvalidInput(format!(
                "a backup file must not be saved inside a library ({}); choose a folder outside it",
                inside.name
            )));
        }
        let rules = self.libraries.scan_rules(library)?;
        let paths = self.data_paths(library, &rules)?;
        let entries = self
            .store
            .saved_entries(library, Some(&paths))?
            .iter()
            .filter_map(|entry| entry_file(&rules, entry).map(|(file, content)| (file.to_string(), content)))
            .filter(|(_, entry)| entry.has_user_data())
            .collect::<std::collections::BTreeMap<_, _>>();
        let count = entries.len();
        let backup =
            BackupFile { version: FORMAT_VERSION, root: self.root_file(&found, &rules, Some(now()))?, entries };
        self.files.write_backup(path, &backup).map_err(AppError::InvalidInput)?;
        Ok(count)
    }

    /// # Errors
    /// Fails when the index cannot be read.
    pub fn status(&self, library: LibraryId) -> Result<SidecarStatus, AppError> {
        Ok(SidecarStatus { exported_at: self.store.exported_at(library)?, pending: self.store.pending(library)?.len() })
    }

    /// The root file for `library` as the index has it now.
    pub(crate) fn root_file(
        &self,
        library: &Library,
        rules: &ScanRules,
        exported: Option<i64>,
    ) -> Result<RootFile, AppError> {
        let data = self.store.saved_library(library.id)?;
        let entries = self.data_paths(library.id, rules)?.len();
        Ok(RootFile {
            folder_rules: rules.folder_rules.clone(),
            library: Some(LibraryFile {
                uid: self.store.library_uid(library.id, None)?,
                name: library.name.clone(),
                metadata_storage: Some(library.settings.metadata().code().to_owned()),
                scan_rules: SavedScanRules {
                    ignore: rules.ignore.clone(),
                    variant_folders: rules.variant_folders.clone(),
                    parent_covers: Some(rules.parent_covers),
                },
                tag_groups: data.tag_groups,
                pinned_tags: data.pinned_tags,
                collections: data.collections,
                tag_rules: self.tag_rules.rules()?,
                entries,
                exported_at: exported.or(self.store.exported_at(library.id)?).unwrap_or(0),
                app_version: env!("CARGO_PKG_VERSION").to_owned(),
            }),
        })
    }

    /// Every model or folder path with user data: what the index holds, plus folders the user marked.
    fn data_paths(&self, library: LibraryId, rules: &ScanRules) -> Result<Vec<RelPath>, AppError> {
        let mut paths: BTreeSet<RelPath> = self
            .store
            .saved_entries(library, None)?
            .into_iter()
            .filter(|entry| entry_file(rules, entry).is_some_and(|(_, file)| file.has_user_data()))
            .map(|entry| entry.path)
            .collect();
        paths.extend(rules.folder_rules.iter().filter_map(exact_folder));
        Ok(paths.into_iter().collect())
    }

    fn writing(&self, library: LibraryId) -> Option<Library> {
        self.libraries.get(library).ok().flatten().filter(|found| writes_sidecars(found.settings))
    }

    /// Writes the files of `paths`; those that fail go into the queue and are returned.
    fn write_paths(&self, library: &Library, paths: &[RelPath]) -> Vec<RelPath> {
        if paths.is_empty() {
            return Vec::new();
        }
        let saved = self
            .libraries
            .scan_rules(library.id)
            .map_err(AppError::from)
            .and_then(|rules| Ok((self.store.saved_entries(library.id, Some(paths))?, rules)));
        let Ok((saved, rules)) = saved else {
            let _ = self.store.mark_pending(library.id, paths);
            return paths.to_vec();
        };
        let jobs: Vec<(RelPath, EntryFile)> = saved.iter().filter_map(|entry| entry_file(&rules, entry)).collect();
        let failed: Vec<RelPath> = self
            .write_all(&library.root, &jobs)
            .into_iter()
            .filter(|(_, result)| result.is_err())
            .filter_map(|(file, _)| entry_path(&file))
            .collect();
        let _ = self.store.mark_pending(library.id, &failed);
        failed
    }

    /// Writes the root file; a failure goes into the queue. Returns whether it worked.
    fn write_root(&self, library: &Library) -> bool {
        let written = self
            .libraries
            .scan_rules(library.id)
            .map_err(AppError::from)
            .and_then(|rules| self.root_file(library, &rules, None))
            .is_ok_and(|file| self.files.write_root(&library.root, &file).is_ok());
        if !written {
            let _ = self.store.mark_pending(library.id, &[RelPath::root()]);
        }
        written
    }

    /// Tries the queued files again; those that work leave the queue.
    fn retry(&self, library: &Library) {
        let Ok(pending) = self.store.pending(library.id) else { return };
        if pending.is_empty() {
            return;
        }
        // The queue is only emptied for files that were written, so a crash in between loses nothing.
        let (root, paths): (Vec<RelPath>, Vec<RelPath>) = pending.iter().cloned().partition(RelPath::is_root);
        let mut failed = self.write_paths(library, &paths);
        if !root.is_empty() && !self.write_root(library) {
            failed.push(RelPath::root());
        }
        let written: Vec<RelPath> = pending.into_iter().filter(|path| !failed.contains(path)).collect();
        let _ = self.store.clear_pending(library.id, &written);
    }

    /// Writes the files a few at a time; returns each file with its outcome.
    fn write_all(&self, root: &str, jobs: &[(RelPath, EntryFile)]) -> Vec<(RelPath, Result<bool, String>)> {
        if jobs.is_empty() {
            return Vec::new();
        }
        let chunk = jobs.len().div_ceil(WRITERS);
        thread::scope(|scope| {
            let workers: Vec<_> = jobs
                .chunks(chunk)
                .map(|part| {
                    scope.spawn(move || {
                        part.iter()
                            .map(|(file, entry)| (file.clone(), self.files.write_entry(root, file, entry)))
                            .collect::<Vec<_>>()
                    })
                })
                .collect();
            workers.into_iter().flat_map(|worker| worker.join().unwrap_or_default()).collect()
        })
    }
}

/// The file and its content for one saved entry; `None` for the library root (that is the root file).
pub(crate) fn entry_file(rules: &ScanRules, saved: &SavedEntry) -> Option<(RelPath, EntryFile)> {
    // The root is the library file; a path that no longer exists gets no file (its folder may be gone too).
    let kind = saved.kind.filter(|_| !saved.path.is_root())?;
    let single_file = kind == ModelKind::File;
    let file =
        if single_file { sidecar_path(&saved.path, ModelKind::File)? } else { saved.path.join(FOLDER_FILE).ok()? };
    let folder = if single_file { saved.path.parent().unwrap_or_default() } else { saved.path.clone() };
    let role =
        rules.folder_rules.iter().find(|rule| rule.pattern == FolderRule::for_folder(&saved.path, rule.role).pattern);
    let folder_file = (!single_file).then(|| FolderFile {
        role: role.map(|rule| rule.role),
        cover: saved.folder_cover.as_ref().and_then(|cover| relative_to(cover, &saved.path)),
        cover_from_library: saved
            .folder_cover
            .as_ref()
            .filter(|cover| relative_to(cover, &saved.path).is_none())
            .map(ToString::to_string),
    });
    let entry = EntryFile {
        cover_in_folder: saved.meta.cover.as_ref().and_then(|cover| relative_to(cover, &folder)),
        meta: saved.meta.clone(),
        added_at: saved.added_at,
        rendered_preview: saved.rendered.clone().filter(|state| state == "skipped"),
        folder: folder_file
            .filter(|folder| folder.role.is_some() || folder.cover.is_some() || folder.cover_from_library.is_some()),
    };
    Some((file, entry))
}

/// The model or folder a metadata file belongs to: `Minis/Dragon/.3dhoarddeck` → `Minis/Dragon`,
/// `Makerworld/13661.3mf.3dhoarddeck` → `Makerworld/13661.3mf`; `None` for the root file.
#[must_use]
pub fn entry_path(file: &RelPath) -> Option<RelPath> {
    if file.name() == FOLDER_FILE {
        return file.parent().filter(|folder| !folder.is_root());
    }
    let model = file.name().strip_suffix(FOLDER_FILE)?;
    file.parent().unwrap_or_default().join(model).ok()
}

/// The folder of a rule for exactly one folder (an escaped path), `None` for a pattern.
pub(crate) fn exact_folder(rule: &FolderRule) -> Option<RelPath> {
    let mut path = String::new();
    let mut chars = rule.pattern.chars();
    while let Some(c) = chars.next() {
        match c {
            '[' => {
                path.push(chars.next()?);
                if chars.next()? != ']' {
                    return None;
                }
            }
            '*' | '?' | '{' | '}' | ']' => return None,
            c => path.push(c),
        }
    }
    let folder = RelPath::new(path).ok()?;
    (FolderRule::for_folder(&folder, rule.role).pattern == rule.pattern).then_some(folder)
}

fn now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scan::FolderRole;

    fn path(value: &str) -> RelPath {
        RelPath::new(value).expect("path")
    }

    #[test]
    fn files_map_to_their_model_or_folder() {
        assert_eq!(entry_path(&path("Minis/Dragon/.3dhoarddeck")), Some(path("Minis/Dragon")));
        assert_eq!(entry_path(&path("Makerworld/13661.3mf.3dhoarddeck")), Some(path("Makerworld/13661.3mf")));
        assert_eq!(entry_path(&path(".3dhoarddeck")), None, "the root file is the library's");
    }

    #[test]
    fn exact_folder_rules_are_told_apart_from_patterns() {
        let exact = FolderRule::for_folder(&path("Minis/Loot [2024]/Dragon*"), FolderRole::Model);
        assert_eq!(exact_folder(&exact), Some(path("Minis/Loot [2024]/Dragon*")));
        let pattern = FolderRule { pattern: "**/Loot Studios/*/*".to_owned(), role: FolderRole::Collection };
        assert_eq!(exact_folder(&pattern), None);
    }
}
