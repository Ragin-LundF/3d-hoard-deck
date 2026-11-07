use hoarddeck_domain::{LibraryId, RelPath};
use tantivy::schema::{
    FAST, Facet, FacetOptions, Field, INDEXED, IndexRecordOption, STORED, STRING, Schema, TextFieldIndexing,
    TextOptions,
};
use tantivy::tokenizer::{AsciiFoldingFilter, LowerCaser, RemoveLongFilter, SimpleTokenizer, TextAnalyzer};

/// Bump when fields or analyzers change; a mismatch wipes the folder and triggers a rebuild from SQLite.
pub(crate) const SCHEMA_VERSION: &str = "2";
pub(crate) const ANALYZER: &str = "hoard";

#[derive(Clone, Copy)]
pub(crate) struct Fields {
    pub model_id: Field,
    pub library: Field,
    pub title: Field,
    pub path: Field,
    pub folder: Field,
    pub format: Field,
}

impl Fields {
    pub(crate) fn schema() -> (Schema, Self) {
        let mut builder = Schema::builder();
        let words = TextOptions::default().set_indexing_options(
            TextFieldIndexing::default().set_tokenizer(ANALYZER).set_index_option(IndexRecordOption::WithFreqs),
        );
        let fields = Self {
            model_id: builder.add_u64_field("model_id", INDEXED | STORED | FAST),
            library: builder.add_u64_field("library", INDEXED),
            title: builder.add_text_field("title", words.clone()),
            path: builder.add_text_field("path", words),
            folder: builder.add_facet_field("folder", FacetOptions::default()),
            format: builder.add_text_field("format", STRING),
        };
        (builder.build(), fields)
    }
}

/// Splits on non-alphanumerics, lowercases and folds accents, so `Dragón_Head` matches `dragon head`.
pub(crate) fn analyzer() -> TextAnalyzer {
    TextAnalyzer::builder(SimpleTokenizer::default())
        .filter(RemoveLongFilter::limit(40))
        .filter(LowerCaser)
        .filter(AsciiFoldingFilter)
        .build()
}

/// `/<library id>/Miniatures/Wild Life`: a facet term matches the folder and everything below it.
/// Segments are NFC-normalized, so a name stored decomposed (macOS, SMB) matches one typed composed.
pub(crate) fn folder_facet(library: LibraryId, folder: &RelPath) -> Facet {
    let segments: Vec<String> = std::iter::once(library.0.to_string()).chain(folder.segments().map(nfc)).collect();
    Facet::from_path(segments)
}

/// Canonical composition: `u` + combining diaeresis becomes `ü`. Without it the tokenizer splits at the combining mark.
pub(crate) fn nfc(text: &str) -> String {
    unicode_normalization::UnicodeNormalization::nfc(text).collect()
}

pub(crate) fn library_term_value(library: LibraryId) -> u64 {
    u64::try_from(library.0).unwrap_or_default()
}
