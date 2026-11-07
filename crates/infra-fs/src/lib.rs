//! File system adapters. Nothing here writes into a library.

mod index_location;
mod scanner;

pub use index_location::{app_home, resolve_index_dir};
pub use scanner::FsLibraryScanner;
