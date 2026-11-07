use std::time::{SystemTime, UNIX_EPOCH};

use hoarddeck_application::IndexError;
use hoarddeck_application::ports::CollectionStore;
use hoarddeck_domain::{Collection, CollectionId, LibraryId, ModelId, RelPath, SmartSearch};
use rusqlite::{OptionalExtension, Row, Transaction, params};

use crate::SqliteIndex;
use crate::rows::optional_rel_path;

const COLUMNS: &str = "c.id, c.name, c.query, c.library_id, c.scope";

impl CollectionStore for SqliteIndex {
    fn list_collections(&self) -> Result<Vec<(Collection, Option<u32>)>, IndexError> {
        self.read(|connection| {
            let mut statement = connection.prepare_cached(&format!(
                "SELECT {COLUMNS}, CASE WHEN c.query IS NULL THEN COUNT(m.id) END
                 FROM collections c
                 LEFT JOIN collection_items i ON i.collection_id = c.id
                 LEFT JOIN models m ON m.library_id = i.library_id AND m.path = i.path
                 GROUP BY c.id ORDER BY c.name COLLATE NOCASE"
            ))?;
            statement.query_map([], |row| Ok((collection(row)?, row.get(5)?)))?.collect()
        })
    }

    fn collection(&self, id: CollectionId) -> Result<Option<Collection>, IndexError> {
        self.read(|connection| {
            connection
                .query_row(&format!("SELECT {COLUMNS} FROM collections c WHERE c.id = ?1"), [id.0], collection)
                .optional()
        })
    }

    fn create_collection(&self, name: &str, smart: Option<&SmartSearch>) -> Result<Collection, IndexError> {
        let now =
            SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX));
        let id = self.write(|connection| {
            connection.execute(
                "INSERT INTO collections (name, query, library_id, scope, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    name,
                    smart.map(|search| search.text.as_str()),
                    smart.and_then(|search| search.library).map(|library| library.0),
                    smart.and_then(|search| search.scope.as_ref()).map(RelPath::as_str),
                    now
                ],
            )?;
            Ok(connection.last_insert_rowid())
        })?;
        Ok(Collection { id: CollectionId(id), name: name.to_owned(), smart: smart.cloned() })
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

    fn add_members(&self, id: CollectionId, models: &[ModelId]) -> Result<(), IndexError> {
        self.write(|connection| {
            let transaction = connection.transaction()?;
            for model in models {
                let inserted = transaction.execute(
                    "INSERT OR IGNORE INTO collection_items (collection_id, library_id, path, position)
                     SELECT ?1, library_id, path,
                            (SELECT COALESCE(MAX(position), -1) + 1 FROM collection_items WHERE collection_id = ?1)
                     FROM models WHERE id = ?2",
                    params![id.0, model.0],
                )?;
                let known: bool =
                    transaction
                        .query_row("SELECT EXISTS (SELECT 1 FROM models WHERE id = ?1)", [model.0], |row| row.get(0))?;
                if inserted == 0 && !known {
                    // An unknown model: roll back the whole call.
                    return Err(rusqlite::Error::QueryReturnedNoRows);
                }
            }
            transaction.commit()
        })
    }

    fn remove_members(&self, id: CollectionId, models: &[ModelId]) -> Result<(), IndexError> {
        self.write(|connection| {
            let transaction = connection.transaction()?;
            for model in models {
                transaction.execute(
                    "DELETE FROM collection_items WHERE collection_id = ?1
                     AND (library_id, path) = (SELECT library_id, path FROM models WHERE id = ?2)",
                    params![id.0, model.0],
                )?;
            }
            transaction.commit()
        })
    }

    fn move_member(&self, id: CollectionId, model: ModelId, index: usize) -> Result<(), IndexError> {
        self.write(|connection| {
            let transaction = connection.transaction()?;
            reorder(&transaction, id, model, index)?;
            transaction.commit()
        })
    }
}

/// Rewrites positions with `model` placed before the shown member at `index`; missing members keep their slots.
fn reorder(transaction: &Transaction<'_>, id: CollectionId, model: ModelId, index: usize) -> rusqlite::Result<()> {
    let mut statement = transaction.prepare(
        "SELECT i.library_id, i.path, m.id FROM collection_items i
         LEFT JOIN models m ON m.library_id = i.library_id AND m.path = i.path
         WHERE i.collection_id = ?1 ORDER BY i.position",
    )?;
    let mut rows: Vec<(LibraryId, String, Option<ModelId>)> = statement
        .query_map([id.0], |row| Ok((LibraryId(row.get(0)?), row.get(1)?, row.get::<_, Option<i64>>(2)?.map(ModelId))))?
        .collect::<rusqlite::Result<_>>()?;
    let from = rows.iter().position(|row| row.2 == Some(model)).ok_or(rusqlite::Error::QueryReturnedNoRows)?;
    let moved = rows.remove(from);
    let shown: Vec<usize> =
        rows.iter().enumerate().filter(|(_, row)| row.2.is_some()).map(|(position, _)| position).collect();
    let target = shown.get(index).copied().unwrap_or(rows.len());
    rows.insert(target, moved);
    for (position, (library, path, _)) in rows.iter().enumerate() {
        transaction.execute(
            "UPDATE collection_items SET position = ?4 WHERE collection_id = ?1 AND library_id = ?2 AND path = ?3",
            params![id.0, library.0, path, i64::try_from(position).unwrap_or(i64::MAX)],
        )?;
    }
    Ok(())
}

fn collection(row: &Row<'_>) -> rusqlite::Result<Collection> {
    let query: Option<String> = row.get(2)?;
    let library: Option<i64> = row.get(3)?;
    let scope = optional_rel_path(row, 4)?;
    Ok(Collection {
        id: CollectionId(row.get(0)?),
        name: row.get(1)?,
        smart: query.map(|text| SmartSearch { text, library: library.map(LibraryId), scope }),
    })
}
