//! File system adapters. Only `FsFileOperations` and `SidecarFileStore` write into a library, and only when its
//! settings allow it.

mod file_operations;
mod hasher;
mod index_location;
mod orynt3d;
mod scanner;
mod sidecars;
mod storage;
mod watcher;

pub use file_operations::FsFileOperations;
pub use hasher::Blake3Hasher;
pub use index_location::{BootstrapFile, app_home, resolve_index_dir};
pub use orynt3d::Orynt3dFiles;
pub use scanner::FsLibraryScanner;
pub use sidecars::SidecarFileStore;
pub use storage::SystemStorage;
pub use watcher::NotifyWatcher;
