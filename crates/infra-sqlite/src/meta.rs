use std::time::{SystemTime, UNIX_EPOCH};

use hoarddeck_application::IndexError;
use hoarddeck_application::ports::MetaStore;
use hoarddeck_domain::{LibraryId, ModelMeta, RelPath, TagName};
use rusqlite::{Connection, OptionalExtension, params};

use crate::SqliteIndex;
use crate::rows::converted;

impl MetaStore for SqliteIndex {
    fn meta(&self, library: LibraryId, path: &RelPath) -> Result<ModelMeta, IndexError> {
        self.read(|connection| meta_row(connection, library, path))
    }

    fn save_meta(&self, library: LibraryId, path: &RelPath, meta: &ModelMeta) -> Result<(), IndexError> {
        let tags = serde_json::to_string(&meta.tags).map_err(|error| IndexError(error.to_string()))?;
        let now =
            SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX));
        self.write(|connection| {
            if meta.is_empty() {
                connection.execute(
                    "DELETE FROM model_meta WHERE library_id = ?1 AND path = ?2",
                    params![library.0, path.as_str()],
                )?;
            } else {
                connection.execute(
                    "INSERT INTO model_meta (library_id, path, display_name, favorite, printed, notes, tags, cover, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                     ON CONFLICT (library_id, path) DO UPDATE SET
                        display_name = ?3, favorite = ?4, printed = ?5, notes = ?6, tags = ?7, cover = ?8, updated_at = ?9",
                    params![
                        library.0,
                        path.as_str(),
                        meta.display_name,
                        meta.favorite,
                        meta.printed,
                        meta.notes,
                        tags,
                        meta.cover.as_ref().map(RelPath::as_str),
                        now
                    ],
                )?;
            }
            Ok(())
        })
    }

    fn meta_updated_at(&self, library: LibraryId, path: &RelPath) -> Result<Option<i64>, IndexError> {
        self.read(|connection| {
            connection
                .query_row(
                    "SELECT updated_at FROM model_meta WHERE library_id = ?1 AND path = ?2",
                    params![library.0, path.as_str()],
                    |row| row.get(0),
                )
                .optional()
        })
    }

    fn tags(&self, library: LibraryId) -> Result<Vec<(TagName, u32)>, IndexError> {
        self.read(|connection| {
            let mut statement = connection.prepare_cached(
                "SELECT MIN(tag.value), COUNT(*) FROM model_meta, json_each(model_meta.tags) AS tag
                 WHERE model_meta.library_id = ?1 GROUP BY lower(tag.value) ORDER BY COUNT(*) DESC, lower(tag.value)",
            )?;
            statement
                .query_map([library.0], |row| {
                    Ok((converted(0, TagName::new(&row.get::<_, String>(0)?))?, row.get(1)?))
                })?
                .collect()
        })
    }
}

pub(crate) fn meta_row(connection: &Connection, library: LibraryId, path: &RelPath) -> rusqlite::Result<ModelMeta> {
    let row = connection
        .query_row(
            "SELECT display_name, favorite, printed, notes, tags, cover FROM model_meta WHERE library_id = ?1 AND path = ?2",
            params![library.0, path.as_str()],
            |row| {
                Ok(ModelMeta {
                    display_name: row.get(0)?,
                    favorite: row.get(1)?,
                    printed: row.get(2)?,
                    notes: row.get(3)?,
                    tags: converted(4, serde_json::from_str(&row.get::<_, String>(4)?))?,
                    cover: crate::rows::optional_rel_path(row, 5)?,
                })
            },
        )
        .optional()?;
    Ok(row.unwrap_or_default())
}
