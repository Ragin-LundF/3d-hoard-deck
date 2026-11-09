//! File fingerprints for the duplicate finder. Reads only.

use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use std::sync::Arc;

use hoarddeck_application::ports::FileHasher;

use crate::IoPriority;
use hoarddeck_domain::RelPath;

/// Bytes read from the start and from the end for the quick fingerprint.
const EDGE: u64 = 64 * 1024;

/// Steps back for pictures and 3D files the user is waiting for, like the scanner.
pub struct Blake3Hasher(pub Arc<IoPriority>);

impl FileHasher for Blake3Hasher {
    fn quick(&self, root: &str, file: &RelPath) -> Result<[u8; 32], String> {
        self.0.yield_to_foreground();
        quick(&absolute(root, file)).map_err(|error| format!("{file}: {error}"))
    }

    fn full(&self, root: &str, file: &RelPath) -> Result<[u8; 32], String> {
        self.0.yield_to_foreground();
        let mut hasher = blake3::Hasher::new();
        hasher
            .update_reader(File::open(absolute(root, file)).map_err(|e| format!("{file}: {e}"))?)
            .map_err(|e| format!("{file}: {e}"))?;
        Ok(*hasher.finalize().as_bytes())
    }
}

/// Size plus the first and last 64 KB: cheap over a network share, and equal files always agree.
fn quick(path: &Path) -> io::Result<[u8; 32]> {
    let mut file = File::open(path)?;
    let size = file.metadata()?.len();
    let mut hasher = blake3::Hasher::new();
    hasher.update(&size.to_le_bytes());
    let mut buffer = Vec::new();
    // `take` + `read_to_end` keeps reading until EDGE bytes or the end; a single `read` may stop early on a share.
    (&mut file).take(EDGE).read_to_end(&mut buffer)?;
    hasher.update(&buffer);
    buffer.resize(usize::try_from(EDGE).unwrap_or(usize::MAX), 0);
    if size > EDGE * 2 {
        file.seek(SeekFrom::End(-i64::try_from(EDGE).unwrap_or(i64::MAX)))?;
        file.read_exact(&mut buffer)?;
        hasher.update(&buffer);
    }
    Ok(*hasher.finalize().as_bytes())
}

fn absolute(root: &str, file: &RelPath) -> PathBuf {
    let mut path = PathBuf::from(root);
    path.extend(file.segments());
    path
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equal_files_agree_and_a_change_in_the_middle_is_caught_by_the_full_hash() {
        let dir = tempfile::tempdir().expect("temp dir");
        let mut content = vec![7_u8; 300 * 1024];
        std::fs::write(dir.path().join("a.stl"), &content).expect("write a");
        std::fs::write(dir.path().join("b.stl"), &content).expect("write b");
        content[150 * 1024] = 8;
        std::fs::write(dir.path().join("c.stl"), &content).expect("write c");
        let root = dir.path().to_str().expect("utf-8");
        let file = |name: &str| RelPath::new(name).expect("valid");
        let hasher = Blake3Hasher(Arc::default());
        assert_eq!(hasher.quick(root, &file("a.stl")), hasher.quick(root, &file("b.stl")));
        assert_eq!(hasher.quick(root, &file("a.stl")), hasher.quick(root, &file("c.stl")), "the middle is not sampled");
        assert_ne!(hasher.full(root, &file("a.stl")), hasher.full(root, &file("c.stl")));
    }
}
