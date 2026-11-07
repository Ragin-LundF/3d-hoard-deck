//! Domain types and pure rules of 3D Hoard Deck. No I/O lives here.

mod catalog;
mod collection;
mod entry;
mod error;
mod file_kind;
mod ids;
mod library;
mod meta;
mod names;
mod rel_path;
mod slicer;
mod sniff;

pub use catalog::{Catalog, Folder, Model, ModelFile, ModelKind};
pub use collection::{Collection, CollectionId, collection_name};
pub use entry::{Entry, EntryKind};
pub use error::DomainError;
pub use file_kind::{FileFormat, FileKind};
pub use ids::{LibraryId, ModelId};
pub use library::{AccessMode, Library, LibrarySettings, MetadataStorage};
pub use meta::{ModelMeta, TagName};
pub use names::{ParsedName, YearMonth, file_title, parse_folder_name};
pub use rel_path::RelPath;
pub use slicer::{FILES_PLACEHOLDER, Slicer, SlicerList};
pub use sniff::{SNIFF_LEN, Sniff, classify_by_header, classify_by_name};
