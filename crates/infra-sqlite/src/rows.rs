//! Mapping between rows and domain or view types.

use hoarddeck_application::ModelSummary;
use hoarddeck_domain::{FileFormat, Folder, LibraryId, ModelId, ModelKind, RelPath, YearMonth};
use rusqlite::Row;
use rusqlite::types::Type;

pub(crate) const SUMMARY_COLUMNS: &str = "id, library_id, path, folder, kind, title, released, cover, formats, file_count, total_size, collection, marked_by_user";

pub(crate) const FOLDER_COLUMNS: &str =
    "path, display_name, released, direct_models, total_models, cover, is_collection";

pub(crate) fn summary(row: &Row<'_>) -> rusqlite::Result<ModelSummary> {
    Ok(ModelSummary {
        id: ModelId(row.get(0)?),
        library: LibraryId(row.get(1)?),
        path: rel_path(row, 2)?,
        folder: rel_path(row, 3)?,
        kind: converted(4, ModelKind::from_code(&row.get::<_, String>(4)?))?,
        title: row.get(5)?,
        released: YearMonth::parse(&row.get::<_, String>(6)?),
        cover: optional_rel_path(row, 7)?,
        formats: formats_from_text(&row.get::<_, String>(8)?),
        file_count: row.get(9)?,
        total_size: to_u64(row.get(10)?),
        collection: optional_rel_path(row, 11)?,
        marked_by_user: row.get(12)?,
    })
}

pub(crate) fn folder(row: &Row<'_>) -> rusqlite::Result<Folder> {
    Ok(Folder {
        path: rel_path(row, 0)?,
        display_name: row.get(1)?,
        released: row.get::<_, Option<String>>(2)?.as_deref().and_then(YearMonth::parse),
        direct_models: row.get(3)?,
        total_models: row.get(4)?,
        cover: optional_rel_path(row, 5)?,
        is_collection: row.get(6)?,
    })
}

pub(crate) fn rel_path(row: &Row<'_>, column: usize) -> rusqlite::Result<RelPath> {
    converted(column, RelPath::new(row.get::<_, String>(column)?))
}

pub(crate) fn optional_rel_path(row: &Row<'_>, column: usize) -> rusqlite::Result<Option<RelPath>> {
    row.get::<_, Option<String>>(column)?.map(|path| converted(column, RelPath::new(path))).transpose()
}

pub(crate) fn formats_to_text(formats: impl IntoIterator<Item = FileFormat>) -> String {
    formats.into_iter().map(FileFormat::code).collect::<Vec<_>>().join(",")
}

/// Unknown codes are skipped so an older app can still read a newer index.
pub(crate) fn formats_from_text(text: &str) -> Vec<FileFormat> {
    text.split(',').filter_map(|code| FileFormat::from_code(code).ok()).collect()
}

pub(crate) fn converted<T, E>(column: usize, result: Result<T, E>) -> rusqlite::Result<T>
where
    E: std::error::Error + Send + Sync + 'static,
{
    result.map_err(|error| rusqlite::Error::FromSqlConversionFailure(column, Type::Text, Box::new(error)))
}

pub(crate) fn to_i64(value: u64) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

pub(crate) fn to_u64(value: i64) -> u64 {
    u64::try_from(value).unwrap_or(0)
}

/// Exclusive upper bound for "everything below `path`": `/` + 1 is `0`, so `[path/, path0)` is the subtree.
pub(crate) fn subtree_bounds(path: &RelPath) -> (String, String) {
    (format!("{path}/"), format!("{path}0"))
}
