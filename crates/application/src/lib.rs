//! Use cases and ports of 3D Hoard Deck.

pub mod ai;
pub mod browse;
mod error;
pub mod files;
pub mod library;
pub mod organize;
pub mod ports;
pub mod scan;
pub mod search;
pub mod sidecar;
pub mod slicer;
pub mod tagging;
mod views;

pub use error::{AppError, IndexError, Refusal, ScanError, SearchError};
pub use views::{ModelDetail, ModelSummary};
