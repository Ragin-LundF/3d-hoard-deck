use hoarddeck_application::IndexError;
use rusqlite::Connection;

use crate::database::db_error;

/// Ordered migrations; `PRAGMA user_version` stores how many have run.
const MIGRATIONS: [&str; 6] = [
    r"
CREATE TABLE libraries (
    id              INTEGER PRIMARY KEY,
    name            TEXT NOT NULL,
    root            TEXT NOT NULL UNIQUE,
    access_mode     TEXT NOT NULL,
    scan_rules      TEXT NOT NULL,
    scan_generation INTEGER NOT NULL DEFAULT 0,
    last_scan_at    INTEGER
);

-- Raw walk results; changing scan rules re-groups from here without walking the library again.
CREATE TABLE entries (
    library_id   INTEGER NOT NULL REFERENCES libraries(id) ON DELETE CASCADE,
    path         TEXT NOT NULL,
    parent       TEXT NOT NULL,
    is_dir       INTEGER NOT NULL,
    kind         TEXT NOT NULL,
    size         INTEGER NOT NULL,
    mtime        INTEGER NOT NULL,
    listed_mtime INTEGER,          -- folder mtime when it was last listed; NULL = never listed
    generation   INTEGER NOT NULL,
    PRIMARY KEY (library_id, path)
) WITHOUT ROWID;
CREATE INDEX entries_parent ON entries (library_id, parent);

CREATE TABLE folders (
    library_id    INTEGER NOT NULL REFERENCES libraries(id) ON DELETE CASCADE,
    path          TEXT NOT NULL,
    parent        TEXT NOT NULL,
    display_name  TEXT NOT NULL COLLATE NOCASE,
    released      TEXT,
    direct_models INTEGER NOT NULL,
    total_models  INTEGER NOT NULL,
    cover         TEXT,
    is_collection INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (library_id, path)
) WITHOUT ROWID;
CREATE INDEX folders_parent ON folders (library_id, parent, display_name);

CREATE TABLE models (
    id         INTEGER PRIMARY KEY AUTOINCREMENT, -- never reuse ids: tags and collections will point at them
    library_id INTEGER NOT NULL REFERENCES libraries(id) ON DELETE CASCADE,
    path       TEXT NOT NULL,
    folder     TEXT NOT NULL,
    kind       TEXT NOT NULL,
    title      TEXT NOT NULL COLLATE NOCASE,
    released   TEXT NOT NULL DEFAULT '',   -- YYYY-MM or '' so keyset paging needs no NULL handling
    cover      TEXT,
    collection TEXT,                       -- nearest folder marked as a collection
    formats    TEXT NOT NULL,              -- comma-separated format codes
    file_count INTEGER NOT NULL,
    total_size INTEGER NOT NULL,
    added_at   INTEGER NOT NULL,
    seen       INTEGER NOT NULL DEFAULT 1,
    UNIQUE (library_id, path)
);
CREATE INDEX models_folder_title ON models (library_id, folder, title, id);
CREATE INDEX models_title ON models (library_id, title, id);

CREATE TABLE model_files (
    model_id INTEGER NOT NULL REFERENCES models(id) ON DELETE CASCADE,
    path     TEXT NOT NULL,
    kind     TEXT NOT NULL,
    size     INTEGER NOT NULL,
    variant  TEXT
);
CREATE INDEX model_files_model ON model_files (model_id);
",
    r"
-- 2: remember which models exist because the user marked their folder, so the UI can offer to undo it.
ALTER TABLE models ADD COLUMN marked_by_user INTEGER NOT NULL DEFAULT 0;
",
    r"
-- 3: where a library keeps tags, names and notes: index_only or index_and_sidecar.
ALTER TABLE libraries ADD COLUMN metadata_storage TEXT NOT NULL DEFAULT 'index_only';
",
    r"
-- 4: app-wide settings as JSON values, e.g. the slicer list.
CREATE TABLE app_settings (key TEXT PRIMARY KEY, value TEXT NOT NULL) WITHOUT ROWID;
",
    r"
-- 5: what the user sets on a model, keyed by path so it survives regrouping.
CREATE TABLE model_meta (
    library_id   INTEGER NOT NULL REFERENCES libraries(id) ON DELETE CASCADE,
    path         TEXT NOT NULL,
    display_name TEXT,
    favorite     INTEGER NOT NULL DEFAULT 0,
    printed      INTEGER NOT NULL DEFAULT 0,
    notes        TEXT NOT NULL DEFAULT '',
    tags         TEXT NOT NULL DEFAULT '[]',  -- JSON array of tag names
    PRIMARY KEY (library_id, path)
) WITHOUT ROWID;
",
    r"
-- 6: a cover the user picked, and collections (hand-made: ordered members; smart: a saved search).
ALTER TABLE model_meta ADD COLUMN cover TEXT;
ALTER TABLE model_meta ADD COLUMN updated_at INTEGER NOT NULL DEFAULT 0;
CREATE TABLE collections (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    name       TEXT NOT NULL,
    query      TEXT,
    created_at INTEGER NOT NULL
);
CREATE TABLE collection_items (
    collection_id INTEGER NOT NULL REFERENCES collections(id) ON DELETE CASCADE,
    library_id    INTEGER NOT NULL REFERENCES libraries(id) ON DELETE CASCADE,
    path          TEXT NOT NULL,
    position      INTEGER NOT NULL,
    PRIMARY KEY (collection_id, library_id, path)
) WITHOUT ROWID;
",
];

pub(crate) fn migrate(connection: &mut Connection) -> Result<(), IndexError> {
    let version: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0)).map_err(db_error)?;
    let version = usize::try_from(version).unwrap_or(usize::MAX);
    if version > MIGRATIONS.len() {
        return Err(IndexError(format!("index schema {version} is newer than this app supports")));
    }
    let transaction = connection.transaction().map_err(db_error)?;
    for (index, migration) in MIGRATIONS.iter().enumerate().skip(version) {
        transaction.execute_batch(migration).map_err(db_error)?;
        transaction
            .pragma_update(None, "user_version", i64::try_from(index + 1).unwrap_or(i64::MAX))
            .map_err(db_error)?;
    }
    transaction.commit().map_err(db_error)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_index_from_an_older_version_is_upgraded_in_place() {
        let mut connection = Connection::open_in_memory().expect("open");
        connection.execute_batch(MIGRATIONS[0]).expect("first migration");
        connection.pragma_update(None, "user_version", 1).expect("version");
        connection
            .execute(
                "INSERT INTO libraries (name, root, access_mode, scan_rules) VALUES ('a', '/a', 'read_only', '{}')",
                [],
            )
            .expect("library");

        migrate(&mut connection).expect("upgrade");

        let version: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0)).expect("version");
        assert_eq!(usize::try_from(version).ok(), Some(MIGRATIONS.len()));
        let libraries: i64 =
            connection.query_row("SELECT COUNT(*) FROM libraries", [], |row| row.get(0)).expect("count");
        assert_eq!(libraries, 1, "existing data survives");
        connection.prepare("SELECT marked_by_user FROM models").expect("new column exists");
    }
}
