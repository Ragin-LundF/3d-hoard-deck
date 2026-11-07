use std::collections::HashMap;

use hoarddeck_application::{ModelDetail, ModelSummary};
use hoarddeck_domain::{FileKind, ModelFile, ModelId};
use rusqlite::{Connection, OptionalExtension, params_from_iter};

use crate::meta::meta_row;
use crate::rows::{SUMMARY_COLUMNS, SUMMARY_FROM, converted, rel_path, summary, to_u64};

pub(crate) fn summaries(connection: &Connection, ids: &[ModelId]) -> rusqlite::Result<Vec<ModelSummary>> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let placeholders = vec!["?"; ids.len()].join(",");
    let mut statement =
        connection.prepare(&format!("SELECT {SUMMARY_COLUMNS} FROM {SUMMARY_FROM} WHERE m.id IN ({placeholders})"))?;
    let mut by_id: HashMap<ModelId, ModelSummary> = statement
        .query_map(params_from_iter(ids.iter().map(|id| id.0)), summary)?
        .map(|s| s.map(|s| (s.id, s)))
        .collect::<rusqlite::Result<_>>()?;
    Ok(ids.iter().filter_map(|id| by_id.remove(id)).collect())
}

pub(crate) fn detail(connection: &Connection, id: ModelId) -> rusqlite::Result<Option<ModelDetail>> {
    let sql = format!("SELECT {SUMMARY_COLUMNS}, m.title FROM {SUMMARY_FROM} WHERE m.id = ?1");
    let Some((summary, original_title)) =
        connection.query_row(&sql, [id.0], |row| Ok((summary(row)?, row.get::<_, String>(15)?))).optional()?
    else {
        return Ok(None);
    };
    let meta = meta_row(connection, summary.library, &summary.path)?;
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
    Ok(Some(ModelDetail { summary, files, original_title, meta }))
}
