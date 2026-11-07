//! Domain types and pure rules of 3D Hoard Deck. No I/O lives here.

mod catalog;
mod entry;
mod error;
mod file_kind;
mod ids;
mod library;
mod names;
mod rel_path;
mod sniff;

pub use catalog::{Catalog, Folder, Model, ModelFile, ModelKind};
pub use entry::{Entry, EntryKind};
pub use error::DomainError;
pub use file_kind::{FileFormat, FileKind};
pub use ids::{LibraryId, ModelId};
pub use library::{AccessMode, Library};
pub use names::{ParsedName, YearMonth, file_title, parse_folder_name};
pub use rel_path::RelPath;
pub use sniff::{SNIFF_LEN, Sniff, classify_by_header, classify_by_name};
