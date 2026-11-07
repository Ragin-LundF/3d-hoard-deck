//! File system adapters. Nothing here writes into a library.

mod hasher;
mod index_location;
mod orynt3d;
mod scanner;
mod sidecars;
mod storage;

pub use hasher::Blake3Hasher;
pub use index_location::{BootstrapFile, app_home, resolve_index_dir};
pub use orynt3d::Orynt3dFiles;
pub use scanner::FsLibraryScanner;
pub use sidecars::SidecarFileStore;
pub use storage::SystemStorage;
