use hoarddeck_application::IndexError;
use hoarddeck_application::ports::LibraryRepository;
use hoarddeck_application::scan::ScanRules;
use hoarddeck_domain::{AccessMode, Library, LibraryId, LibrarySettings, MetadataStorage};
use rusqlite::{OptionalExtension, Row, params};

use crate::SqliteIndex;
use crate::rows::converted;

const COLUMNS: &str = "id, name, root, access_mode, metadata_storage";

impl LibraryRepository for SqliteIndex {
    fn add(&self, name: &str, root: &str, rules: &ScanRules) -> Result<Library, IndexError> {
        let rules = serde_json::to_string(rules).map_err(|error| IndexError(error.to_string()))?;
        let settings = LibrarySettings::default();
        let id = self.write(|connection| {
            connection.execute(
                "INSERT INTO libraries (name, root, access_mode, metadata_storage, scan_rules) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![name, root, settings.access().code(), settings.metadata().code(), rules],
            )?;
            Ok(connection.last_insert_rowid())
        })?;
        Ok(Library { id: LibraryId(id), name: name.to_owned(), root: root.to_owned(), settings })
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

    fn save_settings(&self, id: LibraryId, settings: LibrarySettings) -> Result<(), IndexError> {
        self.write(|connection| {
            connection
                .execute(
                    "UPDATE libraries SET access_mode = ?2, metadata_storage = ?3 WHERE id = ?1",
                    params![id.0, settings.access().code(), settings.metadata().code()],
                )
                .map(|_| ())
        })
    }

    fn update_scan_rules(
        &self,
        id: LibraryId,
        change: &mut dyn FnMut(&mut ScanRules) -> bool,
    ) -> Result<Option<ScanRules>, IndexError> {
        self.write(|connection| {
            let transaction = connection.transaction()?;
            let stored: String =
                transaction.query_row("SELECT scan_rules FROM libraries WHERE id = ?1", [id.0], |row| row.get(0))?;
            let mut rules: ScanRules = crate::rows::converted(0, serde_json::from_str(&stored))?;
            if !change(&mut rules) {
                return Ok(None);
            }
            let json = crate::rows::converted(0, serde_json::to_string(&rules))?;
            transaction.execute("UPDATE libraries SET scan_rules = ?2 WHERE id = ?1", params![id.0, json])?;
            transaction.commit()?;
            Ok(Some(rules))
        })
    }

    fn last_scan_at(&self, id: LibraryId) -> Result<Option<i64>, IndexError> {
        self.read(|connection| {
            connection.query_row("SELECT last_scan_at FROM libraries WHERE id = ?1", [id.0], |row| row.get(0))
        })
    }
}

fn library(row: &Row<'_>) -> rusqlite::Result<Library> {
    let access = converted(3, AccessMode::from_code(&row.get::<_, String>(3)?))?;
    let metadata = converted(4, MetadataStorage::from_code(&row.get::<_, String>(4)?))?;
    Ok(Library {
        id: LibraryId(row.get(0)?),
        name: row.get(1)?,
        root: row.get(2)?,
        settings: converted(3, LibrarySettings::new(access, metadata))?,
    })
}
