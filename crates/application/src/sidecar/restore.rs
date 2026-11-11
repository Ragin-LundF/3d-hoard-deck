use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use hoarddeck_domain::{LibraryId, RelPath};

use super::format::{EntryFile, RootFile, resolve};
use super::sync::{entry_path, exact_folder};
use crate::AppError;
use crate::ports::{
    EntryStore, LibraryRepository, MetaStore, ModelRepository, RestoredEntry, SavedLibraryData, SearchIndex,
    SidecarFiles, UserDataStore,
};
use crate::scan::{FolderRule, LibraryLocks, RuleEditor};
use crate::tagging::TagRuleBook;

/// Where a restore reads from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RestoreSource {
    /// The `.3dhoarddeck` files in the library (those the last scan found).
    Files,
    /// One backup file written by "Export backup file".
    Backup(PathBuf),
}

/// Which side wins when the index and a file disagree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestoreMode {
    /// A file wins only when it was written after the index value changed (an empty index takes every file).
    NewerWins,
    /// The files win: for "my index is wrong, trust the drive".
    FilesWin,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RestoreReport {
    /// Models and folders whose data came back from a file.
    pub restored: usize,
    pub unchanged: usize,
    /// Files that could not be read, with the reason.
    pub unreadable: Vec<String>,
    /// Folder rules added or changed.
    pub rules_restored: usize,
    /// Members of saved collections whose model does not exist (anymore).
    pub dropped_members: usize,
    /// Saved tag rules applied because the app had none yet.
    pub tag_rules_restored: usize,
    /// Saved tag rules not applied because the app has its own; the user may take them over.
    pub tag_rules_offered: usize,
}

/// What a library folder's root file says it holds, shown when the folder is added as a library.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SavedSummary {
    pub name: String,
    pub exported_at: i64,
    pub folder_rules: usize,
    pub collections: usize,
    pub entries: usize,
}

/// Fills the index from the `.3dhoarddeck` files of a library or from a backup file: after a lost index, or on another
/// computer with the same drive. Reads the library only.
pub struct RestoreFromSidecars {
    pub libraries: Arc<dyn LibraryRepository>,
    pub models: Arc<dyn ModelRepository>,
    pub meta: Arc<dyn MetaStore>,
    pub store: Arc<dyn UserDataStore>,
    pub entries: Arc<dyn EntryStore>,
    pub files: Arc<dyn SidecarFiles>,
    pub search: Arc<dyn SearchIndex>,
    pub locks: Arc<LibraryLocks>,
    pub editor: Arc<RuleEditor>,
    pub tag_rules: Arc<TagRuleBook>,
}

impl RestoreFromSidecars {
    /// Restores folder rules and scan rules (one regroup), then names, tags, marks, notes, covers, dates added and
    /// skipped previews, then tag groups, pins and collections, and rebuilds the search entries of the library.
    /// # Errors
    /// Fails for an unknown library, an unreadable backup file, or when the index cannot be written. Broken metadata
    /// files are reported, not fatal.
    pub fn execute(
        &self,
        library: LibraryId,
        source: &RestoreSource,
        mode: RestoreMode,
    ) -> Result<RestoreReport, AppError> {
        let found = self.libraries.get(library)?.ok_or(AppError::LibraryNotFound(library))?;
        let mut report = RestoreReport::default();
        let (root, saved) = self.load(&found.root, library, source, &mut report)?;
        // Read before the rules change: a library without folder rules is a fresh index.
        let fresh = self.editor.rules(library)?.folder_rules.is_empty();
        self.restore_metadata_storage(&found, root.as_ref(), mode, fresh)?;
        report.rules_restored = self.restore_rules(library, root.as_ref(), &saved, mode)?;
        self.restore_entries(library, &saved, mode, &mut report)?;
        if let Some(section) = root.as_ref().and_then(|file| file.library.as_ref()) {
            let data = SavedLibraryData {
                tag_groups: section.tag_groups.clone(),
                pinned_tags: section.pinned_tags.clone(),
                collections: section.collections.clone(),
            };
            report.dropped_members = self.store.restore_library(library, &data, mode == RestoreMode::FilesWin)?;
            if !section.uid.is_empty() {
                self.store.library_uid(library, Some(&section.uid))?;
            }
            if !section.tag_rules.is_empty() {
                if self.tag_rules.rules()?.is_empty() {
                    self.apply_tag_rules(&section.tag_rules)?;
                    report.tag_rules_restored = section.tag_rules.len();
                } else if self.tag_rules.rules()? != section.tag_rules {
                    report.tag_rules_offered = section.tag_rules.len();
                }
            }
        }
        let _sync = self.locks.search_sync();
        self.search.replace_library(library, &self.models.search_documents(library)?)?;
        Ok(report)
    }

    /// Replaces the app's tag rules with those saved for `library` (after the user agreed). Returns how many.
    /// # Errors
    /// Fails when nothing is saved, the backup cannot be read, or the rules cannot be saved.
    pub fn use_saved_tag_rules(&self, library: LibraryId, source: &RestoreSource) -> Result<usize, AppError> {
        let found = self.libraries.get(library)?.ok_or(AppError::LibraryNotFound(library))?;
        let root = match source {
            RestoreSource::Files => {
                self.files.read_root(&found.root).map_err(AppError::InvalidInput)?.map(|(file, _)| file)
            }
            RestoreSource::Backup(path) => Some(self.files.read_backup(path).map_err(AppError::InvalidInput)?.root),
        };
        let rules = root.and_then(|file| file.library).map(|section| section.tag_rules).unwrap_or_default();
        if rules.is_empty() {
            return Err(AppError::InvalidInput("no tag rules are saved for this library".to_owned()));
        }
        self.apply_tag_rules(&rules)?;
        Ok(rules.len())
    }

    /// What the root file in `folder` holds; `None` when there is none or it has no library section.
    #[must_use]
    pub fn saved_summary(&self, folder: &str) -> Option<SavedSummary> {
        let (root, _) = self.files.read_root(folder).ok().flatten()?;
        let section = root.library?;
        Some(SavedSummary {
            name: section.name,
            exported_at: section.exported_at,
            folder_rules: root.folder_rules.len(),
            collections: section.collections.len(),
            entries: section.entries,
        })
    }

    /// "Where tags and notes are saved", when the library's access allows it (a file never grants write access).
    fn restore_metadata_storage(
        &self,
        library: &hoarddeck_domain::Library,
        root: Option<&RootFile>,
        mode: RestoreMode,
        fresh: bool,
    ) -> Result<(), AppError> {
        let saved = root
            .and_then(|file| file.library.as_ref())
            .and_then(|section| section.metadata_storage.as_deref())
            .and_then(|code| hoarddeck_domain::MetadataStorage::from_code(code).ok());
        let Some(saved) =
            saved.filter(|saved| (mode == RestoreMode::FilesWin || fresh) && *saved != library.settings.metadata())
        else {
            return Ok(());
        };
        if let Ok(settings) = hoarddeck_domain::LibrarySettings::new(library.settings.access(), saved) {
            self.libraries.save_settings(library.id, settings)?;
        }
        Ok(())
    }

    fn apply_tag_rules(&self, rules: &[crate::tagging::TagRule]) -> Result<(), AppError> {
        self.tag_rules.save(rules)?;
        for library in self.libraries.list()? {
            self.editor.regroup_soon(library.id);
        }
        Ok(())
    }

    /// The root file and every entry with the time it was written.
    #[allow(clippy::type_complexity)]
    fn load(
        &self,
        root: &str,
        library: LibraryId,
        source: &RestoreSource,
        report: &mut RestoreReport,
    ) -> Result<(Option<RootFile>, Vec<(RelPath, bool, EntryFile, i64)>), AppError> {
        match source {
            RestoreSource::Backup(path) => {
                let backup = self.files.read_backup(path).map_err(AppError::InvalidInput)?;
                let written = backup.root.library.as_ref().map_or(0, |section| section.exported_at);
                let saved = backup
                    .entries
                    .into_iter()
                    .filter_map(|(file, entry)| {
                        let file = RelPath::new(file).ok()?;
                        Some((entry_path(&file)?, file.name() != ".3dhoarddeck", entry, written))
                    })
                    .collect();
                Ok((Some(backup.root), saved))
            }
            RestoreSource::Files => {
                let root_file = match self.files.read_root(root) {
                    Ok(found) => found.map(|(file, _)| file),
                    Err(error) => {
                        report.unreadable.push(error);
                        None
                    }
                };
                let mut saved = Vec::new();
                for file in self.entries.metadata_files(library)? {
                    let Some(path) = entry_path(&file) else { continue };
                    let single = file.name() != ".3dhoarddeck";
                    match self.files.read_entry(root, &file) {
                        Ok(Some((entry, written))) => saved.push((path, single, entry, written)),
                        Ok(None) => {}
                        Err(error) => report.unreadable.push(error),
                    }
                }
                Ok((root_file, saved))
            }
        }
    }

    /// Patterns from the root file, marks of single folders from the files in those folders (at the place they are
    /// now, so a folder renamed outside the app keeps its mark), and the other scan rules. Regroups once.
    fn restore_rules(
        &self,
        library: LibraryId,
        root: Option<&RootFile>,
        saved: &[(RelPath, bool, EntryFile, i64)],
        mode: RestoreMode,
    ) -> Result<usize, AppError> {
        let current = self.editor.rules(library)?;
        let mut wanted: Vec<FolderRule> = root
            .map(|file| file.folder_rules.iter().filter(|rule| exact_folder(rule).is_none()).cloned().collect())
            .unwrap_or_default();
        // Marks of single folders: the copy in the folder wins over the root file, which may name an old path.
        let marked: Vec<FolderRule> = saved
            .iter()
            .filter(|(_, single, _, _)| !single)
            .filter_map(|(path, _, entry, _)| Some(FolderRule::for_folder(path, entry.folder.as_ref()?.role?)))
            .collect();
        let has_folder_files =
            !marked.is_empty() || saved.iter().any(|(_, single, entry, _)| !single && entry.folder.is_some());
        if !has_folder_files {
            // Version 1 files: only the root file knows the marks.
            wanted.extend(
                root.map(|file| {
                    file.folder_rules.iter().filter(|rule| exact_folder(rule).is_some()).cloned().collect::<Vec<_>>()
                })
                .unwrap_or_default(),
            );
        }
        wanted.extend(marked);
        let changes: Vec<FolderRule> = wanted.into_iter().filter(|rule| !current.folder_rules.contains(rule)).collect();
        let scan =
            root.and_then(|file| file.library.as_ref()).map(|section| section.scan_rules.clone()).unwrap_or_default();
        let new_ignore: Vec<String> =
            scan.ignore.iter().filter(|glob| !current.ignore.contains(glob)).cloned().collect();
        let new_variants: Vec<String> =
            scan.variant_folders.iter().filter(|name| !current.variant_folders.contains(name)).cloned().collect();
        // A library without any folder rule yet is a fresh index: it takes the saved setting.
        let fresh = current.folder_rules.is_empty();
        let parent_covers = scan
            .parent_covers
            .filter(|value| (mode == RestoreMode::FilesWin || fresh) && *value != current.parent_covers);
        if changes.is_empty() && new_ignore.is_empty() && new_variants.is_empty() && parent_covers.is_none() {
            return Ok(0);
        }
        let count = changes.len() + new_ignore.len() + new_variants.len() + usize::from(parent_covers.is_some());
        self.editor.edit(library, |rules| {
            for rule in changes {
                rules.folder_rules.retain(|existing| existing.pattern != rule.pattern);
                rules.folder_rules.push(rule);
            }
            rules.ignore.extend(new_ignore);
            rules.variant_folders.extend(new_variants);
            if let Some(value) = parent_covers {
                rules.parent_covers = value;
            }
        })?;
        Ok(count)
    }

    fn restore_entries(
        &self,
        library: LibraryId,
        saved: &[(RelPath, bool, EntryFile, i64)],
        mode: RestoreMode,
        report: &mut RestoreReport,
    ) -> Result<(), AppError> {
        let paths: Vec<RelPath> = saved.iter().map(|(path, _, _, _)| path.clone()).collect();
        let current: HashMap<RelPath, _> = self
            .store
            .saved_entries(library, Some(&paths))?
            .into_iter()
            .map(|entry| (entry.path.clone(), entry))
            .collect();
        let mut restored = Vec::new();
        for (path, single, entry, written) in saved {
            let folder = if *single { path.parent().unwrap_or_default() } else { path.clone() };
            let mut meta = entry.meta.clone();
            // The cover relative to the folder still works after the folder was renamed outside the app.
            if let Some(cover) = entry.cover_in_folder.as_deref().and_then(|relative| resolve(relative, &folder)) {
                meta.cover = Some(cover);
            }
            let Ok(meta) = meta.normalized() else {
                report.unreadable.push(format!("{path}: invalid metadata"));
                continue;
            };
            let existing = current.get(path);
            // An empty file is data too: tags and notes the user removed must not come back from the index.
            let take_meta = existing.map_or(!meta.is_empty(), |known| known.meta != meta)
                && match mode {
                    RestoreMode::FilesWin => true,
                    RestoreMode::NewerWins => {
                        self.meta.meta_updated_at(library, path)?.is_none_or(|saved| *written > saved)
                    }
                };
            let fill = |known: bool| mode == RestoreMode::FilesWin || !known;
            let folder_file = entry.folder.as_ref();
            let folder_cover = folder_file
                .and_then(|file| {
                    file.cover
                        .as_deref()
                        .and_then(|relative| resolve(relative, path))
                        .or_else(|| file.cover_from_library.as_deref().and_then(|cover| RelPath::new(cover).ok()))
                })
                .filter(|cover| {
                    fill(existing.is_some_and(|known| known.folder_cover.is_some()))
                        && existing.is_none_or(|known| known.folder_cover.as_ref() != Some(cover))
                });
            let skipped = entry.rendered_preview.as_deref() == Some("skipped")
                && existing.is_none_or(|known| known.rendered.as_deref() != Some("skipped"))
                && fill(existing.is_some_and(|known| known.rendered.is_some()));
            let added_at = entry.added_at.filter(|at| existing.is_none_or(|known| known.added_at != Some(*at)));
            if !take_meta && folder_cover.is_none() && !skipped && added_at.is_none() {
                report.unchanged += 1;
                continue;
            }
            report.restored += 1;
            restored.push(RestoredEntry {
                path: path.clone(),
                meta: take_meta.then_some((meta, *written)),
                added_at,
                rendered_skipped: skipped,
                folder_cover,
            });
        }
        self.store.restore_entries(library, &restored)?;
        Ok(())
    }
}
