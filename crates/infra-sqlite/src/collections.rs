use std::time::{SystemTime, UNIX_EPOCH};

use hoarddeck_application::IndexError;
use hoarddeck_application::ports::CollectionStore;
use hoarddeck_domain::{Collection, CollectionId, ModelId};
use rusqlite::{OptionalExtension, Row, params};

use crate::SqliteIndex;

impl CollectionStore for SqliteIndex {
    fn list_collections(&self) -> Result<Vec<(Collection, Option<u32>)>, IndexError> {
        self.read(|connection| {
            let mut statement = connection.prepare_cached(
                "SELECT c.id, c.name, c.query, CASE WHEN c.query IS NULL THEN COUNT(m.id) END
                 FROM collections c
                 LEFT JOIN collection_items i ON i.collection_id = c.id
                 LEFT JOIN models m ON m.library_id = i.library_id AND m.path = i.path
                 GROUP BY c.id ORDER BY c.name COLLATE NOCASE",
            )?;
            statement.query_map([], |row| Ok((collection(row)?, row.get(3)?)))?.collect()
        })
    }

    fn collection(&self, id: CollectionId) -> Result<Option<Collection>, IndexError> {
        self.read(|connection| {
            connection.query_row("SELECT id, name, query FROM collections WHERE id = ?1", [id.0], collection).optional()
        })
    }

    fn create_collection(&self, name: &str, query: Option<&str>) -> Result<Collection, IndexError> {
        let now =
            SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX));
        let id = self.write(|connection| {
            connection.execute(
                "INSERT INTO collections (name, query, created_at) VALUES (?1, ?2, ?3)",
                params![name, query, now],
            )?;
            Ok(connection.last_insert_rowid())
        })?;
        Ok(Collection { id: CollectionId(id), name: name.to_owned(), query: query.map(str::to_owned) })
    }

    fn rename_collection(&self, id: CollectionId, name: &str) -> Result<(), IndexError> {
        self.write(|connection| {
            connection.execute("UPDATE collections SET name = ?2 WHERE id = ?1", params![id.0, name]).map(|_| ())
        })
    }

    fn delete_collection(&self, id: CollectionId) -> Result<(), IndexError> {
        self.write(|connection| connection.execute("DELETE FROM collections WHERE id = ?1", [id.0]).map(|_| ()))
    }

    fn collection_members(&self, id: CollectionId) -> Result<Vec<ModelId>, IndexError> {
        self.read(|connection| {
            let mut statement = connection.prepare_cached(
                "SELECT m.id FROM collection_items i JOIN models m ON m.library_id = i.library_id AND m.path = i.path
                 WHERE i.collection_id = ?1 ORDER BY i.position",
            )?;
            statement.query_map([id.0], |row| Ok(ModelId(row.get(0)?)))?.collect()
        })
    }

    fn set_collection_members(&self, id: CollectionId, members: &[ModelId]) -> Result<(), IndexError> {
        self.write(|connection| {
            let transaction = connection.transaction()?;
            transaction.execute("DELETE FROM collection_items WHERE collection_id = ?1", [id.0])?;
            for (position, member) in members.iter().enumerate() {
                let inserted = transaction.execute(
                    "INSERT INTO collection_items (collection_id, library_id, path, position)
                     SELECT ?1, library_id, path, ?3 FROM models WHERE id = ?2",
                    params![id.0, member.0, i64::try_from(position).unwrap_or(i64::MAX)],
                )?;
                if inserted == 0 {
                    // An unknown model: roll back instead of saving a half list.
                    return Err(rusqlite::Error::QueryReturnedNoRows);
                }
            }
            transaction.commit()
        })
    }
}

fn collection(row: &Row<'_>) -> rusqlite::Result<Collection> {
    Ok(Collection { id: CollectionId(row.get(0)?), name: row.get(1)?, query: row.get(2)? })
}
