use std::fmt;

use serde::{Deserialize, Serialize};

use crate::DomainError;

/// A path inside a library, relative to its root, with `/` separators.
/// The root itself is the empty path. Never starts or ends with `/`, never contains `.` or `..` segments.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Default, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct RelPath(String);

impl RelPath {
    #[must_use]
    pub fn root() -> Self {
        Self(String::new())
    }

    /// # Errors
    /// Returns [`DomainError::InvalidPath`] when the path is absolute, has empty, `.` or `..` segments, or contains `\`.
    pub fn new(path: impl Into<String>) -> Result<Self, DomainError> {
        let path = path.into();
        if path.is_empty() {
            return Ok(Self::root());
        }
        let valid = !path.contains('\\')
            && path.split('/').all(|segment| !segment.is_empty() && segment != "." && segment != "..");
        if valid { Ok(Self(path)) } else { Err(DomainError::InvalidPath(path)) }
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    #[must_use]
    pub fn is_root(&self) -> bool {
        self.0.is_empty()
    }

    /// Last segment; empty for the root.
    #[must_use]
    pub fn name(&self) -> &str {
        self.0.rsplit('/').next().unwrap_or_default()
    }

    /// `None` for the root.
    #[must_use]
    pub fn parent(&self) -> Option<Self> {
        if self.is_root() {
            return None;
        }
        Some(self.0.rsplit_once('/').map_or_else(Self::root, |(parent, _)| Self(parent.to_owned())))
    }

    /// # Errors
    /// Returns [`DomainError::InvalidPath`] when `name` is not a single valid segment.
    pub fn join(&self, name: &str) -> Result<Self, DomainError> {
        if name.contains('/') {
            return Err(DomainError::InvalidPath(name.to_owned()));
        }
        if self.is_root() { Self::new(name) } else { Self::new(format!("{}/{name}", self.0)) }
    }

    #[must_use]
    pub fn depth(&self) -> usize {
        if self.is_root() { 0 } else { self.0.split('/').count() }
    }

    pub fn segments(&self) -> impl Iterator<Item = &str> {
        self.0.split('/').filter(|segment| !segment.is_empty())
    }

    /// True when `self` equals `ancestor` or lies below it.
    #[must_use]
    pub fn is_within(&self, ancestor: &Self) -> bool {
        ancestor.is_root()
            || self.0 == ancestor.0
            || (self.0.starts_with(&ancestor.0) && self.0.as_bytes().get(ancestor.0.len()) == Some(&b'/'))
    }

    /// All ancestors from the root down to the parent, excluding `self`.
    #[must_use]
    pub fn ancestors(&self) -> Vec<Self> {
        let mut result = Vec::with_capacity(self.depth());
        let mut current = self.parent();
        while let Some(path) = current {
            current = path.parent();
            result.push(path);
        }
        result.reverse();
        result
    }
}

impl fmt::Display for RelPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<String> for RelPath {
    type Error = DomainError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<RelPath> for String {
    fn from(value: RelPath) -> Self {
        value.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn path(value: &str) -> RelPath {
        RelPath::new(value).expect("valid path")
    }

    #[test]
    fn rejects_paths_that_could_escape_the_library() {
        for bad in ["/abs", "a//b", "a/../b", "./a", "a/", "a\\b"] {
            assert!(RelPath::new(bad).is_err(), "{bad} must be rejected");
        }
    }

    #[test]
    fn navigates_parents_and_names() {
        let file = path("Miniatures/Wild Life/Turtle/turtle.stl");
        assert_eq!(file.name(), "turtle.stl");
        assert_eq!(file.parent(), Some(path("Miniatures/Wild Life/Turtle")));
        assert_eq!(path("Top").parent(), Some(RelPath::root()));
        assert_eq!(RelPath::root().parent(), None);
        assert_eq!(file.depth(), 4);
        assert_eq!(
            file.ancestors(),
            vec![
                RelPath::root(),
                path("Miniatures"),
                path("Miniatures/Wild Life"),
                path("Miniatures/Wild Life/Turtle")
            ]
        );
    }

    #[test]
    fn is_within_respects_segment_boundaries() {
        assert!(path("A/B/c").is_within(&path("A/B")));
        assert!(path("A/B").is_within(&path("A/B")));
        assert!(!path("A/BC").is_within(&path("A/B")));
        assert!(path("A").is_within(&RelPath::root()));
    }
}
