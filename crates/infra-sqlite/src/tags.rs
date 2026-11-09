use hoarddeck_application::IndexError;
use hoarddeck_application::ports::{TagGroup, TagInfo, TagScope, TagStore};
use hoarddeck_domain::{LibraryId, RelPath, TagName};
use rusqlite::types::Value;
use rusqlite::{params, params_from_iter};

use crate::SqliteIndex;
use crate::rows::{converted, rel_path, subtree_bounds};

/// Tag names as a JSON array for `json_each`. Keys are always made with the `tag_key()` SQL function, on both sides, so they
/// match the tags stored in `model_meta` whatever the letters.
fn names_json(tags: &[TagName]) -> String {
    serde_json::to_string(&tags.iter().map(TagName::as_str).collect::<Vec<_>>()).unwrap_or_else(|_| "[]".to_owned())
}

impl TagStore for SqliteIndex {
    fn tag_infos(&self, library: LibraryId) -> Result<Vec<TagInfo>, IndexError> {
        self.read(|connection| {
            let mut statement = connection.prepare_cached(
                "WITH t(id, value, updated) AS (
                    SELECT m.id, j.value, mm.updated_at FROM models m
                    JOIN model_meta mm ON mm.library_id = m.library_id AND mm.path = m.path, json_each(mm.tags) j
                    WHERE m.library_id = ?1
                    UNION
                    SELECT m.id, j.value, coalesce(mm.updated_at, 0) FROM models m
                    LEFT JOIN model_meta mm ON mm.library_id = m.library_id AND mm.path = m.path, json_each(m.auto_tags) j
                    WHERE m.library_id = ?1 AND NOT EXISTS
                        (SELECT 1 FROM json_each(mm.hidden_tags) h WHERE tag_key(h.value) = tag_key(j.value))
                 )
                 SELECT MIN(t.value), COUNT(DISTINCT t.id), i.group_id, coalesce(i.pinned, 0), MAX(t.updated)
                 FROM t LEFT JOIN tag_info i ON i.library_id = ?1 AND i.key = tag_key(t.value)
                 GROUP BY tag_key(t.value) ORDER BY COUNT(DISTINCT t.id) DESC, tag_key(t.value)",
            )?;
            statement
                .query_map([library.0], |row| {
                    Ok(TagInfo {
                        name: converted(0, TagName::new(&row.get::<_, String>(0)?))?,
                        count: row.get(1)?,
                        group: row.get(2)?,
                        pinned: row.get(3)?,
                        recent: row.get(4)?,
                    })
                })?
                .collect()
        })
    }

    fn tag_counts(
        &self,
        library: LibraryId,
        scope: &TagScope,
        limit: usize,
    ) -> Result<Vec<(TagName, u32)>, IndexError> {
        let limit = i64::try_from(limit).unwrap_or(i64::MAX);
        let mut values: Vec<Value> = vec![library.0.into(), limit.into()];
        let filter = match scope {
            TagScope::Folder(folder) if folder.is_root() => "1",
            TagScope::Folder(folder) => {
                let (low, high) = subtree_bounds(folder);
                values.extend([folder.as_str().to_owned().into(), low.into(), high.into()]);
                "(m.folder = ?3 OR (m.folder >= ?4 AND m.folder < ?5))"
            }
            TagScope::Models(ids) => {
                values.push(
                    serde_json::to_string(&ids.iter().map(|id| id.0).collect::<Vec<_>>()).unwrap_or_default().into(),
                );
                "m.id IN (SELECT value FROM json_each(?3))"
            }
        };
        // The user's tags and the rule tags they did not hide, each model counted once per tag.
        let sql = format!(
            "WITH t(id, value) AS (
                SELECT m.id, j.value FROM models m
                JOIN model_meta mm ON mm.library_id = m.library_id AND mm.path = m.path, json_each(mm.tags) j
                WHERE m.library_id = ?1 AND {filter}
                UNION
                SELECT m.id, j.value FROM models m
                LEFT JOIN model_meta mm ON mm.library_id = m.library_id AND mm.path = m.path, json_each(m.auto_tags) j
                WHERE m.library_id = ?1 AND {filter} AND NOT EXISTS
                    (SELECT 1 FROM json_each(mm.hidden_tags) h WHERE tag_key(h.value) = tag_key(j.value))
             )
             SELECT MIN(value), COUNT(DISTINCT id) FROM t
             GROUP BY tag_key(value) ORDER BY COUNT(DISTINCT id) DESC, tag_key(value) LIMIT ?2"
        );
        self.read(|connection| {
            let mut statement = connection.prepare_cached(&sql)?;
            let map =
                |row: &rusqlite::Row<'_>| Ok((converted(0, TagName::new(&row.get::<_, String>(0)?))?, row.get(1)?));
            let rows = statement.query_map(params_from_iter(values), map)?;
            rows.collect()
        })
    }

    fn paths_with_tags(&self, library: LibraryId, tags: &[TagName]) -> Result<Vec<RelPath>, IndexError> {
        self.read(|connection| {
            let mut statement = connection.prepare_cached(
                "SELECT mm.path FROM model_meta mm, json_each(mm.tags) t
                 WHERE mm.library_id = ?1 AND tag_key(t.value) IN (SELECT tag_key(value) FROM json_each(?2))
                 UNION
                 SELECT m.path FROM models m, json_each(m.auto_tags) t
                 WHERE m.library_id = ?1 AND tag_key(t.value) IN (SELECT tag_key(value) FROM json_each(?2))",
            )?;
            statement.query_map(params![library.0, names_json(tags)], |row| rel_path(row, 0))?.collect()
        })
    }

    fn move_tag_info(&self, library: LibraryId, from: &[TagName], to: &TagName) -> Result<(), IndexError> {
        self.write(|connection| {
            let transaction = connection.transaction()?;
            transaction.execute(
                "INSERT OR IGNORE INTO tag_info (library_id, key, group_id, pinned)
                 SELECT library_id, tag_key(?3), group_id, pinned FROM tag_info
                 WHERE library_id = ?1 AND key IN (SELECT tag_key(value) FROM json_each(?2)) LIMIT 1",
                params![library.0, names_json(from), to.as_str()],
            )?;
            transaction.execute(
                "DELETE FROM tag_info WHERE library_id = ?1 AND key IN (SELECT tag_key(value) FROM json_each(?2)) AND key <> tag_key(?3)",
                params![library.0, names_json(from), to.as_str()],
            )?;
            transaction.commit()
        })
    }

    fn forget_tags(&self, library: LibraryId, tags: &[TagName]) -> Result<(), IndexError> {
        self.write(|connection| {
            connection
                .execute(
                    "DELETE FROM tag_info WHERE library_id = ?1 AND key IN (SELECT tag_key(value) FROM json_each(?2))",
                    params![library.0, names_json(tags)],
                )
                .map(|_| ())
        })
    }

    fn set_tag_group(&self, library: LibraryId, tags: &[TagName], group: Option<i64>) -> Result<(), IndexError> {
        self.write(|connection| {
            connection
                .execute(
                    "INSERT INTO tag_info (library_id, key, group_id) SELECT ?1, tag_key(value), ?3 FROM json_each(?2) WHERE true
                     ON CONFLICT (library_id, key) DO UPDATE SET group_id = excluded.group_id",
                    params![library.0, names_json(tags), group],
                )
                .map(|_| ())
        })
    }

    fn set_tag_pinned(&self, library: LibraryId, tags: &[TagName], pinned: bool) -> Result<(), IndexError> {
        self.write(|connection| {
            connection
                .execute(
                    "INSERT INTO tag_info (library_id, key, pinned) SELECT ?1, tag_key(value), ?3 FROM json_each(?2) WHERE true
                     ON CONFLICT (library_id, key) DO UPDATE SET pinned = excluded.pinned",
                    params![library.0, names_json(tags), pinned],
                )
                .map(|_| ())
        })
    }

    fn tag_groups(&self, library: LibraryId) -> Result<Vec<TagGroup>, IndexError> {
        self.read(|connection| {
            let mut statement = connection
                .prepare_cached("SELECT id, name, color FROM tag_groups WHERE library_id = ?1 ORDER BY position, id")?;
            statement
                .query_map([library.0], |row| Ok(TagGroup { id: row.get(0)?, name: row.get(1)?, color: row.get(2)? }))?
                .collect()
        })
    }

    fn save_tag_group(&self, library: LibraryId, id: Option<i64>, name: &str, color: &str) -> Result<i64, IndexError> {
        self.write(|connection| {
            if let Some(id) = id {
                connection.execute(
                    "UPDATE tag_groups SET name = ?3, color = ?4 WHERE library_id = ?1 AND id = ?2",
                    params![library.0, id, name, color],
                )?;
                return Ok(id);
            }
            connection.execute(
                "INSERT INTO tag_groups (library_id, name, color, position)
                 VALUES (?1, ?2, ?3, (SELECT coalesce(MAX(position), 0) + 1 FROM tag_groups WHERE library_id = ?1))",
                params![library.0, name, color],
            )?;
            Ok(connection.last_insert_rowid())
        })
    }

    fn delete_tag_group(&self, library: LibraryId, id: i64) -> Result<(), IndexError> {
        self.write(|connection| {
            connection
                .execute("DELETE FROM tag_groups WHERE library_id = ?1 AND id = ?2", params![library.0, id])
                .map(|_| ())
        })
    }
}
