use std::time::{SystemTime, UNIX_EPOCH};

use hoarddeck_application::IndexError;
use hoarddeck_application::ai::AiSettings;
use hoarddeck_application::ports::AiStore;
use hoarddeck_domain::{LibraryId, RelPath, TagName};
use rusqlite::{OptionalExtension, params};

use crate::SqliteIndex;
use crate::rows::{converted, rel_path};

const AI: &str = "ai";

fn json(tags: &[TagName]) -> String {
    serde_json::to_string(tags).unwrap_or_else(|_| "[]".to_owned())
}

impl AiStore for SqliteIndex {
    fn load_ai_settings(&self) -> Result<AiSettings, IndexError> {
        let stored: Option<String> = self.read(|connection| {
            connection.query_row("SELECT value FROM app_settings WHERE key = ?1", [AI], |row| row.get(0)).optional()
        })?;
        // Settings an older or newer app wrote fall back to "off" rather than keeping the app from opening.
        Ok(stored.and_then(|text| serde_json::from_str(&text).ok()).unwrap_or_default())
    }

    fn save_ai_settings(&self, settings: &AiSettings) -> Result<(), IndexError> {
        let text = serde_json::to_string(settings).map_err(|error| IndexError(error.to_string()))?;
        self.write(|connection| {
            connection
                .execute(
                    "INSERT INTO app_settings (key, value) VALUES (?1, ?2) ON CONFLICT (key) DO UPDATE SET value = ?2",
                    params![AI, text],
                )
                .map(|_| ())
        })
    }

    fn suggestion(&self, library: LibraryId, path: &RelPath) -> Result<Option<(Vec<TagName>, String)>, IndexError> {
        self.read(|connection| {
            connection
                .query_row(
                    "SELECT tags, source FROM tag_suggestions WHERE library_id = ?1 AND path = ?2",
                    params![library.0, path.as_str()],
                    |row| Ok((converted(0, serde_json::from_str(&row.get::<_, String>(0)?))?, row.get(1)?)),
                )
                .optional()
        })
    }

    fn save_suggestion(
        &self,
        library: LibraryId,
        path: &RelPath,
        tags: &[TagName],
        source: &str,
    ) -> Result<(), IndexError> {
        let now =
            SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX));
        self.write(|connection| {
            connection
                .execute(
                    "INSERT INTO tag_suggestions (library_id, path, tags, source, created_at) VALUES (?1, ?2, ?3, ?4, ?5)
                     ON CONFLICT (library_id, path) DO UPDATE SET tags = ?3, source = ?4, created_at = ?5",
                    params![library.0, path.as_str(), json(tags), source, now],
                )
                .map(|_| ())
        })
    }

    fn remove_suggested(&self, library: LibraryId, paths: &[RelPath], tags: &[TagName]) -> Result<(), IndexError> {
        let names = json(tags);
        self.write(|connection| {
            let transaction = connection.transaction()?;
            {
                // The row stays (with fewer or no tags), so a later run does not ask about the model again.
                let mut statement = transaction.prepare_cached(
                    "UPDATE tag_suggestions SET tags = (
                        SELECT json_group_array(t.value) FROM json_each(tag_suggestions.tags) t
                        WHERE tag_key(t.value) NOT IN (SELECT tag_key(value) FROM json_each(?3))
                     ) WHERE library_id = ?1 AND path = ?2",
                )?;
                for path in paths {
                    statement.execute(params![library.0, path.as_str(), names])?;
                }
            }
            transaction.commit()
        })
    }

    fn suggestion_counts(&self, library: LibraryId) -> Result<Vec<(TagName, u32)>, IndexError> {
        self.read(|connection| {
            let mut statement = connection.prepare_cached(
                "SELECT MIN(t.value), COUNT(*) FROM tag_suggestions s, json_each(s.tags) t WHERE s.library_id = ?1
                 GROUP BY tag_key(t.value) ORDER BY COUNT(*) DESC, tag_key(t.value)",
            )?;
            statement
                .query_map([library.0], |row| {
                    Ok((converted(0, TagName::new(&row.get::<_, String>(0)?))?, row.get(1)?))
                })?
                .collect()
        })
    }

    fn paths_with_suggestion(&self, library: LibraryId, tag: &TagName) -> Result<Vec<RelPath>, IndexError> {
        self.read(|connection| {
            let mut statement = connection.prepare_cached(
                "SELECT DISTINCT s.path FROM tag_suggestions s, json_each(s.tags) t
                 WHERE s.library_id = ?1 AND tag_key(t.value) = tag_key(?2)",
            )?;
            statement.query_map(params![library.0, tag.as_str()], |row| rel_path(row, 0))?.collect()
        })
    }
}
