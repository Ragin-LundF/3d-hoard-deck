//! Read queries. Folder scopes are prefix ranges on the materialized path, never recursive SQL.

use std::collections::HashMap;

use hoarddeck_application::browse::{BrowsePage, BrowseRequest, Cursor, Sort};
use hoarddeck_application::search::SearchDocument;
use hoarddeck_application::{ModelDetail, ModelSummary};
use hoarddeck_domain::{FileKind, Folder, LibraryId, ModelFile, ModelId, RelPath};
use rusqlite::types::Value;
use rusqlite::{Connection, OptionalExtension, params, params_from_iter};

use crate::rows::{
    FOLDER_COLUMNS, SUMMARY_COLUMNS, converted, folder, formats_from_text, rel_path, subtree_bounds, summary, to_u64,
};

pub(crate) fn search_documents(connection: &Connection, library: LibraryId) -> rusqlite::Result<Vec<SearchDocument>> {
    let mut statement =
        connection.prepare("SELECT id, title, path, folder, formats FROM models WHERE library_id = ?1")?;
    statement
        .query_map([library.0], |row| {
            Ok(SearchDocument {
                id: ModelId(row.get(0)?),
                library,
                title: row.get(1)?,
                path: rel_path(row, 2)?,
                folder: rel_path(row, 3)?,
                formats: formats_from_text(&row.get::<_, String>(4)?),
            })
        })?
        .collect()
}

pub(crate) fn summaries(connection: &Connection, ids: &[ModelId]) -> rusqlite::Result<Vec<ModelSummary>> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let placeholders = vec!["?"; ids.len()].join(",");
    let mut statement =
        connection.prepare(&format!("SELECT {SUMMARY_COLUMNS} FROM models WHERE id IN ({placeholders})"))?;
    let mut by_id: HashMap<ModelId, ModelSummary> = statement
        .query_map(params_from_iter(ids.iter().map(|id| id.0)), summary)?
        .map(|s| s.map(|s| (s.id, s)))
        .collect::<rusqlite::Result<_>>()?;
    Ok(ids.iter().filter_map(|id| by_id.remove(id)).collect())
}

pub(crate) fn detail(connection: &Connection, id: ModelId) -> rusqlite::Result<Option<ModelDetail>> {
    let Some(summary) = connection
        .query_row(&format!("SELECT {SUMMARY_COLUMNS} FROM models WHERE id = ?1"), [id.0], summary)
        .optional()?
    else {
        return Ok(None);
    };
    let mut statement = connection
        .prepare_cached("SELECT path, kind, size, variant FROM model_files WHERE model_id = ?1 ORDER BY path")?;
    let files = statement
        .query_map([id.0], |row| {
            Ok(ModelFile {
                path: rel_path(row, 0)?,
                kind: converted(1, FileKind::from_code(&row.get::<_, String>(1)?))?,
                size: to_u64(row.get(2)?),
                variant: row.get(3)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    Ok(Some(ModelDetail { summary, files }))
}

pub(crate) fn child_folders(
    connection: &Connection,
    library: LibraryId,
    parent: &RelPath,
) -> rusqlite::Result<Vec<Folder>> {
    let mut statement = connection.prepare_cached(&format!(
        "SELECT {FOLDER_COLUMNS} FROM folders WHERE library_id = ?1 AND parent = ?2 ORDER BY display_name, path"
    ))?;
    statement.query_map(params![library.0, parent.as_str()], folder)?.collect()
}

/// One keyset page. Fetches one row more than asked to know whether a next page exists.
pub(crate) fn browse(connection: &Connection, request: &BrowseRequest) -> rusqlite::Result<BrowsePage> {
    let mut sql = format!("SELECT {SUMMARY_COLUMNS} FROM models WHERE library_id = ?");
    let mut values: Vec<Value> = vec![Value::Integer(request.library.0)];
    push_scope(&mut sql, &mut values, &request.folder, request.recursive);
    let (key_column, order) = match request.sort {
        Sort::Name => ("title", "title ASC, id ASC"),
        Sort::Added => ("id", "id DESC"),
        Sort::Released => ("released", "released DESC, id DESC"),
    };
    if let Some(cursor) = &request.cursor {
        match request.sort {
            Sort::Name => sql.push_str(" AND (title > ? OR (title = ? AND id > ?))"),
            Sort::Released => sql.push_str(" AND (released < ? OR (released = ? AND id < ?))"),
            Sort::Added => sql.push_str(" AND id < ?"),
        }
        if request.sort != Sort::Added {
            values.push(Value::Text(cursor.key.clone()));
            values.push(Value::Text(cursor.key.clone()));
        }
        values.push(Value::Integer(cursor.id.0));
    }
    sql.push_str(" ORDER BY ");
    sql.push_str(order);
    sql.push_str(" LIMIT ?");
    values.push(Value::Integer(i64::try_from(request.limit).unwrap_or(i64::MAX).saturating_add(1)));

    let mut statement = connection.prepare_cached(&sql)?;
    let mut models: Vec<ModelSummary> =
        statement.query_map(params_from_iter(values), summary)?.collect::<rusqlite::Result<_>>()?;
    let next = if models.len() > request.limit {
        models.truncate(request.limit);
        models.last().map(|last| Cursor { key: cursor_key(key_column, last), id: last.id })
    } else {
        None
    };
    Ok(BrowsePage { models, next })
}

fn push_scope(sql: &mut String, values: &mut Vec<Value>, folder: &RelPath, recursive: bool) {
    match (recursive, folder.is_root()) {
        (true, true) => {}
        (true, false) => {
            let (low, high) = subtree_bounds(folder);
            sql.push_str(" AND (folder = ? OR (folder >= ? AND folder < ?))");
            values.extend([Value::Text(folder.to_string()), Value::Text(low), Value::Text(high)]);
        }
        (false, _) => {
            sql.push_str(" AND folder = ?");
            values.push(Value::Text(folder.to_string()));
        }
    }
}

fn cursor_key(column: &str, model: &ModelSummary) -> String {
    match column {
        "title" => model.title.clone(),
        "released" => model.released.map(|r| r.to_string()).unwrap_or_default(),
        _ => String::new(),
    }
}
