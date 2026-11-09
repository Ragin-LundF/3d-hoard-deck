use std::fs::{self, File, Metadata};
use std::io::{ErrorKind, Read};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::SyncSender;
use std::thread;
use std::time::{Duration, UNIX_EPOCH};

use crate::IoPriority;
use hoarddeck_application::ScanError;
use hoarddeck_application::ports::LibraryScanner;
use hoarddeck_application::scan::{CompiledRules, KnownDirs, ScanItem};
use hoarddeck_domain::{Entry, EntryKind, FileKind, RelPath, SNIFF_LEN, Sniff, classify_by_header, classify_by_name};

/// Parallel, read-only directory walker. Folders whose mtime is unchanged since the last scan are not listed again,
/// so a rescan of an unchanged network library only stats folders.
pub struct FsLibraryScanner {
    threads: usize,
    priority: Arc<IoPriority>,
}

impl FsLibraryScanner {
    /// `threads`: parallel folder reads; keep it low for network volumes to avoid thrashing the NAS.
    #[must_use]
    /// `priority`: the scan pauses briefly while the user waits for a picture or 3D file.
    pub fn new(threads: usize, priority: Arc<IoPriority>) -> Self {
        Self { threads: threads.max(1), priority }
    }
}

struct Walk<'a> {
    root: PathBuf,
    known: &'a KnownDirs,
    rules: &'a CompiledRules,
    out: &'a SyncSender<ScanItem>,
    stopped: AtomicBool,
    priority: &'a IoPriority,
}

impl LibraryScanner for FsLibraryScanner {
    fn exists(&self, root: &str, path: &RelPath) -> Result<bool, String> {
        match fs::symlink_metadata(Path::new(root).join(path.as_str())) {
            Ok(_) => Ok(true),
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(false),
            Err(error) => Err(error.to_string()),
        }
    }

    fn scan(
        &self,
        root: &str,
        from: &RelPath,
        known: &KnownDirs,
        rules: &CompiledRules,
        out: &SyncSender<ScanItem>,
    ) -> Result<(), ScanError> {
        let root = PathBuf::from(root);
        let is_dir =
            fs::metadata(&root).map_err(|error| ScanError::Offline(format!("{}: {error}", root.display())))?.is_dir();
        if !is_dir {
            return Err(ScanError::Offline(format!("{} is not a folder", root.display())));
        }
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(self.threads)
            .build()
            .map_err(|error| ScanError::Aborted(error.to_string()))?;
        let walk = Walk { root, known, rules, out, stopped: AtomicBool::new(false), priority: &self.priority };
        pool.scope(|scope| visit(scope, &walk, from.clone()));
        if walk.stopped.load(Ordering::Relaxed) {
            return Err(ScanError::Aborted("the index writer stopped".to_owned()));
        }
        Ok(())
    }
}

fn visit<'s>(scope: &rayon::Scope<'s>, walk: &'s Walk<'s>, dir: RelPath) {
    if walk.stopped.load(Ordering::Relaxed) {
        return;
    }
    walk.priority.yield_to_foreground();
    let absolute = walk.absolute(&dir);
    let mtime = match with_retries(|| fs::metadata(&absolute)) {
        Ok(metadata) => mtime_millis(&metadata),
        // Gone since its parent was read, or a scanned folder deleted outside the app: reporting nothing lets the
        // end of the scan remove it. The library root itself was checked for being reachable before.
        Err(error) if error.kind() == ErrorKind::NotFound => return,
        Err(error) => return walk.send(ScanItem::Failed { dir, message: error.to_string() }),
    };
    let subdirs: Vec<RelPath> = match walk.known.get(&dir) {
        Some(known) if known.mtime == mtime => {
            let subdirs = known
                .child_dirs
                .iter()
                .filter_map(|name| dir.join(name).ok())
                .filter(|subdir| !walk.rules.is_ignored_dir(subdir))
                .collect();
            walk.send(ScanItem::Unchanged { dir });
            subdirs
        }
        _ => match with_retries(|| list(&absolute, &dir, walk.rules)) {
            Ok((children, unreadable)) => {
                let subdirs = children.iter().filter(|c| c.kind == EntryKind::Dir).map(|c| c.path.clone()).collect();
                // With an unreadable file the folder must be listed again next time, or that file stays unknown: an
                // mtime no folder has makes the next incremental scan list it.
                let mtime = if unreadable.is_empty() { mtime } else { i64::MIN };
                walk.send(ScanItem::Listed { dir, mtime, children });
                for (path, message) in unreadable {
                    walk.send(ScanItem::Failed { dir: path, message });
                }
                subdirs
            }
            Err(error) => return walk.send(ScanItem::Failed { dir, message: error.to_string() }),
        },
    };
    for subdir in subdirs {
        scope.spawn(move |scope| visit(scope, walk, subdir));
    }
}

impl Walk<'_> {
    fn absolute(&self, dir: &RelPath) -> PathBuf {
        let mut path = self.root.clone();
        path.extend(dir.segments());
        path
    }

    fn send(&self, item: ScanItem) {
        if self.out.send(item).is_err() {
            self.stopped.store(true, Ordering::Relaxed);
        }
    }
}

/// Children that could not be read, with the reason.
type Unreadable = Vec<(RelPath, String)>;

/// Direct children of a folder. Ignored paths, symlinks and names that are not valid UTF-8 are skipped. A child
/// that cannot be read is reported instead of failing the whole folder.
fn list(absolute: &Path, dir: &RelPath, rules: &CompiledRules) -> std::io::Result<(Vec<Entry>, Unreadable)> {
    let (mut children, mut unreadable) = (Vec::new(), Vec::new());
    for item in fs::read_dir(absolute)? {
        let item = item?;
        let Some(name) = item.file_name().to_str().map(str::to_owned) else { continue };
        let Ok(path) = dir.join(&name) else { continue };
        // Ignore rules by name come before any stat: macOS lists AppleDouble `._` files on SMB shares that it
        // cannot stat.
        if rules.is_ignored(&path) || rules.is_ignored_dir(&path) {
            continue;
        }
        let Ok(file_type) = item.file_type() else {
            unreadable.push((path, "the entry type could not be read".to_owned()));
            continue;
        };
        let ignored = if file_type.is_dir() { rules.is_ignored_dir(&path) } else { !file_type.is_file() };
        if ignored {
            continue;
        }
        let metadata = match item.metadata() {
            Ok(metadata) => metadata,
            // A folder stays a child with an mtime nothing has, so it is visited (and retried) on every scan.
            Err(_) if file_type.is_dir() => {
                children.push(Entry { path, kind: EntryKind::Dir, size: 0, mtime: 0 });
                continue;
            }
            Err(error) => {
                unreadable.push((path, error.to_string()));
                continue;
            }
        };
        let kind = if metadata.is_dir() {
            EntryKind::Dir
        } else {
            EntryKind::File(classify(&item.path(), &name, metadata.len()))
        };
        children.push(Entry {
            path,
            kind,
            size: if metadata.is_file() { metadata.len() } else { 0 },
            mtime: mtime_millis(&metadata),
        });
    }
    Ok((children, unreadable))
}

/// Network shares sometimes fail a listing for a moment (interrupted, timed out, busy, connection dropped, I/O
/// error); those are tried again.
fn is_transient(error: &std::io::Error) -> bool {
    const EIO: i32 = 5;
    matches!(
        error.kind(),
        ErrorKind::Interrupted
            | ErrorKind::TimedOut
            | ErrorKind::WouldBlock
            | ErrorKind::ResourceBusy
            | ErrorKind::NotConnected
            | ErrorKind::ConnectionReset
            | ErrorKind::ConnectionAborted
    ) || error.raw_os_error() == Some(EIO)
}

fn with_retries<T>(mut attempt: impl FnMut() -> std::io::Result<T>) -> std::io::Result<T> {
    let mut delay = Duration::from_millis(200);
    for _ in 0..2 {
        match attempt() {
            Err(error) if is_transient(&error) => {
                thread::sleep(delay);
                delay *= 4;
            }
            result => return result,
        }
    }
    attempt()
}

/// Extension first; disguised meshes (`.stl.xxx`, `.txt`) are confirmed by reading the first bytes, read-only.
fn classify(path: &Path, name: &str, size: u64) -> FileKind {
    match classify_by_name(name) {
        Sniff::Known(kind) => kind,
        Sniff::NeedsHeader => {
            read_header(path).map_or(FileKind::Other, |header| classify_by_header(name, &header, size))
        }
    }
}

fn read_header(path: &Path) -> std::io::Result<Vec<u8>> {
    let mut header = Vec::with_capacity(SNIFF_LEN);
    File::open(path)?.take(SNIFF_LEN as u64).read_to_end(&mut header)?;
    Ok(header)
}

fn mtime_millis(metadata: &Metadata) -> i64 {
    metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |duration| i64::try_from(duration.as_millis()).unwrap_or(i64::MAX))
}
