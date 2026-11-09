use hoarddeck_application::search::SearchDocument;
use hoarddeck_domain::{LibraryId, ModelId, ModelKind};
use rusqlite::{Connection, OptionalExtension, Row};

use crate::rows::{converted, formats_from_text, rel_path};

const COLUMNS: &str = "m.id, m.library_id, m.kind, m.title, mm.display_name, m.path, m.folder, m.formats, \
     COALESCE(mm.tags, '[]'), COALESCE(mm.notes, ''), COALESCE(mm.favorite, 0), COALESCE(mm.printed, 0), m.auto_tags, \
     COALESCE(mm.hidden_tags, '[]')";
const FROM: &str = "models m LEFT JOIN model_meta mm ON mm.library_id = m.library_id AND mm.path = m.path";

pub(crate) fn search_documents(connection: &Connection, library: LibraryId) -> rusqlite::Result<Vec<SearchDocument>> {
    let mut statement = connection.prepare(&format!("SELECT {COLUMNS} FROM {FROM} WHERE m.library_id = ?1"))?;
    statement.query_map([library.0], document)?.collect()
}

pub(crate) fn search_document(connection: &Connection, id: ModelId) -> rusqlite::Result<Option<SearchDocument>> {
    connection.query_row(&format!("SELECT {COLUMNS} FROM {FROM} WHERE m.id = ?1"), [id.0], document).optional()
}

fn document(row: &Row<'_>) -> rusqlite::Result<SearchDocument> {
    let original: String = row.get(3)?;
    let display: Option<String> = row.get(4)?;
    let mut tags: Vec<String> = converted(8, serde_json::from_str(&row.get::<_, String>(8)?))?;
    let auto: Vec<String> = converted(12, serde_json::from_str(&row.get::<_, String>(12)?))?;
    let hidden: Vec<String> = converted(13, serde_json::from_str(&row.get::<_, String>(13)?))?;
    // Rule tags are searched like the user's own, except those the user hid on this model.
    let hidden: Vec<String> = hidden.iter().map(|tag| tag.to_lowercase()).collect();
    tags.extend(auto.into_iter().filter(|tag| !hidden.contains(&tag.to_lowercase())));
    Ok(SearchDocument {
        id: ModelId(row.get(0)?),
        library: LibraryId(row.get(1)?),
        kind: converted(2, ModelKind::from_code(&row.get::<_, String>(2)?))?,
        // Both names find the model after the user renamed it.
        title: display.map_or_else(|| original.clone(), |display| format!("{display} {original}")),
        path: rel_path(row, 5)?,
        folder: rel_path(row, 6)?,
        formats: formats_from_text(&row.get::<_, String>(7)?),
        tags,
        notes: row.get(9)?,
        favorite: row.get(10)?,
        printed: row.get(11)?,
    })
}
