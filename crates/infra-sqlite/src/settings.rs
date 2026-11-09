use hoarddeck_application::IndexError;
use hoarddeck_application::ports::{SlicerStore, TagRuleStore};
use hoarddeck_application::tagging::TagRule;
use hoarddeck_domain::SlicerList;
use rusqlite::{OptionalExtension, params};

use crate::SqliteIndex;

const SLICERS: &str = "slicers";

impl SlicerStore for SqliteIndex {
    fn load_slicers(&self) -> Result<Option<SlicerList>, IndexError> {
        let json: Option<String> = self.read(|connection| {
            connection
                .query_row("SELECT value FROM app_settings WHERE key = ?1", [SLICERS], |row| row.get(0))
                .optional()
        })?;
        json.map(|json| serde_json::from_str(&json).map_err(|error| IndexError(format!("slicer list: {error}"))))
            .transpose()
    }

    fn save_slicers(&self, list: &SlicerList) -> Result<(), IndexError> {
        let json = serde_json::to_string(list).map_err(|error| IndexError(error.to_string()))?;
        self.write(|connection| {
            connection
                .execute(
                    "INSERT INTO app_settings (key, value) VALUES (?1, ?2) ON CONFLICT (key) DO UPDATE SET value = ?2",
                    params![SLICERS, json],
                )
                .map(|_| ())
        })
    }
}

const TAG_RULES: &str = "tag_rules";

impl TagRuleStore for SqliteIndex {
    fn load_tag_rules(&self) -> Result<Vec<TagRule>, IndexError> {
        let json: Option<String> = self.read(|connection| {
            connection
                .query_row("SELECT value FROM app_settings WHERE key = ?1", [TAG_RULES], |row| row.get(0))
                .optional()
        })?;
        json.map_or_else(
            || Ok(Vec::new()),
            |json| serde_json::from_str(&json).map_err(|error| IndexError(format!("tag rules: {error}"))),
        )
    }

    fn save_tag_rules(&self, rules: &[TagRule]) -> Result<(), IndexError> {
        let json = serde_json::to_string(rules).map_err(|error| IndexError(error.to_string()))?;
        self.write(|connection| {
            connection
                .execute(
                    "INSERT INTO app_settings (key, value) VALUES (?1, ?2) ON CONFLICT (key) DO UPDATE SET value = ?2",
                    params![TAG_RULES, json],
                )
                .map(|_| ())
        })
    }
}
