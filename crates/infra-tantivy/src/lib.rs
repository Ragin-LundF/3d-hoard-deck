//! Tantivy search adapter.

mod fields;
mod query;
mod search_folder;
mod search_index;

pub use search_folder::SearchFolder;
pub use search_index::TantivySearchIndex;
