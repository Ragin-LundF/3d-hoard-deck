use hoarddeck_application::IndexError;
use hoarddeck_application::ports::{HashCache, SizedFile};
use rusqlite::{OptionalExtension, params};

use crate::SqliteIndex;

impl HashCache for SqliteIndex {
    fn cached(&self, file: &SizedFile, full: bool) -> Result<Option<[u8; 32]>, IndexError> {
        let (library, path, size, mtime) = file;
        let column = if full { "full" } else { "quick" };
        self.read(|connection| {
            let hash: Option<Option<Vec<u8>>> = connection
                .query_row(
                    &format!(
                        "SELECT {column} FROM file_hashes WHERE library_id = ?1 AND path = ?2 AND size = ?3 AND mtime = ?4"
                    ),
                    params![library.0, path.as_str(), i64::try_from(*size).unwrap_or(i64::MAX), mtime],
                    |row| row.get(0),
                )
                .optional()?;
            Ok(hash.flatten().and_then(|bytes| bytes.try_into().ok()))
        })
    }

    fn store(&self, file: &SizedFile, full: bool, hash: [u8; 32]) -> Result<(), IndexError> {
        let (library, path, size, mtime) = file;
        let (quick, full) = if full { (None, Some(hash.to_vec())) } else { (Some(hash.to_vec()), None) };
        self.write(|connection| {
            // A changed size or time makes the other fingerprint stale, so it is dropped.
            connection
                .execute(
                    "INSERT INTO file_hashes (library_id, path, size, mtime, quick, full) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                     ON CONFLICT (library_id, path) DO UPDATE SET
                       quick = CASE WHEN size = excluded.size AND mtime = excluded.mtime
                                    THEN coalesce(excluded.quick, quick) ELSE excluded.quick END,
                       full  = CASE WHEN size = excluded.size AND mtime = excluded.mtime
                                    THEN coalesce(excluded.full, full) ELSE excluded.full END,
                       size = excluded.size, mtime = excluded.mtime",
                    params![library.0, path.as_str(), i64::try_from(*size).unwrap_or(i64::MAX), mtime, quick, full],
                )
                .map(|_| ())
        })
    }
}
