//! Raw walk results: the `EntryStore` port.

mod writes;

use std::time::{SystemTime, UNIX_EPOCH};

use hoarddeck_application::IndexError;
use hoarddeck_application::ports::{EntryStore, SizedFile};
use hoarddeck_application::scan::{Generation, KnownDir, KnownDirs, ScanItem};
use hoarddeck_domain::{Entry, EntryKind, FileKind, LibraryId, RelPath};
use rusqlite::{Transaction, params};

use crate::SqliteIndex;
use crate::rows::{converted, rel_path, subtree_bounds, to_i64, to_u64};

impl EntryStore for SqliteIndex {
    fn begin_scan(&self, library: LibraryId) -> Result<(Generation, KnownDirs), IndexError> {
        let generation = self.write(|connection| {
            connection.query_row(
                "UPDATE libraries SET scan_generation = scan_generation + 1 WHERE id = ?1 RETURNING scan_generation",
                [library.0],
                |row| row.get(0),
            )
        })?;
        Ok((Generation(generation), self.known_dirs(library)?))
    }

    fn apply(&self, library: LibraryId, generation: Generation, items: &[ScanItem]) -> Result<(), IndexError> {
        if items.is_empty() {
            return Ok(());
        }
        self.write(|connection| {
            let transaction = connection.transaction()?;
            for item in items {
                writes::apply_item(&transaction, library, generation, item)?;
            }
            transaction.commit()
        })
    }

    fn finish_scan(&self, library: LibraryId, generation: Generation) -> Result<u64, IndexError> {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| to_i64(d.as_secs()));
        self.write(|connection| {
            let transaction = connection.transaction()?;
            let removed = remove_missing(&transaction, library, generation)?;
            transaction.execute("UPDATE libraries SET last_scan_at = ?2 WHERE id = ?1", params![library.0, now])?;
            transaction.commit()?;
            Ok(removed)
        })
    }

    fn finish_folder_scan(
        &self,
        library: LibraryId,
        generation: Generation,
        folder: &RelPath,
        visited: bool,
    ) -> Result<u64, IndexError> {
        if folder.is_root() {
            return self.finish_scan(library, generation);
        }
        self.write(|connection| {
            let transaction = connection.transaction()?;
            let removed = if visited {
                remove_missing(&transaction, library, generation)?
            } else {
                remove_subtree(&transaction, library, folder)?
            };
            transaction.commit()?;
            Ok(removed)
        })
    }

    fn folders(&self, library: LibraryId) -> Result<Vec<RelPath>, IndexError> {
        self.read(|connection| {
            let mut statement = connection.prepare_cached(
                "SELECT path FROM entries WHERE library_id = ?1 AND is_dir = 1 AND path <> '' ORDER BY path",
            )?;
            statement.query_map([library.0], |row| rel_path(row, 0))?.collect()
        })
    }

    fn same_size_files(&self, min_size: u64) -> Result<Vec<Vec<SizedFile>>, IndexError> {
        self.read(|connection| {
            // Model files only: meshes, slicer projects and archives.
            let mut statement = connection.prepare(
                "SELECT library_id, path, size, mtime FROM entries
                 WHERE is_dir = 0 AND size >= ?1 AND kind NOT IN ('image', 'doc', 'other') AND size IN (
                     SELECT size FROM entries
                     WHERE is_dir = 0 AND size >= ?1 AND kind NOT IN ('image', 'doc', 'other')
                     GROUP BY size HAVING COUNT(*) > 1)
                 ORDER BY size DESC, library_id, path",
            )?;
            let rows = statement.query_map([to_i64(min_size)], |row| {
                Ok((LibraryId(row.get(0)?), rel_path(row, 1)?, to_u64(row.get(2)?), row.get(3)?))
            })?;
            let mut groups: Vec<Vec<SizedFile>> = Vec::new();
            for row in rows {
                let row = row?;
                match groups.last_mut() {
                    Some(group) if group[0].2 == row.2 => group.push(row),
                    _ => groups.push(vec![row]),
                }
            }
            Ok(groups)
        })
    }

    fn files(&self, library: LibraryId) -> Result<Vec<Entry>, IndexError> {
        self.read(|connection| {
            let mut statement = connection
                .prepare_cached("SELECT path, kind, size, mtime FROM entries WHERE library_id = ?1 AND is_dir = 0")?;
            statement
                .query_map([library.0], |row| {
                    Ok(Entry {
                        path: rel_path(row, 0)?,
                        kind: EntryKind::File(converted(1, FileKind::from_code(&row.get::<_, String>(1)?))?),
                        size: to_u64(row.get(2)?),
                        mtime: row.get(3)?,
                    })
                })?
                .collect()
        })
    }
}

impl SqliteIndex {
    fn known_dirs(&self, library: LibraryId) -> Result<KnownDirs, IndexError> {
        self.read(|connection| {
            let mut known = KnownDirs::new();
            let mut listed = connection.prepare(
                "SELECT path, listed_mtime FROM entries WHERE library_id = ?1 AND is_dir = 1 AND listed_mtime IS NOT NULL",
            )?;
            for row in listed.query_map([library.0], |row| Ok((rel_path(row, 0)?, row.get::<_, i64>(1)?)))? {
                let (path, mtime) = row?;
                known.insert(path, KnownDir { mtime, child_dirs: Vec::new() });
            }
            let mut children =
                connection.prepare("SELECT parent, path FROM entries WHERE library_id = ?1 AND is_dir = 1 AND path <> ''")?;
            for row in children.query_map([library.0], |row| Ok((rel_path(row, 0)?, rel_path(row, 1)?)))? {
                let (parent, child) = row?;
                if let Some(dir) = known.get_mut(&parent) {
                    dir.child_dirs.push(child.name().to_owned());
                }
            }
            Ok(known)
        })
    }
}

/// Removes what a scan did not see in the folders it listed, and everything below subfolders that vanished from
/// them. Folders that were only visited (unchanged mtime) or could not be read keep their contents.
fn remove_missing(transaction: &Transaction<'_>, library: LibraryId, generation: Generation) -> rusqlite::Result<u64> {
    const IN_LISTED: &str = "library_id = ?1 AND generation < ?2
        AND parent IN (SELECT path FROM entries INDEXED BY entries_listed WHERE library_id = ?1 AND listed_gen = ?2)";
    let vanished: Vec<RelPath> = {
        let mut statement =
            transaction.prepare_cached(&format!("SELECT path FROM entries WHERE is_dir = 1 AND {IN_LISTED}"))?;
        statement
            .query_map(params![library.0, generation.0], |row| rel_path(row, 0))?
            .collect::<rusqlite::Result<_>>()?
    };
    let mut removed = 0;
    for dir in &vanished {
        removed += remove_subtree(transaction, library, dir)?;
    }
    removed += transaction
        .execute(&format!("DELETE FROM entries WHERE {IN_LISTED}"), params![library.0, generation.0])?
        as u64;
    Ok(removed)
}

/// Removes a folder and everything below it.
fn remove_subtree(transaction: &Transaction<'_>, library: LibraryId, dir: &RelPath) -> rusqlite::Result<u64> {
    let (low, high) = subtree_bounds(dir);
    let removed = transaction.execute(
        "DELETE FROM entries WHERE library_id = ?1 AND (path = ?2 OR (path >= ?3 AND path < ?4))",
        params![library.0, dir.as_str(), low, high],
    )?;
    Ok(removed as u64)
}
