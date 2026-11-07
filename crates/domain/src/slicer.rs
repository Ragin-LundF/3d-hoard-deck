use serde::{Deserialize, Serialize};

use crate::DomainError;

/// Placeholder in [`Slicer::args`] that is replaced by the files to open.
pub const FILES_PLACEHOLDER: &str = "{files}";

/// A program that opens model files: a built-in preset (Bambu Studio …) or one the user added.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Slicer {
    pub name: String,
    /// Executable, or an `.app` bundle on macOS.
    pub program: String,
    /// Arguments; `{files}` expands to the files. Default: just the files.
    pub args: Vec<String>,
    /// Lower-case file extensions it opens, e.g. `stl`, `3mf`, `lys`.
    pub extensions: Vec<String>,
}

impl Slicer {
    /// # Errors
    /// Returns [`DomainError::InvalidSlicer`] for an empty name, program or extension list.
    pub fn new(name: &str, program: &str, args: Vec<String>, extensions: &[String]) -> Result<Self, DomainError> {
        let extensions: Vec<String> = extensions
            .iter()
            .map(|extension| extension.trim().trim_start_matches('.').to_ascii_lowercase())
            .filter(|extension| !extension.is_empty())
            .collect();
        if name.trim().is_empty() || program.trim().is_empty() || extensions.is_empty() {
            return Err(DomainError::InvalidSlicer);
        }
        let args = if args.is_empty() { vec![FILES_PLACEHOLDER.to_owned()] } else { args };
        Ok(Self { name: name.trim().to_owned(), program: program.trim().to_owned(), args, extensions })
    }

    /// `extension` without a dot, any case.
    #[must_use]
    pub fn opens(&self, extension: &str) -> bool {
        self.extensions.iter().any(|known| known.eq_ignore_ascii_case(extension))
    }

    /// The arguments without the `{files}` placeholder (for launchers that pass files separately).
    #[must_use]
    pub fn extra_args(&self) -> Vec<String> {
        self.args.iter().filter(|arg| *arg != FILES_PLACEHOLDER).cloned().collect()
    }

    /// The argument list with `{files}` replaced; files go last when the placeholder is missing.
    #[must_use]
    pub fn arguments(&self, files: &[String]) -> Vec<String> {
        let mut result = Vec::new();
        let mut placed = false;
        for arg in &self.args {
            if arg == FILES_PLACEHOLDER {
                result.extend(files.iter().cloned());
                placed = true;
            } else {
                result.push(arg.clone());
            }
        }
        if !placed {
            result.extend(files.iter().cloned());
        }
        result
    }
}

/// The user's slicer list; the first one is the default.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlicerList {
    pub slicers: Vec<Slicer>,
}

impl SlicerList {
    /// The default slicer if it opens all these file types, else the first one that does.
    #[must_use]
    pub fn for_extensions(&self, extensions: &[&str]) -> Option<&Slicer> {
        self.slicers.iter().find(|slicer| extensions.iter().all(|extension| slicer.opens(extension)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slicer(name: &str, extensions: &[&str]) -> Slicer {
        Slicer::new(
            name,
            "/Applications/X.app",
            Vec::new(),
            &extensions.iter().map(|e| (*e).to_owned()).collect::<Vec<_>>(),
        )
        .expect("valid")
    }

    #[test]
    fn extensions_are_normalized_and_matched_case_insensitively() {
        let bambu = slicer("Bambu Studio", &[".STL", "3mf", " "]);
        assert_eq!(bambu.extensions, vec!["stl", "3mf"]);
        assert!(bambu.opens("STL") && bambu.opens("3mf") && !bambu.opens("lys"));
        assert!(Slicer::new(" ", "x", Vec::new(), &["stl".to_owned()]).is_err());
    }

    #[test]
    fn files_replace_the_placeholder_or_go_last() {
        let files = vec!["/a b.stl".to_owned(), "/c.stl".to_owned()];
        assert_eq!(slicer("S", &["stl"]).arguments(&files), files);
        let custom = Slicer::new(
            "P",
            "prusa",
            vec!["--single-instance".into(), "{files}".into(), "--x".into()],
            &["stl".into()],
        )
        .expect("valid");
        assert_eq!(custom.arguments(&files), vec!["--single-instance", "/a b.stl", "/c.stl", "--x"]);
    }

    #[test]
    fn the_first_slicer_that_opens_every_file_is_picked() {
        let list =
            SlicerList { slicers: vec![slicer("Bambu Studio", &["stl", "3mf"]), slicer("Lychee", &["stl", "lys"])] };
        assert_eq!(list.for_extensions(&["stl"]).map(|s| s.name.as_str()), Some("Bambu Studio"));
        assert_eq!(list.for_extensions(&["lys"]).map(|s| s.name.as_str()), Some("Lychee"));
        assert_eq!(list.for_extensions(&["chitubox"]), None);
    }
}
