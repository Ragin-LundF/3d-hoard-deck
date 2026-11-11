//! The user data metadata files keep: read for writing them, written when restoring from them.

use std::collections::BTreeSet;
use std::time::{SystemTime, UNIX_EPOCH};

use hoarddeck_application::IndexError;
use hoarddeck_application::ports::{RestoredEntry, SavedEntry, SavedLibraryData, UserDataStore};
use hoarddeck_application::sidecar::{SavedCollection, SavedTagGroup};
use hoarddeck_domain::{LibraryId, ModelKind, RelPath};
use rusqlite::{Connection, OptionalExtension, params};

use crate::SqliteIndex;
use crate::meta::meta_row;
use crate::rows::{converted, optional_rel_path, rel_path};

/// id, name, query, library, scope of a collection.
type CollectionRow = (i64, String, Option<String>, Option<i64>, Option<String>);

fn now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
}

impl UserDataStore for SqliteIndex {
    fn saved_entries(&self, library: LibraryId, paths: Option<&[RelPath]>) -> Result<Vec<SavedEntry>, IndexError> {
        self.read(|connection| {
            let paths: Vec<RelPath> = match paths {
                Some(paths) => paths.to_vec(),
                None => data_paths(connection, library)?,
            };
            paths.into_iter().map(|path| saved_entry(connection, library, path)).collect()
        })
    }

    fn saved_library(&self, library: LibraryId) -> Result<SavedLibraryData, IndexError> {
        self.read(|connection| {
            let mut groups = Vec::new();
            let mut statement =
                connection.prepare("SELECT id, name, color, position FROM tag_groups WHERE library_id = ?1 ORDER BY position, id")?;
            let rows: Vec<(i64, String, String, i64)> = statement
                .query_map([library.0], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)))?
                .collect::<rusqlite::Result<_>>()?;
            for (id, name, color, position) in rows {
                let tags = strings(connection, "SELECT key FROM tag_info WHERE library_id = ?1 AND group_id = ?2 ORDER BY key", params![library.0, id])?;
                groups.push(SavedTagGroup { name, color, position, tags });
            }
            let pinned_tags =
                strings(connection, "SELECT key FROM tag_info WHERE library_id = ?1 AND pinned = 1 ORDER BY key", params![library.0])?;
            let mut collections = Vec::new();
            let mut statement = connection.prepare(
                "SELECT id, name, query, library_id, scope FROM collections c
                 WHERE (c.query IS NULL AND EXISTS (SELECT 1 FROM collection_items i WHERE i.collection_id = c.id AND i.library_id = ?1))
                    OR (c.query IS NOT NULL AND (c.library_id = ?1 OR c.library_id IS NULL))
                 ORDER BY c.id",
            )?;
            let rows: Vec<CollectionRow> = statement
                .query_map([library.0], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)))?
                .collect::<rusqlite::Result<_>>()?;
            for (id, name, query, owner, scope) in rows {
                let members = if query.is_none() {
                    strings(connection, "SELECT path FROM collection_items WHERE collection_id = ?1 AND library_id = ?2 ORDER BY position", params![id, library.0])?
                } else {
                    Vec::new()
                };
                let library_only = query.is_some() && owner == Some(library.0) && scope.is_none();
                collections.push(SavedCollection { name, members, query, scope, library_only });
            }
            Ok(SavedLibraryData { tag_groups: groups, pinned_tags, collections })
        })
    }

    fn restore_entries(&self, library: LibraryId, entries: &[RestoredEntry]) -> Result<(), IndexError> {
        let rows = entries
            .iter()
            .map(|entry| {
                let meta = entry
                    .meta
                    .as_ref()
                    .map(|(meta, written)| {
                        let tags = serde_json::to_string(&meta.tags).map_err(|error| IndexError(error.to_string()))?;
                        let hidden =
                            serde_json::to_string(&meta.hidden_tags).map_err(|error| IndexError(error.to_string()))?;
                        Ok((meta, *written, tags, hidden))
                    })
                    .transpose()?;
                Ok((entry, meta))
            })
            .collect::<Result<Vec<_>, IndexError>>()?;
        self.write(|connection| {
            let transaction = connection.transaction()?;
            for (entry, meta) in &rows {
                let path = entry.path.as_str();
                if let Some((meta, written, tags, hidden)) = meta {
                    transaction.execute(
                        "INSERT INTO model_meta (library_id, path, display_name, favorite, printed, notes, tags, cover, updated_at, hidden_tags)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
                         ON CONFLICT (library_id, path) DO UPDATE SET
                            display_name = ?3, favorite = ?4, printed = ?5, notes = ?6, tags = ?7, cover = ?8, updated_at = ?9,
                            hidden_tags = ?10",
                        params![
                            library.0,
                            path,
                            meta.display_name,
                            meta.favorite,
                            meta.printed,
                            meta.notes,
                            tags,
                            meta.cover.as_ref().map(RelPath::as_str),
                            written,
                            hidden
                        ],
                    )?;
                }
                if let Some(added) = entry.added_at {
                    transaction.execute("UPDATE models SET added_at = ?3 WHERE library_id = ?1 AND path = ?2", params![library.0, path, added])?;
                }
                if entry.rendered_skipped {
                    transaction.execute(
                        "INSERT INTO rendered_previews (library_id, path, state) VALUES (?1, ?2, 'skipped')
                         ON CONFLICT (library_id, path) DO UPDATE SET state = 'skipped'",
                        params![library.0, path],
                    )?;
                }
                if let Some(cover) = &entry.folder_cover {
                    transaction.execute(
                        "INSERT INTO folder_covers (library_id, path, cover) VALUES (?1, ?2, ?3)
                         ON CONFLICT (library_id, path) DO UPDATE SET cover = ?3",
                        params![library.0, path, cover.as_str()],
                    )?;
                }
            }
            transaction.commit()
        })
    }

    fn restore_library(&self, library: LibraryId, data: &SavedLibraryData, replace: bool) -> Result<usize, IndexError> {
        self.write(|connection| {
            let transaction = connection.transaction()?;
            for group in &data.tag_groups {
                let existing: Option<i64> = transaction
                    .query_row("SELECT id FROM tag_groups WHERE library_id = ?1 AND name = ?2", params![library.0, group.name], |row| row.get(0))
                    .optional()?;
                let id = if let Some(id) = existing {
                    if replace {
                        transaction.execute("UPDATE tag_groups SET color = ?2, position = ?3 WHERE id = ?1", params![id, group.color, group.position])?;
                    }
                    id
                } else {
                    transaction.execute(
                        "INSERT INTO tag_groups (library_id, name, color, position) VALUES (?1, ?2, ?3, ?4)",
                        params![library.0, group.name, group.color, group.position],
                    )?;
                    transaction.last_insert_rowid()
                };
                for key in &group.tags {
                    transaction.execute(
                        "INSERT INTO tag_info (library_id, key, group_id, pinned) VALUES (?1, ?2, ?3, 0)
                         ON CONFLICT (library_id, key) DO UPDATE SET group_id = CASE WHEN ?4 OR group_id IS NULL THEN ?3 ELSE group_id END",
                        params![library.0, key, id, replace],
                    )?;
                }
            }
            for key in &data.pinned_tags {
                transaction.execute(
                    "INSERT INTO tag_info (library_id, key, group_id, pinned) VALUES (?1, ?2, NULL, 1)
                     ON CONFLICT (library_id, key) DO UPDATE SET pinned = 1",
                    params![library.0, key],
                )?;
            }
            let mut dropped = 0;
            for saved in &data.collections {
                let smart = saved.query.is_some();
                let existing: Option<i64> = transaction
                    .query_row(
                        "SELECT id FROM collections WHERE name = ?1 AND (query IS NULL) = ?2",
                        params![saved.name, !smart],
                        |row| row.get(0),
                    )
                    .optional()?;
                let id = if let Some(id) = existing {
                    id
                } else {
                    let owner = (saved.library_only || saved.scope.is_some()).then_some(library.0).filter(|_| smart);
                    transaction.execute(
                        "INSERT INTO collections (name, query, library_id, scope, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
                        params![saved.name, saved.query, owner, saved.scope, now()],
                    )?;
                    transaction.last_insert_rowid()
                };
                if smart {
                    continue;
                }
                if replace {
                    transaction.execute("DELETE FROM collection_items WHERE collection_id = ?1 AND library_id = ?2", params![id, library.0])?;
                }
                let next_position: i64 = transaction.query_row(
                    "SELECT COALESCE(MAX(position) + 1, 0) FROM collection_items WHERE collection_id = ?1",
                    [id],
                    |row| row.get(0),
                )?;
                for (offset, member) in saved.members.iter().enumerate() {
                    let exists: bool = transaction.query_row(
                        "SELECT EXISTS (SELECT 1 FROM models WHERE library_id = ?1 AND path = ?2)",
                        params![library.0, member],
                        |row| row.get(0),
                    )?;
                    if !exists {
                        dropped += 1;
                        continue;
                    }
                    transaction.execute(
                        "INSERT INTO collection_items (collection_id, library_id, path, position) VALUES (?1, ?2, ?3, ?4)
                         ON CONFLICT DO NOTHING",
                        params![id, library.0, member, next_position + i64::try_from(offset).unwrap_or(0)],
                    )?;
                }
            }
            transaction.commit()?;
            Ok(dropped)
        })
    }

    fn mark_pending(&self, library: LibraryId, paths: &[RelPath]) -> Result<(), IndexError> {
        if paths.is_empty() {
            return Ok(());
        }
        self.write(|connection| {
            let transaction = connection.transaction()?;
            for path in paths {
                transaction.execute(
                    "INSERT INTO sidecar_pending (library_id, path) VALUES (?1, ?2) ON CONFLICT DO NOTHING",
                    params![library.0, path.as_str()],
                )?;
            }
            transaction.commit()
        })
    }

    fn pending(&self, library: LibraryId) -> Result<Vec<RelPath>, IndexError> {
        self.read(|connection| {
            let mut statement =
                connection.prepare("SELECT path FROM sidecar_pending WHERE library_id = ?1 ORDER BY path")?;
            statement.query_map([library.0], |row| rel_path(row, 0))?.collect()
        })
    }

    fn clear_pending(&self, library: LibraryId, paths: &[RelPath]) -> Result<(), IndexError> {
        if paths.is_empty() {
            return Ok(());
        }
        self.write(|connection| {
            let transaction = connection.transaction()?;
            for path in paths {
                transaction.execute(
                    "DELETE FROM sidecar_pending WHERE library_id = ?1 AND path = ?2",
                    params![library.0, path.as_str()],
                )?;
            }
            transaction.commit()
        })
    }

    fn library_uid(&self, library: LibraryId, adopt: Option<&str>) -> Result<String, IndexError> {
        self.write(|connection| {
            if let Some(uid) = adopt {
                connection.execute(
                    "INSERT INTO sidecar_status (library_id, uid) VALUES (?1, ?2) ON CONFLICT (library_id) DO UPDATE SET uid = ?2",
                    params![library.0, uid],
                )?;
                return Ok(uid.to_owned());
            }
            let known: Option<Option<String>> =
                connection.query_row("SELECT uid FROM sidecar_status WHERE library_id = ?1", [library.0], |row| row.get(0)).optional()?;
            if let Some(Some(uid)) = known {
                return Ok(uid);
            }
            let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_nanos());
            let uid = format!("{nanos:x}-{:x}", library.0);
            connection.execute(
                "INSERT INTO sidecar_status (library_id, uid) VALUES (?1, ?2) ON CONFLICT (library_id) DO UPDATE SET uid = ?2",
                params![library.0, uid],
            )?;
            Ok(uid)
        })
    }

    fn set_exported_at(&self, library: LibraryId, at: i64) -> Result<(), IndexError> {
        self.write(|connection| {
            connection.execute(
                "INSERT INTO sidecar_status (library_id, exported_at) VALUES (?1, ?2) ON CONFLICT (library_id) DO UPDATE SET exported_at = ?2",
                params![library.0, at],
            )?;
            Ok(())
        })
    }

    fn exported_at(&self, library: LibraryId) -> Result<Option<i64>, IndexError> {
        self.read(|connection| {
            Ok(connection
                .query_row("SELECT exported_at FROM sidecar_status WHERE library_id = ?1", [library.0], |row| {
                    row.get(0)
                })
                .optional()?
                .flatten())
        })
    }
}

/// Paths with any user data: metadata, a picked folder cover, or "no rendered preview".
fn data_paths(connection: &Connection, library: LibraryId) -> rusqlite::Result<Vec<RelPath>> {
    let mut statement = connection.prepare(
        "SELECT path FROM model_meta WHERE library_id = ?1
         UNION SELECT path FROM folder_covers WHERE library_id = ?1
         UNION SELECT path FROM rendered_previews WHERE library_id = ?1 AND state = 'skipped'",
    )?;
    let paths: BTreeSet<RelPath> =
        statement.query_map([library.0], |row| rel_path(row, 0))?.collect::<rusqlite::Result<_>>()?;
    Ok(paths.into_iter().collect())
}

fn saved_entry(connection: &Connection, library: LibraryId, path: RelPath) -> rusqlite::Result<SavedEntry> {
    let model: Option<(String, i64)> = connection
        .prepare_cached("SELECT kind, added_at FROM models WHERE library_id = ?1 AND path = ?2")?
        .query_row(params![library.0, path.as_str()], |row| Ok((row.get(0)?, row.get(1)?)))
        .optional()?;
    let kind = match &model {
        Some((kind, _)) => Some(converted(0, ModelKind::from_code(kind))?),
        // No model here (anymore), e.g. a file that is now part of a folder model: the scanned entry tells whether
        // the path is a file or a folder; a path that no longer exists has no kind.
        None => connection
            .prepare_cached("SELECT is_dir FROM entries WHERE library_id = ?1 AND path = ?2")?
            .query_row(params![library.0, path.as_str()], |row| row.get::<_, bool>(0))
            .optional()?
            .map(|is_dir| if is_dir { ModelKind::Folder } else { ModelKind::File }),
    };
    let rendered: Option<String> = connection
        .prepare_cached("SELECT state FROM rendered_previews WHERE library_id = ?1 AND path = ?2")?
        .query_row(params![library.0, path.as_str()], |row| row.get(0))
        .optional()?;
    let folder_cover = connection
        .prepare_cached("SELECT cover FROM folder_covers WHERE library_id = ?1 AND path = ?2")?
        .query_row(params![library.0, path.as_str()], |row| optional_rel_path(row, 0))
        .optional()?
        .flatten();
    Ok(SavedEntry {
        meta: meta_row(connection, library, &path)?,
        kind,
        added_at: model.map(|(_, added)| added),
        rendered,
        folder_cover,
        path,
    })
}

fn strings(connection: &Connection, sql: &str, params: impl rusqlite::Params) -> rusqlite::Result<Vec<String>> {
    let mut statement = connection.prepare_cached(sql)?;
    statement.query_map(params, |row| row.get(0))?.collect()
}
