use std::time::{SystemTime, UNIX_EPOCH};

use hoarddeck_application::IndexError;
use hoarddeck_application::ports::{FileJournal, JournalEntry, JournalStatus};
use hoarddeck_application::scan::ScanRules;
use hoarddeck_domain::{LibraryId, ModelKind, RelPath};
use rusqlite::{OptionalExtension, Row, params};

use crate::SqliteIndex;
use crate::rows::{converted, rel_path};

const COLUMNS: &str = "id, library_id, from_path, to_path, kind, done_at, status";

/// `?2` is the old path, `?3` the new one; rows at the old path or below it follow. Prefixes are compared with
/// `substr`, because `_` and `%` in folder names would be wildcards for `LIKE`.
const MOVE_DATA: [&str; 4] = [
    "UPDATE model_meta SET path = ?3 || substr(path, length(?2) + 1)
     WHERE library_id = ?1 AND (path = ?2 OR substr(path, 1, length(?2) + 1) = ?2 || '/')",
    "UPDATE model_meta SET cover = ?3 || substr(cover, length(?2) + 1)
     WHERE library_id = ?1 AND (cover = ?2 OR substr(cover, 1, length(?2) + 1) = ?2 || '/')",
    "UPDATE collection_items SET path = ?3 || substr(path, length(?2) + 1)
     WHERE library_id = ?1 AND (path = ?2 OR substr(path, 1, length(?2) + 1) = ?2 || '/')",
    "UPDATE collections SET scope = ?3 || substr(scope, length(?2) + 1)
     WHERE library_id = ?1 AND (scope = ?2 OR substr(scope, 1, length(?2) + 1) = ?2 || '/')",
];

impl FileJournal for SqliteIndex {
    fn begin(&self, library: LibraryId, from: &RelPath, to: &RelPath, kind: ModelKind) -> Result<i64, IndexError> {
        self.write(|connection| {
            connection.execute(
                "INSERT INTO file_journal (library_id, from_path, to_path, kind, done_at, status)
                 VALUES (?1, ?2, ?3, ?4, ?5, 'pending')",
                params![library.0, from.as_str(), to.as_str(), kind.code(), now()],
            )?;
            Ok(connection.last_insert_rowid())
        })
    }

    fn set_status(&self, id: i64, status: JournalStatus) -> Result<(), IndexError> {
        self.write(|connection| {
            connection
                .execute("UPDATE file_journal SET status = ?2 WHERE id = ?1", params![id, status.code()])
                .map(|_| ())
        })
    }

    fn finish(
        &self,
        id: i64,
        library: LibraryId,
        from: &RelPath,
        to: &RelPath,
        status: JournalStatus,
    ) -> Result<(), IndexError> {
        self.write(|connection| {
            let transaction = connection.transaction()?;
            for statement in MOVE_DATA {
                transaction.execute(statement, params![library.0, from.as_str(), to.as_str()])?;
            }
            let stored: String =
                transaction
                    .query_row("SELECT scan_rules FROM libraries WHERE id = ?1", [library.0], |row| row.get(0))?;
            let rules: ScanRules = crate::rows::converted(0, serde_json::from_str(&stored))?;
            let rules = crate::rows::converted(0, serde_json::to_string(&rules.moved(from, to)))?;
            transaction.execute("UPDATE libraries SET scan_rules = ?2 WHERE id = ?1", params![library.0, rules])?;
            let undone_at = (status == JournalStatus::Undone).then(now);
            transaction.execute(
                "UPDATE file_journal SET status = ?2, undone_at = coalesce(?3, undone_at) WHERE id = ?1",
                params![id, status.code(), undone_at],
            )?;
            transaction.commit()
        })
    }

    fn entry(&self, id: i64) -> Result<Option<JournalEntry>, IndexError> {
        self.read(|connection| {
            connection.query_row(&format!("SELECT {COLUMNS} FROM file_journal WHERE id = ?1"), [id], entry).optional()
        })
    }

    fn recent(&self, limit: usize) -> Result<Vec<JournalEntry>, IndexError> {
        self.read(|connection| {
            let mut statement =
                connection.prepare_cached(&format!("SELECT {COLUMNS} FROM file_journal ORDER BY id DESC LIMIT ?1"))?;
            statement.query_map([i64::try_from(limit).unwrap_or(i64::MAX)], entry)?.collect()
        })
    }

    fn unfinished(&self) -> Result<Vec<JournalEntry>, IndexError> {
        self.read(|connection| {
            let mut statement = connection.prepare(&format!(
                "SELECT {COLUMNS} FROM file_journal WHERE status IN ('pending', 'undoing') ORDER BY id"
            ))?;
            statement.query_map([], entry)?.collect()
        })
    }
}

fn entry(row: &Row<'_>) -> rusqlite::Result<JournalEntry> {
    let status: String = row.get(6)?;
    Ok(JournalEntry {
        id: row.get(0)?,
        library: LibraryId(row.get(1)?),
        from: rel_path(row, 2)?,
        to: rel_path(row, 3)?,
        kind: converted(4, ModelKind::from_code(&row.get::<_, String>(4)?))?,
        done_at: row.get(5)?,
        status: JournalStatus::from_code(&status).ok_or_else(|| {
            rusqlite::Error::FromSqlConversionFailure(6, rusqlite::types::Type::Text, format!("status {status}").into())
        })?,
    })
}

fn now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
}
