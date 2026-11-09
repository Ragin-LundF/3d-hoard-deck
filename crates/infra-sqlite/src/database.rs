use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use hoarddeck_application::IndexError;
use rusqlite::{Connection, OpenFlags, functions::FunctionFlags};

use crate::schema;

pub(crate) const FILE_NAME: &str = "index.sqlite";
const MAX_IDLE_READERS: usize = 4;

/// The SQLite index. One writer connection; readers come from a small pool, so reads never wait for a scan.
pub struct SqliteIndex {
    path: PathBuf,
    writer: Mutex<Connection>,
    readers: Mutex<Vec<Connection>>,
}

impl SqliteIndex {
    /// Opens or creates `index.sqlite` in `dir` and migrates it.
    ///
    /// # Errors
    /// Fails when the folder or database cannot be created, or the schema is newer than this app.
    pub fn open(dir: &Path) -> Result<Self, IndexError> {
        fs::create_dir_all(dir).map_err(|error| IndexError(format!("{}: {error}", dir.display())))?;
        let path = dir.join(FILE_NAME);
        let mut writer = Connection::open(&path).map_err(db_error)?;
        configure(&writer)?;
        schema::migrate(&mut writer)?;
        Ok(Self { path, writer: Mutex::new(writer), readers: Mutex::new(Vec::new()) })
    }

    pub(crate) fn write<T>(&self, work: impl FnOnce(&mut Connection) -> rusqlite::Result<T>) -> Result<T, IndexError> {
        let mut connection = self.writer.lock().map_err(|_| IndexError("writer lock poisoned".to_owned()))?;
        work(&mut connection).map_err(db_error)
    }

    pub(crate) fn read<T>(&self, work: impl FnOnce(&Connection) -> rusqlite::Result<T>) -> Result<T, IndexError> {
        let idle = self.readers.lock().map_err(|_| IndexError("reader pool poisoned".to_owned()))?.pop();
        let connection = match idle {
            Some(connection) => connection,
            None => self.open_reader()?,
        };
        let result = work(&connection).map_err(db_error);
        if let Ok(mut readers) = self.readers.lock()
            && readers.len() < MAX_IDLE_READERS
        {
            readers.push(connection);
        }
        result
    }

    fn open_reader(&self) -> Result<Connection, IndexError> {
        let flags = OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX;
        let connection = Connection::open_with_flags(&self.path, flags).map_err(db_error)?;
        connection.pragma_update(None, "mmap_size", 268_435_456).map_err(db_error)?;
        connection.busy_timeout(std::time::Duration::from_secs(5)).map_err(db_error)?;
        register_functions(&connection)?;
        Ok(connection)
    }
}

/// `tag_key(text)`: lower case the way the app compares tags. SQLite's own `lower()` leaves letters like Ä alone, so
/// "Ärger" and "ärger" would count as two tags.
fn register_functions(connection: &Connection) -> Result<(), IndexError> {
    connection
        .create_scalar_function(
            "tag_key",
            1,
            FunctionFlags::SQLITE_UTF8 | FunctionFlags::SQLITE_DETERMINISTIC,
            |context| Ok(context.get::<String>(0)?.to_lowercase()),
        )
        .map_err(db_error)
}

fn configure(connection: &Connection) -> Result<(), IndexError> {
    connection
        .execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA foreign_keys = ON;
             PRAGMA mmap_size = 268435456;
             PRAGMA busy_timeout = 5000;",
        )
        .map_err(db_error)?;
    register_functions(connection)
}

#[allow(clippy::needless_pass_by_value)] // used as `map_err(db_error)`
pub(crate) fn db_error(error: rusqlite::Error) -> IndexError {
    IndexError(error.to_string())
}
