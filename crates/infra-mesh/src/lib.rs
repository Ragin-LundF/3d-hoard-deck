//! Mesh and archive readers. 3MF plates are flattened to triangle soups the viewer can draw directly.

mod archive;
mod encode;
mod simplify;
mod threemf;

pub use archive::{ZipArchives, read_archive_entry};
pub use encode::encode_plates;
pub use simplify::{Simplified, simplify_stl};
pub use threemf::{MeshError, Plate, read_plates};
