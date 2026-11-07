//! Mesh readers. 3MF plates are flattened to triangle soups the viewer can draw directly.

mod encode;
mod threemf;

pub use encode::encode_plates;
pub use threemf::{MeshError, Plate, read_plates};
