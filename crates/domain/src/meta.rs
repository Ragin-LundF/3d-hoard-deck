use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::{DomainError, RelPath};

const MAX_TAG: usize = 64;
const MAX_NAME: usize = 200;
const MAX_NOTES: usize = 20_000;

/// A tag as the user typed it, trimmed, with inner whitespace collapsed. Compared case-insensitively.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct TagName(String);

impl TagName {
    /// # Errors
    /// Returns [`DomainError::InvalidMeta`] for empty or overly long tags.
    pub fn new(text: &str) -> Result<Self, DomainError> {
        let tag = text.split_whitespace().collect::<Vec<_>>().join(" ");
        if tag.is_empty() || tag.chars().count() > MAX_TAG {
            return Err(DomainError::InvalidMeta("a tag needs 1 to 64 characters"));
        }
        Ok(Self(tag))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Lower-case form used for matching and searching.
    #[must_use]
    pub fn key(&self) -> String {
        self.0.to_lowercase()
    }
}

impl TryFrom<String> for TagName {
    type Error = DomainError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(&value)
    }
}

impl From<TagName> for String {
    fn from(value: TagName) -> Self {
        value.0
    }
}

/// What the user adds to a model: a display name, tags, favorite and printed marks, notes.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ModelMeta {
    /// Shown instead of the folder name; the name on disk stays as it is.
    pub display_name: Option<String>,
    pub tags: BTreeSet<TagName>,
    pub favorite: bool,
    pub printed: bool,
    pub notes: String,
    /// An image of the model the user picked as cover, instead of the detected one.
    pub cover: Option<RelPath>,
    /// Tags from tag rules the user does not want on this model.
    pub hidden_tags: BTreeSet<TagName>,
}

impl ModelMeta {
    /// Trims texts, drops tags that only differ in case, and checks lengths.
    ///
    /// # Errors
    /// Returns [`DomainError::InvalidMeta`] for a name or notes that are too long.
    pub fn normalized(mut self) -> Result<Self, DomainError> {
        self.display_name = self.display_name.map(|name| name.trim().to_owned()).filter(|name| !name.is_empty());
        if self.display_name.as_ref().is_some_and(|name| name.chars().count() > MAX_NAME) {
            return Err(DomainError::InvalidMeta("a name can have at most 200 characters"));
        }
        let end = self.notes.trim_end().len();
        self.notes.truncate(end);
        let start = self.notes.len() - self.notes.trim_start().len();
        self.notes.drain(..start);
        if self.notes.chars().count() > MAX_NOTES {
            return Err(DomainError::InvalidMeta("notes can have at most 20,000 characters"));
        }
        let mut seen = BTreeSet::new();
        self.tags.retain(|tag| seen.insert(tag.key()));
        let mut seen = BTreeSet::new();
        self.hidden_tags.retain(|tag| seen.insert(tag.key()));
        Ok(self)
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tags_are_tidied_and_deduplicated_case_insensitively() {
        assert_eq!(TagName::new("  Sci   Fi ").expect("valid").as_str(), "Sci Fi");
        assert!(TagName::new("   ").is_err());
        let meta = ModelMeta {
            tags: ["Dragon", "dragon", "Resin"].iter().map(|t| TagName::new(t).expect("valid")).collect(),
            display_name: Some("  ".to_owned()),
            notes: "  print at 0.05 mm ".to_owned(),
            ..ModelMeta::default()
        }
        .normalized()
        .expect("valid");
        assert_eq!(meta.tags.iter().map(TagName::as_str).collect::<Vec<_>>(), vec!["Dragon", "Resin"]);
        assert_eq!((meta.display_name, meta.notes.as_str()), (None, "print at 0.05 mm"));
    }

    #[test]
    fn overly_long_texts_are_refused() {
        let meta = ModelMeta { display_name: Some("x".repeat(201)), ..ModelMeta::default() };
        assert!(meta.normalized().is_err());
    }
}
