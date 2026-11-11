//! Scanning: walk a library, store raw entries, group them into folders and models.

mod auto_rescan;
mod compiled_rules;
mod detect;
mod folder_rules;
mod items;
mod library_locks;
mod presets;
mod regroup;
mod rule_editor;
mod scan_library;
mod scan_rules;
mod set_folder_role;

pub use auto_rescan::AutoRescan;
pub use compiled_rules::{CompiledRules, is_metadata_file};
pub use detect::detect;
pub(crate) use detect::model_stem;
pub use folder_rules::FolderRules;
pub use items::{Generation, KnownDir, KnownDirs, ScanItem, ScanPhase, ScanProgress};
pub use library_locks::{LibraryLock, LibraryLocks};
pub use presets::{CollectionPresets, PRESETS, Preset, PresetProposal};
use regroup::regroup;
pub use rule_editor::RuleEditor;
pub use scan_library::{ScanLibrary, ScanMode, ScanReport};
pub use scan_rules::{FolderRole, FolderRule, ScanRules};
pub use set_folder_role::SetFolderRole;
