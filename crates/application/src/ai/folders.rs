use serde::{Deserialize, Serialize};

use hoarddeck_domain::RelPath;

/// Where the AI may suggest tags in one library: folders ticked (with everything below) minus folders unticked inside
/// them, plus how runs over them behave. Nothing is included until the user picks folders.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AiFolders {
    pub included: Vec<RelPath>,
    pub excluded: Vec<RelPath>,
    /// Ask only about models without tags of their own.
    pub only_untagged: bool,
    /// After each scan, ask about new models in these folders.
    pub after_scan: bool,
    /// Leave out models without a preview picture instead of asking with the name and folder only.
    pub skip_without_preview: bool,
}

impl AiFolders {
    /// The deepest ticked or unticked folder above `path` decides.
    #[must_use]
    pub fn wants(&self, path: &RelPath) -> bool {
        let deepest = |list: &[RelPath]| list.iter().filter(|folder| path.is_within(folder)).map(RelPath::depth).max();
        match (deepest(&self.included), deepest(&self.excluded)) {
            (Some(included), Some(excluded)) => included > excluded,
            (Some(_), None) => true,
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_deepest_ticked_or_unticked_folder_decides() {
        let path = |text: &str| RelPath::new(text).expect("valid");
        let folders = AiFolders {
            included: vec![path("Miniatures"), path("Miniatures/Loot/Fantasy/Keep")],
            excluded: vec![path("Miniatures/Loot")],
            ..AiFolders::default()
        };
        assert!(folders.wants(&path("Miniatures/Animal Den/Lion")));
        assert!(!folders.wants(&path("Miniatures/Loot/Fantasy/Orc")));
        assert!(folders.wants(&path("Miniatures/Loot/Fantasy/Keep/Knight")));
        assert!(!folders.wants(&path("Terrain/Wall")));
        assert!(!AiFolders::default().wants(&path("Miniatures/Lion")), "nothing until folders are picked");
    }
}
