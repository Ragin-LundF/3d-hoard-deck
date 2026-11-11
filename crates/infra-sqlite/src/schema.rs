use hoarddeck_application::IndexError;
use rusqlite::Connection;

use crate::database::db_error;

/// Ordered migrations; `PRAGMA user_version` stores how many have run.
const MIGRATIONS: [&str; 19] = [
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
    query      TEXT,                -- search text of a smart collection, NULL for a hand-made one
    library_id INTEGER REFERENCES libraries(id) ON DELETE SET NULL,
    scope      TEXT,                -- folder the smart search is limited to
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
    r"
-- 7: renames and moves done on disk, for undo; file fingerprints for the duplicate finder.
CREATE TABLE file_journal (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    library_id INTEGER NOT NULL REFERENCES libraries(id) ON DELETE CASCADE,
    from_path  TEXT NOT NULL,
    to_path    TEXT NOT NULL,
    kind       TEXT NOT NULL,
    done_at    INTEGER NOT NULL,
    undone_at  INTEGER
);
CREATE TABLE file_hashes (
    library_id INTEGER NOT NULL REFERENCES libraries(id) ON DELETE CASCADE,
    path       TEXT NOT NULL,
    size       INTEGER NOT NULL,
    mtime      INTEGER NOT NULL,
    quick      BLOB,
    full       BLOB,
    PRIMARY KEY (library_id, path)
) WITHOUT ROWID;
",
    r"
-- 8: journal entries go pending → done or failed, and undoing → undone, so a crash mid-change can be settled.
ALTER TABLE file_journal ADD COLUMN status TEXT NOT NULL DEFAULT 'done';
UPDATE file_journal SET status = 'undone' WHERE undone_at IS NOT NULL;
",
    r"
-- 9: folder covers the user picked; they win over the detected ones and survive regrouping.
CREATE TABLE folder_covers (
    library_id INTEGER NOT NULL REFERENCES libraries(id) ON DELETE CASCADE,
    path       TEXT NOT NULL,
    cover      TEXT NOT NULL,
    PRIMARY KEY (library_id, path)
) WITHOUT ROWID;
",
    r"
-- 10: a fingerprint of each model row, so a regroup writes only the models that really changed.
ALTER TABLE models ADD COLUMN fingerprint INTEGER;
",
    r"
-- 11: which scan last listed a folder. A scan then removes only what is missing in folders it read, and folders
-- whose mtime did not change cost no writes for the files in them.
ALTER TABLE entries ADD COLUMN listed_gen INTEGER;
CREATE INDEX entries_listed ON entries (library_id, listed_gen);
",
    r"
-- 12: tag groups (Scale, Genre, …) and pinned tags. Tags themselves stay in model_meta; these rows only add
-- presentation, keyed by the lower-case tag.
CREATE TABLE tag_groups (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    library_id INTEGER NOT NULL REFERENCES libraries(id) ON DELETE CASCADE,
    name       TEXT NOT NULL,
    color      TEXT NOT NULL,
    position   INTEGER NOT NULL
);
CREATE TABLE tag_info (
    library_id INTEGER NOT NULL REFERENCES libraries(id) ON DELETE CASCADE,
    key        TEXT NOT NULL,
    group_id   INTEGER REFERENCES tag_groups(id) ON DELETE SET NULL,
    pinned     INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (library_id, key)
) WITHOUT ROWID;
",
    r"
-- 13: tags from tag rules (derived from the path, rewritten on every regroup), rule tags the user hid on a model,
-- and tag suggestions from an AI, kept until the user keeps or dismisses them. Both user-owned values are keyed by
-- path like model_meta, so a regroup does not lose them.
ALTER TABLE models ADD COLUMN auto_tags TEXT NOT NULL DEFAULT '[]';
ALTER TABLE model_meta ADD COLUMN hidden_tags TEXT NOT NULL DEFAULT '[]';
CREATE TABLE tag_suggestions (
    library_id INTEGER NOT NULL REFERENCES libraries(id) ON DELETE CASCADE,
    path       TEXT NOT NULL,
    tags       TEXT NOT NULL,
    source     TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    PRIMARY KEY (library_id, path)
) WITHOUT ROWID;
",
    r"
-- 14: where a model's cover comes from: its own pictures, the preview inside its 3MF, or the folder above.
ALTER TABLE models ADD COLUMN cover_source TEXT NOT NULL DEFAULT 'own';
",
    r"
-- 15: previews rendered in the app for models without any picture: 'cached' (waiting for the user), 'kept' (the
-- user's cover in a read-only library), 'skipped' (not wanted). The pictures live in the thumbnail folder.
CREATE TABLE rendered_previews (
    library_id INTEGER NOT NULL REFERENCES libraries(id) ON DELETE CASCADE,
    path       TEXT NOT NULL,
    state      TEXT NOT NULL,
    PRIMARY KEY (library_id, path)
) WITHOUT ROWID;
",
    r"
-- 16: the folder picture shown instead when a model's 3MF cover has no preview inside.
ALTER TABLE models ADD COLUMN fallback_cover TEXT;
",
    r"
-- 17: archives extracted into a new folder are journaled too (from = archive, to = folder), with the archive's size
-- and modification time, so it is only deleted while it is still the file that was extracted.
ALTER TABLE file_journal ADD COLUMN archive_size INTEGER;
ALTER TABLE file_journal ADD COLUMN archive_mtime INTEGER;
",
    r"
-- 18: the archive files of an extraction (all parts of a multi-volume RAR), so only those are ever deleted.
ALTER TABLE file_journal ADD COLUMN archive_parts TEXT;
",
    r"
-- 19: metadata files that could not be written (tried again later; the root file as path ''), and per library the id
-- its files carry and when they were last written in full.
CREATE TABLE sidecar_pending (
    library_id INTEGER NOT NULL REFERENCES libraries(id) ON DELETE CASCADE,
    path       TEXT NOT NULL,
    PRIMARY KEY (library_id, path)
) WITHOUT ROWID;
CREATE TABLE sidecar_status (
    library_id  INTEGER PRIMARY KEY REFERENCES libraries(id) ON DELETE CASCADE,
    uid         TEXT,
    exported_at INTEGER
);
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
