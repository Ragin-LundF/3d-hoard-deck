use serde::{Deserialize, Serialize};

use crate::DomainError;

const MAX_NAME: usize = 120;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct CollectionId(pub i64);

/// A named group of models across libraries: hand-ordered, or a saved search that fills itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Collection {
    pub id: CollectionId,
    pub name: String,
    /// Search text for a smart collection; `None` for a hand-made one.
    pub query: Option<String>,
}

/// Validates a collection name.
///
/// # Errors
/// Returns [`DomainError::InvalidMeta`] for an empty or overly long name.
pub fn collection_name(name: &str) -> Result<String, DomainError> {
    let name = name.split_whitespace().collect::<Vec<_>>().join(" ");
    if name.is_empty() || name.chars().count() > MAX_NAME {
        return Err(DomainError::InvalidMeta("a collection name needs 1 to 120 characters"));
    }
    Ok(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_tidied_and_checked() {
        assert_eq!(collection_name("  Chess   set ").expect("valid"), "Chess set");
        assert!(collection_name(" ").is_err());
        assert!(collection_name(&"x".repeat(121)).is_err());
    }
}
