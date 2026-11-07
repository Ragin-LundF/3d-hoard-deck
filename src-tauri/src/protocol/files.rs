//! Which library files may be served, and how to find them safely on disk.

use std::fs;
use std::path::{Path, PathBuf};

use hoarddeck_app::App;
use hoarddeck_domain::{LibraryId, RelPath};

/// Full-size images (gallery, model page) up to this size.
const MAX_IMAGE_BYTES: u64 = 40 * 1024 * 1024;
/// shortcut: meshes go to the viewer unchanged up to this size; larger ones need a simplified preview mesh from Rust.
const MAX_MESH_BYTES: u64 = 256 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Kind {
    Image,
    Stl,
    ThreeMf,
    Obj,
}

impl Kind {
    /// Images for covers and galleries; STL (also disguised as `.stl.xxx` / `.stl.txt`), 3MF and OBJ for the viewer.
    fn of(name: &str) -> Option<Self> {
        let lower = name.to_ascii_lowercase();
        let extension = Path::new(&lower).extension()?.to_str()?;
        match extension {
            "jpg" | "jpeg" | "png" | "webp" | "gif" | "bmp" => Some(Self::Image),
            "stl" => Some(Self::Stl),
            "3mf" => Some(Self::ThreeMf),
            "obj" => Some(Self::Obj),
            _ if lower.contains(".stl.") => Some(Self::Stl),
            _ => None,
        }
    }

    pub(super) fn max_bytes(self) -> u64 {
        if self == Self::Image { MAX_IMAGE_BYTES } else { MAX_MESH_BYTES }
    }
}

/// MIME type by extension; meshes are plain bytes for the viewer.
pub(super) fn content_type(path: &Path) -> &'static str {
    let extension = path.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase).unwrap_or_default();
    match extension.as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "webp" => "image/webp",
        "gif" => "image/gif",
        "bmp" => "image/bmp",
        _ => "application/octet-stream",
    }
}

pub(super) struct LibraryFile {
    pub path: PathBuf,
    pub kind: Kind,
}

/// `<library id>/<relative path>` to a file inside that library's root. Rejects other file types, traversal and
/// symlinks that point outside the root.
pub(super) fn resolve(app: &App, decoded: &str) -> Option<LibraryFile> {
    let (library, relative) = decoded.split_once('/')?;
    let relative = RelPath::new(relative).ok().filter(|path| !path.is_root())?;
    let kind = Kind::of(relative.name())?;
    let library = app.libraries.get(LibraryId(library.parse().ok()?)).ok()??;
    let root = fs::canonicalize(&library.root).ok()?;
    let mut file = root.clone();
    file.extend(relative.segments());
    let path = fs::canonicalize(file).ok()?;
    (path.starts_with(&root) && path.is_file()).then_some(LibraryFile { path, kind })
}

#[cfg(test)]
mod tests {
    use super::Kind;

    #[test]
    fn serves_images_and_meshes_but_nothing_else() {
        assert_eq!(Kind::of("cover.JPG"), Some(Kind::Image));
        assert_eq!(Kind::of("dragon.stl.xxx"), Some(Kind::Stl));
        assert_eq!(Kind::of("head.STL.txt"), Some(Kind::Stl));
        assert_eq!(Kind::of("plate.3mf"), Some(Kind::ThreeMf));
        assert_eq!(Kind::of("bust.obj"), Some(Kind::Obj));
        for name in ["notes.txt", "project.lys", ".3dhoarddeck", "archive.zip", "noextension"] {
            assert_eq!(Kind::of(name), None, "{name}");
        }
    }
}
