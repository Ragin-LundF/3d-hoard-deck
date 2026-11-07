use std::fs;
use std::path::Path;
use std::sync::Mutex;

use hoarddeck_application::SearchError;
use hoarddeck_application::ports::SearchIndex;
use hoarddeck_application::search::{SearchDocument, SearchHits, SearchQuery};
use hoarddeck_domain::{LibraryId, ModelId};
use tantivy::collector::{Count, TopDocs};
use tantivy::directory::MmapDirectory;
use tantivy::schema::Value;
use tantivy::{Index, IndexReader, IndexWriter, ReloadPolicy, TantivyDocument, Term, doc};

use crate::fields::{ANALYZER, Fields, SCHEMA_VERSION, analyzer, folder_facet, library_term_value, nfc};
use crate::query;

const VERSION_FILE: &str = "hoarddeck-schema";
const WRITER_MEMORY: usize = 50_000_000;

/// Tantivy index in its own folder. Derived data: [`Self::was_reset`] tells the app to rebuild it from SQLite.
pub struct TantivySearchIndex {
    index: Index,
    fields: Fields,
    writer: Mutex<IndexWriter>,
    reader: IndexReader,
    was_reset: bool,
}

impl TantivySearchIndex {
    /// Opens or creates the index in `dir`. A folder with another schema version is wiped and recreated empty.
    ///
    /// # Errors
    /// Fails when the folder cannot be created or the index cannot be opened.
    pub fn open(dir: &Path) -> Result<Self, SearchError> {
        let version_file = dir.join(VERSION_FILE);
        let current = fs::read_to_string(&version_file).ok();
        let was_reset = current.as_deref() != Some(SCHEMA_VERSION);
        if was_reset && dir.exists() {
            fs::remove_dir_all(dir).map_err(io_error)?;
        }
        fs::create_dir_all(dir).map_err(io_error)?;
        let (schema, fields) = Fields::schema();
        let index =
            Index::open_or_create(MmapDirectory::open(dir).map_err(|error| SearchError(error.to_string()))?, schema)
                .map_err(search_error)?;
        index.tokenizers().register(ANALYZER, analyzer());
        let writer = index.writer(WRITER_MEMORY).map_err(search_error)?;
        let reader = index.reader_builder().reload_policy(ReloadPolicy::Manual).try_into().map_err(search_error)?;
        fs::write(&version_file, SCHEMA_VERSION).map_err(io_error)?;
        Ok(Self { index, fields, writer: Mutex::new(writer), reader, was_reset })
    }

    /// True when the index was created empty on open (first start or schema change) and needs a rebuild.
    #[must_use]
    pub fn was_reset(&self) -> bool {
        self.was_reset
    }
}

impl SearchIndex for TantivySearchIndex {
    fn replace_library(&self, library: LibraryId, docs: &[SearchDocument]) -> Result<(), SearchError> {
        let mut writer = self.writer.lock().map_err(|_| SearchError("writer lock poisoned".to_owned()))?;
        let result = self.write_library(&mut writer, library, docs);
        if result.is_err() {
            // Drop the half-done delete and adds; otherwise the next commit would publish them.
            writer.rollback().map_err(search_error)?;
        }
        result
    }

    fn search(&self, query: &SearchQuery) -> Result<SearchHits, SearchError> {
        let mut analyzer = self.index.tokenizers().get(ANALYZER).unwrap_or_else(analyzer);
        let tantivy_query = query::build(query, self.fields, &mut analyzer);
        let searcher = self.reader.searcher();
        let top = TopDocs::with_limit(query.limit).and_offset(query.offset).order_by_score();
        let (hits, total) = searcher.search(&tantivy_query, &(top, Count)).map_err(search_error)?;
        let ids = hits
            .into_iter()
            .map(|(_, address)| {
                let stored: TantivyDocument = searcher.doc(address).map_err(search_error)?;
                let id = stored.get_first(self.fields.model_id).and_then(|value| value.as_u64()).unwrap_or_default();
                Ok(ModelId(i64::try_from(id).unwrap_or_default()))
            })
            .collect::<Result<_, SearchError>>()?;
        Ok(SearchHits { ids, total })
    }
}

impl TantivySearchIndex {
    fn write_library(
        &self,
        writer: &mut IndexWriter,
        library: LibraryId,
        docs: &[SearchDocument],
    ) -> Result<(), SearchError> {
        let fields = self.fields;
        writer.delete_term(Term::from_field_u64(fields.library, library_term_value(library)));
        for document in docs {
            let mut tantivy_doc = doc!(
                fields.model_id => u64::try_from(document.id.0).unwrap_or_default(),
                fields.library => library_term_value(library),
                fields.title => nfc(&document.title),
                fields.path => nfc(document.path.as_str()),
                fields.folder => folder_facet(library, &document.folder),
            );
            for format in &document.formats {
                tantivy_doc.add_text(fields.format, format.code());
            }
            writer.add_document(tantivy_doc).map_err(search_error)?;
        }
        writer.commit().map_err(search_error)?;
        self.reader.reload().map_err(search_error)
    }
}

#[allow(clippy::needless_pass_by_value)] // used as `map_err(search_error)`
fn search_error(error: tantivy::TantivyError) -> SearchError {
    SearchError(error.to_string())
}

#[allow(clippy::needless_pass_by_value)]
fn io_error(error: std::io::Error) -> SearchError {
    SearchError(error.to_string())
}
