//! Collection presets for common pack layouts. Data, not code: a new layout is a new entry.

use std::sync::Arc;

use globset::GlobBuilder;
use hoarddeck_domain::{LibraryId, parse_folder_name};

use super::{FolderRole, FolderRule, RuleEditor};
use crate::AppError;
use crate::ports::EntryStore;

pub struct Preset {
    pub id: &'static str,
    pub name: &'static str,
    /// Folders matching this glob (relative to the library root) become one collection each.
    pub pattern: &'static str,
}

pub const PRESETS: [Preset; 5] = [
    Preset { id: "loot-studios", name: "Loot Studios releases", pattern: "**/Loot Studios/*/*" },
    Preset { id: "stlflix", name: "STLFlix releases", pattern: "**/STLFlix/*" },
    Preset { id: "3d-druck-packs", name: "3D-Druck pack items", pattern: "**/STL Packs/3D-Druck Pack (*)/*" },
    Preset { id: "3d-stl-packs", name: "3D STL pack items", pattern: "**/STL Packs/3D STL Pack */*" },
    Preset {
        id: "creator-releases",
        name: "Monthly creator releases",
        pattern: "**/Miniatures/*/[0-9][0-9][0-9][0-9]-[0-9][0-9]*",
    },
];

const SAMPLES: usize = 5;

/// A preset that matches folders in this library and is not in use yet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PresetProposal {
    pub id: String,
    pub name: String,
    pub pattern: String,
    pub matches: usize,
    /// Up to five display names of matching folders.
    pub samples: Vec<String>,
}

/// Proposes presets after a scan; nothing changes until the user accepts one.
pub struct CollectionPresets {
    pub entries: Arc<dyn EntryStore>,
    pub editor: Arc<RuleEditor>,
}

impl CollectionPresets {
    /// # Errors
    /// Fails when the index cannot be read.
    pub fn propose(&self, library: LibraryId) -> Result<Vec<PresetProposal>, AppError> {
        let in_use = self.editor.rules(library)?.folder_rules;
        let folders = self.entries.folders(library)?;
        let mut proposals = Vec::new();
        for preset in PRESETS.iter().filter(|preset| !in_use.iter().any(|rule| rule.pattern == preset.pattern)) {
            let glob = GlobBuilder::new(preset.pattern)
                .case_insensitive(true)
                .literal_separator(true)
                .build()
                .map_err(|error| AppError::InvalidInput(error.to_string()))?
                .compile_matcher();
            let matching: Vec<_> = folders.iter().filter(|folder| glob.is_match(folder.as_str())).collect();
            if matching.is_empty() {
                continue;
            }
            proposals.push(PresetProposal {
                id: preset.id.to_owned(),
                name: preset.name.to_owned(),
                pattern: preset.pattern.to_owned(),
                matches: matching.len(),
                samples: matching.iter().take(SAMPLES).map(|folder| parse_folder_name(folder.name()).display).collect(),
            });
        }
        Ok(proposals)
    }

    /// Turns the preset into an ordinary collection rule and regroups.
    /// # Errors
    /// Fails for an unknown preset, a busy library, or when the index cannot be written.
    pub fn accept(&self, library: LibraryId, id: &str) -> Result<(), AppError> {
        let preset = PRESETS
            .iter()
            .find(|preset| preset.id == id)
            .ok_or_else(|| AppError::InvalidInput(format!("unknown preset {id}")))?;
        self.editor.edit(library, |rules| {
            if !rules.folder_rules.iter().any(|rule| rule.pattern == preset.pattern) {
                rules
                    .folder_rules
                    .push(FolderRule { pattern: preset.pattern.to_owned(), role: FolderRole::Collection });
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scan::{CompiledRules, ScanRules};
    use hoarddeck_domain::RelPath;

    #[test]
    fn preset_patterns_are_valid_and_match_the_documented_layouts() {
        let cases = [
            ("loot-studios", "Minis/Loot Studios/Fantasy/2023-01 - Dragons", true),
            ("loot-studios", "Minis/Loot Studios/Fantasy", false),
            ("stlflix", "STLFlix/Gothic Cathedral", true),
            ("3d-druck-packs", "STL Packs/3D-Druck Pack (12)/Castle", true),
            ("3d-stl-packs", "STL Packs/3D STL Pack Vol 3/Robot", true),
            ("creator-releases", "Miniatures/Archvillain/2022-09 - September Release", true),
            ("creator-releases", "Miniatures/Archvillain/Busts", false),
        ];
        for (id, folder, expected) in cases {
            let preset = PRESETS.iter().find(|preset| preset.id == id).expect("known preset");
            let rules = ScanRules {
                folder_rules: vec![FolderRule { pattern: preset.pattern.to_owned(), role: FolderRole::Collection }],
                ..ScanRules::default()
            };
            let compiled = CompiledRules::compile(&rules).expect("valid glob");
            assert_eq!(compiled.is_collection(&RelPath::new(folder).expect("valid")), expected, "{id} on {folder}");
        }
    }
}
