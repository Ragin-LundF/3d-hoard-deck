//! IPC shapes. Ids are `u32` and sizes `f64` so they stay exact as JavaScript numbers.

mod browse;
mod error;
mod library;
mod model;
mod search;

pub use browse::{BrowseRequestDto, FolderPageDto, FolderRoleDto};
pub use error::{IpcError, rel_path};
pub use library::{FolderDto, LibraryDto};
pub use model::ModelDetailDto;
pub use search::{SearchRequestDto, SearchResultDto};

fn id32(id: i64) -> u32 {
    u32::try_from(id).unwrap_or(u32::MAX)
}
