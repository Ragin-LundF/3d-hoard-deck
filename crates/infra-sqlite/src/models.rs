use std::time::{SystemTime, UNIX_EPOCH};

use hoarddeck_application::browse::{BrowsePage, BrowseRequest};
use hoarddeck_application::ports::ModelRepository;
use hoarddeck_application::search::SearchDocument;
use hoarddeck_application::{IndexError, ModelDetail, ModelSummary};
use hoarddeck_domain::{Catalog, Folder, LibraryId, Model, ModelId, RelPath};
use rusqlite::{OptionalExtension, Transaction, params};

use crate::SqliteIndex;
use crate::rows::{formats_to_text, rel_path, to_i64};

impl ModelRepository for SqliteIndex {
    fn replace_catalog(&self, library: LibraryId, catalog: &Catalog) -> Result<(), IndexError> {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| to_i64(d.as_secs()));
        self.write(|connection| {
            let transaction = connection.transaction()?;
            transaction.execute(
                "DELETE FROM model_files WHERE model_id IN (SELECT id FROM models WHERE library_id = ?1)",
                [library.0],
            )?;
            transaction.execute("UPDATE models SET seen = 0 WHERE library_id = ?1", [library.0])?;
            for model in &catalog.models {
                upsert_model(&transaction, library, model, now)?;
            }
            transaction.execute("DELETE FROM models WHERE library_id = ?1 AND seen = 0", [library.0])?;
            transaction.execute("DELETE FROM folders WHERE library_id = ?1", [library.0])?;
            for folder in &catalog.folders {
                insert_folder(&transaction, library, folder)?;
            }
            transaction.commit()
        })
    }

    fn search_documents(&self, library: LibraryId) -> Result<Vec<SearchDocument>, IndexError> {
        self.read(|connection| crate::queries::search_documents(connection, library))
    }

    fn search_document(&self, id: ModelId) -> Result<Option<SearchDocument>, IndexError> {
        self.read(|connection| crate::queries::search_document(connection, id))
    }

    fn summaries(&self, ids: &[ModelId]) -> Result<Vec<ModelSummary>, IndexError> {
        self.read(|connection| crate::queries::summaries(connection, ids))
    }

    fn get(&self, id: ModelId) -> Result<Option<ModelDetail>, IndexError> {
        self.read(|connection| crate::queries::detail(connection, id))
    }

    fn browse(&self, request: &BrowseRequest) -> Result<BrowsePage, IndexError> {
        self.read(|connection| crate::queries::browse(connection, request))
    }

    fn model_of_file(&self, library: LibraryId, file: &RelPath) -> Result<Option<ModelId>, IndexError> {
        self.read(|connection| {
            connection
                .query_row(
                    "SELECT m.id FROM model_files f JOIN models m ON m.id = f.model_id WHERE m.library_id = ?1 AND f.path = ?2",
                    params![library.0, file.as_str()],
                    |row| Ok(ModelId(row.get(0)?)),
                )
                .optional()
        })
    }

    fn child_folders(&self, library: LibraryId, folder: &RelPath) -> Result<Vec<Folder>, IndexError> {
        self.read(|connection| crate::queries::child_folders(connection, library, folder))
    }

    fn loose_images(&self, library: LibraryId, folder: &RelPath) -> Result<Vec<RelPath>, IndexError> {
        self.read(|connection| {
            let mut statement = connection.prepare_cached(
                "SELECT e.path FROM entries e INDEXED BY entries_parent
                 WHERE e.library_id = ?1 AND e.parent = ?2 AND e.is_dir = 0 AND e.kind = 'image'
                   AND NOT EXISTS (SELECT 1 FROM model_files mf JOIN models m ON m.id = mf.model_id
                                   WHERE m.library_id = ?1 AND mf.path = e.path)
                   AND NOT EXISTS (SELECT 1 FROM models m WHERE m.library_id = ?1 AND m.cover = e.path)
                 ORDER BY e.path",
            )?;
            statement.query_map(params![library.0, folder.as_str()], |row| rel_path(row, 0))?.collect()
        })
    }
}

/// Keeps the id (and `added_at`) of a model whose path is unchanged, so later tags and collections stay attached.
fn upsert_model(transaction: &Transaction<'_>, library: LibraryId, model: &Model, now: i64) -> rusqlite::Result<()> {
    let id: i64 = transaction
        .prepare_cached(
            "INSERT INTO models (library_id, path, folder, kind, title, released, cover, formats, file_count, total_size, added_at, collection, marked_by_user, seen)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, 1)
             ON CONFLICT (library_id, path) DO UPDATE SET
                folder = ?3, kind = ?4, title = ?5, released = ?6, cover = ?7, formats = ?8,
                file_count = ?9, total_size = ?10, collection = ?12, marked_by_user = ?13, seen = 1
             RETURNING id",
        )?
        .query_row(
            params![
                library.0,
                model.path.as_str(),
                model.folder.as_str(),
                model.kind.code(),
                model.title,
                model.released.map(|r| r.to_string()).unwrap_or_default(),
                model.cover.as_ref().map(RelPath::as_str),
                formats_to_text(model.formats()),
                to_i64(model.files.len() as u64),
                to_i64(model.total_size()),
                now,
                model.collection.as_ref().map(RelPath::as_str),
                model.marked_by_user,
            ],
            |row| row.get(0),
        )?;
    let mut insert = transaction
        .prepare_cached("INSERT INTO model_files (model_id, path, kind, size, variant) VALUES (?1, ?2, ?3, ?4, ?5)")?;
    for file in &model.files {
        insert.execute(params![id, file.path.as_str(), file.kind.code(), to_i64(file.size), file.variant])?;
    }
    Ok(())
}

fn insert_folder(transaction: &Transaction<'_>, library: LibraryId, folder: &Folder) -> rusqlite::Result<()> {
    transaction
        .prepare_cached(
            "INSERT INTO folders (library_id, path, parent, display_name, released, direct_models, total_models, cover, is_collection)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        )?
        .execute(params![
            library.0,
            folder.path.as_str(),
            folder.path.parent().map(String::from).unwrap_or_default(),
            folder.display_name,
            folder.released.map(|r| r.to_string()),
            folder.direct_models,
            folder.total_models,
            folder.cover.as_ref().map(RelPath::as_str),
            folder.is_collection,
        ])?;
    Ok(())
}
