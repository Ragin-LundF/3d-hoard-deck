//! Read queries. Folder scopes are prefix ranges on the materialized path, never recursive SQL.

mod browse;
mod documents;
mod summaries;

pub(crate) use browse::{browse, child_folders};
pub(crate) use documents::{search_document, search_documents};
pub(crate) use summaries::{detail, summaries};
