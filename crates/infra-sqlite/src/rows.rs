//! Mapping between rows and domain or view types.

use hoarddeck_application::ModelSummary;
use hoarddeck_domain::{CoverSource, FileFormat, Folder, LibraryId, ModelId, ModelKind, RelPath, YearMonth};
use rusqlite::Row;
use rusqlite::types::Type;

/// Models joined with what the user set on them; `m` and `mm` are the aliases used in every model query.
pub(crate) const SUMMARY_FROM: &str =
    "models m LEFT JOIN model_meta mm ON mm.library_id = m.library_id AND mm.path = m.path";

/// The title the user sees: their display name, else the derived one.
pub(crate) const TITLE: &str = "COALESCE(mm.display_name, m.title)";

pub(crate) const SUMMARY_COLUMNS: &str = "m.id, m.library_id, m.path, m.folder, m.kind, COALESCE(mm.display_name, m.title), \
     m.released, COALESCE((SELECT f.path FROM model_files f WHERE f.model_id = m.id AND f.path = mm.cover), m.cover), m.formats, m.file_count, m.total_size, m.collection, m.marked_by_user, \
     COALESCE(mm.favorite, 0), COALESCE(mm.printed, 0), \
     CASE WHEN mm.cover IS NOT NULL AND EXISTS (SELECT 1 FROM model_files f WHERE f.model_id = m.id AND f.path = mm.cover) \
          THEN 'own' ELSE m.cover_source END, \
     (SELECT r.state FROM rendered_previews r WHERE r.library_id = m.library_id AND r.path = m.path), \
     m.fallback_cover";

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
        favorite: row.get(13)?,
        printed: row.get(14)?,
        cover_source: CoverSource::from_code(&row.get::<_, String>(15)?),
        rendered: row.get(16)?,
        fallback_cover: optional_rel_path(row, 17)?,
    })
}

/// Columns: path, display name, released, direct and total models, cover, collection flag.
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
