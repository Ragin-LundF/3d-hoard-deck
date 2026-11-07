//! File classification: by name first, by content for disguised meshes (`.stl.xxx`, `.stl.txt`).

use crate::{FileFormat, FileKind};

/// How many leading bytes the content sniffer needs.
pub const SNIFF_LEN: usize = 1024;

/// Result of classifying by name: either final, or the first [`SNIFF_LEN`] bytes are needed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sniff {
    Known(FileKind),
    NeedsHeader,
}

/// Classifies by file name. Disguised meshes (`x.stl.xxx`, `x.stl.txt`, any `.txt`) need their header.
#[must_use]
pub fn classify_by_name(name: &str) -> Sniff {
    let lower = name.to_ascii_lowercase();
    let extension = lower.rsplit_once('.').map_or("", |(_, ext)| ext);
    let kind = match extension {
        "stl" => FileKind::Mesh(FileFormat::Stl),
        "3mf" => FileKind::Mesh(FileFormat::ThreeMf),
        "obj" => FileKind::Mesh(FileFormat::Obj),
        "ply" => FileKind::Mesh(FileFormat::Ply),
        "step" | "stp" => FileKind::Mesh(FileFormat::Step),
        "gltf" | "glb" => FileKind::Mesh(FileFormat::Gltf),
        "fbx" => FileKind::Mesh(FileFormat::Fbx),
        "blend" => FileKind::Mesh(FileFormat::Blend),
        "lys" | "lychee" | "chitubox" | "ctb" | "goo" | "gcode" | "bgcode" => FileKind::SlicerProject,
        "jpg" | "jpeg" | "png" | "webp" | "gif" | "bmp" => FileKind::Image,
        "pdf" | "md" | "html" | "htm" => FileKind::Document,
        "zip" | "rar" | "7z" => FileKind::Archive,
        "txt" | "xxx" => return Sniff::NeedsHeader,
        _ if lower.contains(".stl.") => return Sniff::NeedsHeader,
        _ => FileKind::Other,
    };
    Sniff::Known(kind)
}

/// Classifies by content: a binary STL has `84 + 50 × triangles` bytes, an ASCII STL starts with `solid` and has facets.
#[must_use]
pub fn classify_by_header(name: &str, header: &[u8], size: u64) -> FileKind {
    if is_binary_stl(header, size) || is_ascii_stl(header) {
        return FileKind::Mesh(FileFormat::Stl);
    }
    if name.to_ascii_lowercase().ends_with(".txt") { FileKind::Document } else { FileKind::Other }
}

/// Some exporters append a few bytes after the last triangle; up to this many are tolerated.
const BINARY_STL_SLACK: u64 = 512;

fn is_binary_stl(header: &[u8], size: u64) -> bool {
    let Some(count) = header.get(80..84) else { return false };
    let triangles = u64::from(u32::from_le_bytes([count[0], count[1], count[2], count[3]]));
    let expected = 84 + 50 * triangles;
    triangles > 0 && size >= expected && size - expected <= BINARY_STL_SLACK
}

fn is_ascii_stl(header: &[u8]) -> bool {
    let text = String::from_utf8_lossy(header);
    let trimmed = text.trim_start();
    trimmed.starts_with("solid") && (trimmed.contains("facet") || trimmed.contains("endsolid"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::FileFormat;

    fn binary_stl_header(triangles: u32) -> Vec<u8> {
        let mut header = vec![0_u8; 84];
        header[80..84].copy_from_slice(&triangles.to_le_bytes());
        header
    }

    #[test]
    fn known_extensions_need_no_header() {
        assert_eq!(classify_by_name("Turtle.STL"), Sniff::Known(FileKind::Mesh(FileFormat::Stl)));
        assert_eq!(classify_by_name("plate.3mf"), Sniff::Known(FileKind::Mesh(FileFormat::ThreeMf)));
        assert_eq!(classify_by_name("DSC_1234.jpg"), Sniff::Known(FileKind::Image));
        assert_eq!(classify_by_name("ogre.lys"), Sniff::Known(FileKind::SlicerProject));
        assert_eq!(classify_by_name("config.orynt3d"), Sniff::Known(FileKind::Other));
    }

    #[test]
    fn disguised_names_need_the_header() {
        assert_eq!(classify_by_name("dragon.stl.xxx"), Sniff::NeedsHeader);
        assert_eq!(classify_by_name("dragon.stl.txt"), Sniff::NeedsHeader);
        assert_eq!(classify_by_name("readme.txt"), Sniff::NeedsHeader);
    }

    #[test]
    fn binary_stl_is_recognized_by_size_equation() {
        let header = binary_stl_header(10);
        assert_eq!(classify_by_header("a.stl.xxx", &header, 84 + 500), FileKind::Mesh(FileFormat::Stl));
        assert_eq!(classify_by_header("a.stl.xxx", &header, 84 + 499), FileKind::Other);
        assert_eq!(
            classify_by_header("a.stl.xxx", &header, 84 + 500 + 2),
            FileKind::Mesh(FileFormat::Stl),
            "trailing bytes"
        );
        assert_eq!(classify_by_header("a.stl.xxx", &header, 84 + 500 + 4096), FileKind::Other);
    }

    #[test]
    fn ascii_stl_and_plain_text_are_told_apart() {
        let stl = b"solid dragon\n  facet normal 0 0 1\n";
        assert_eq!(classify_by_header("dragon.stl.txt", stl, 999), FileKind::Mesh(FileFormat::Stl));
        assert_eq!(classify_by_header("readme.txt", b"solid advice: print slow", 24), FileKind::Document);
    }
}
