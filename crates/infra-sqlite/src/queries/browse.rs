use std::fmt::Write;

use hoarddeck_application::ModelSummary;
use hoarddeck_application::browse::{BrowsePage, BrowseRequest, Cursor, Sort};
use hoarddeck_domain::{Folder, LibraryId, RelPath};
use rusqlite::types::Value;
use rusqlite::{Connection, params, params_from_iter};

use crate::rows::{FOLDER_COLUMNS, SUMMARY_COLUMNS, SUMMARY_FROM, TITLE, folder, subtree_bounds, summary};

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
/// Name order follows the title the user sees, display names included.
pub(crate) fn browse(connection: &Connection, request: &BrowseRequest) -> rusqlite::Result<BrowsePage> {
    let mut sql = format!("SELECT {SUMMARY_COLUMNS} FROM {SUMMARY_FROM} WHERE m.library_id = ?");
    let mut values: Vec<Value> = vec![Value::Integer(request.library.0)];
    push_scope(&mut sql, &mut values, &request.folder, request.recursive);
    if let Some(cursor) = &request.cursor {
        match request.sort {
            Sort::Name => {
                let _ = write!(sql, " AND ({TITLE} COLLATE NOCASE > ? OR ({TITLE} COLLATE NOCASE = ? AND m.id > ?))");
            }
            Sort::Released => sql.push_str(" AND (m.released < ? OR (m.released = ? AND m.id < ?))"),
            Sort::Added => sql.push_str(" AND m.id < ?"),
        }
        if request.sort != Sort::Added {
            values.push(Value::Text(cursor.key.clone()));
            values.push(Value::Text(cursor.key.clone()));
        }
        values.push(Value::Integer(cursor.id.0));
    }
    let _ = match request.sort {
        Sort::Name => write!(sql, " ORDER BY {TITLE} COLLATE NOCASE ASC, m.id ASC"),
        Sort::Added => write!(sql, " ORDER BY m.id DESC"),
        Sort::Released => write!(sql, " ORDER BY m.released DESC, m.id DESC"),
    };
    sql.push_str(" LIMIT ?");
    values.push(Value::Integer(i64::try_from(request.limit).unwrap_or(i64::MAX).saturating_add(1)));

    let mut statement = connection.prepare_cached(&sql)?;
    let mut models: Vec<ModelSummary> =
        statement.query_map(params_from_iter(values), summary)?.collect::<rusqlite::Result<_>>()?;
    let next = if models.len() > request.limit {
        models.truncate(request.limit);
        models.last().map(|last| Cursor { key: cursor_key(request.sort, last), id: last.id })
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
            sql.push_str(" AND (m.folder = ? OR (m.folder >= ? AND m.folder < ?))");
            values.extend([Value::Text(folder.to_string()), Value::Text(low), Value::Text(high)]);
        }
        (false, _) => {
            sql.push_str(" AND m.folder = ?");
            values.push(Value::Text(folder.to_string()));
        }
    }
}

fn cursor_key(sort: Sort, model: &ModelSummary) -> String {
    match sort {
        Sort::Name => model.title.clone(),
        Sort::Released => model.released.map(|r| r.to_string()).unwrap_or_default(),
        Sort::Added => String::new(),
    }
}
