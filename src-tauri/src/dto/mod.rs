//! IPC shapes. Ids are `u32` and sizes `f64` so they stay exact as JavaScript numbers.

mod ai;
mod browse;
mod collection;
mod duplicates;
mod error;
mod journal;
mod library;
mod meta;
mod model;
mod rules;
mod search;
mod slicer;
mod tag_rules;
mod tags;

pub use ai::{AiFoldersDto, AiSettingsDto, AiTestDto, AiViewDto, DecisionDto, ResolutionDto, ReviewItemDto};
pub use browse::{ArchiveEntryDto, BrowseRequestDto, FolderPageDto, FolderRoleDto};
pub use collection::CollectionDto;
pub use duplicates::DuplicateGroupDto;
pub use error::{IpcError, rel_path};
pub use journal::{FileChangeDto, JournalEntryDto, ScreenshotDto};
pub use library::{AccessModeDto, ExistingIndexDto, FolderDto, LibraryDto, MetadataStorageDto};
pub use meta::{ImportReportDto, ModelMetaDto, RebuildReportDto, TagCountDto};
pub use model::ModelDetailDto;
pub use rules::{FolderRuleDto, PresetProposalDto};
pub use search::{SearchRequestDto, SearchResultDto};
pub use slicer::SlicerDto;
pub use tag_rules::TagRuleDto;
pub use tags::{BatchResultDto, CheckItemDto, TagGroupDto, TagInfoDto};

fn id32(id: i64) -> u32 {
    u32::try_from(id).unwrap_or(u32::MAX)
}
