//! Platform adapters: launching programs and detecting installed slicers.

mod launcher;
mod slicers;

pub use launcher::SystemLauncher;
pub use slicers::InstalledSlicers;
