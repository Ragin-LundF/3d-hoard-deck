use hoarddeck_application::IndexError;
use hoarddeck_application::ports::LibraryRepository;
use hoarddeck_application::scan::ScanRules;
use hoarddeck_domain::{AccessMode, Library, LibraryId};
use rusqlite::{OptionalExtension, Row, params};

use crate::SqliteIndex;
use crate::rows::converted;

const COLUMNS: &str = "id, name, root, access_mode";

impl LibraryRepository for SqliteIndex {
    fn add(&self, name: &str, root: &str, rules: &ScanRules) -> Result<Library, IndexError> {
        let rules = serde_json::to_string(rules).map_err(|error| IndexError(error.to_string()))?;
        let access = AccessMode::default();
        let id = self.write(|connection| {
            connection.execute(
                "INSERT INTO libraries (name, root, access_mode, scan_rules) VALUES (?1, ?2, ?3, ?4)",
                params![name, root, access.code(), rules],
            )?;
            Ok(connection.last_insert_rowid())
        })?;
        Ok(Library { id: LibraryId(id), name: name.to_owned(), root: root.to_owned(), access })
    }

    fn list(&self) -> Result<Vec<Library>, IndexError> {
        self.read(|connection| {
            let mut statement = connection.prepare_cached(&format!("SELECT {COLUMNS} FROM libraries ORDER BY name"))?;
            statement.query_map([], library)?.collect()
        })
    }

    fn get(&self, id: LibraryId) -> Result<Option<Library>, IndexError> {
        self.read(|connection| {
            connection.query_row(&format!("SELECT {COLUMNS} FROM libraries WHERE id = ?1"), [id.0], library).optional()
        })
    }

    fn scan_rules(&self, id: LibraryId) -> Result<ScanRules, IndexError> {
        let json: String = self.read(|connection| {
            connection.query_row("SELECT scan_rules FROM libraries WHERE id = ?1", [id.0], |row| row.get(0))
        })?;
        serde_json::from_str(&json).map_err(|error| IndexError(format!("scan rules of library {}: {error}", id.0)))
    }

    fn save_scan_rules(&self, id: LibraryId, rules: &ScanRules) -> Result<(), IndexError> {
        let json = serde_json::to_string(rules).map_err(|error| IndexError(error.to_string()))?;
        self.write(|connection| {
            connection.execute("UPDATE libraries SET scan_rules = ?2 WHERE id = ?1", params![id.0, json]).map(|_| ())
        })
    }
}

fn library(row: &Row<'_>) -> rusqlite::Result<Library> {
    Ok(Library {
        id: LibraryId(row.get(0)?),
        name: row.get(1)?,
        root: row.get(2)?,
        access: converted(3, AccessMode::from_code(&row.get::<_, String>(3)?))?,
    })
}
