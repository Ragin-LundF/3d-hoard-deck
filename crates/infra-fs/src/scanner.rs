use std::fs::{self, File, Metadata};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::SyncSender;
use std::time::UNIX_EPOCH;

use hoarddeck_application::ScanError;
use hoarddeck_application::ports::LibraryScanner;
use hoarddeck_application::scan::{CompiledRules, KnownDirs, ScanItem};
use hoarddeck_domain::{Entry, EntryKind, FileKind, RelPath, SNIFF_LEN, Sniff, classify_by_header, classify_by_name};

/// Parallel, read-only directory walker. Folders whose mtime is unchanged since the last scan are not listed again,
/// so a rescan of an unchanged network library only stats folders.
pub struct FsLibraryScanner {
    threads: usize,
}

impl FsLibraryScanner {
    /// `threads`: parallel folder reads; keep it low for network volumes to avoid thrashing the NAS.
    #[must_use]
    pub fn new(threads: usize) -> Self {
        Self { threads: threads.max(1) }
    }
}

struct Walk<'a> {
    root: PathBuf,
    known: &'a KnownDirs,
    rules: &'a CompiledRules,
    out: &'a SyncSender<ScanItem>,
    stopped: AtomicBool,
}

impl LibraryScanner for FsLibraryScanner {
    fn scan(
        &self,
        root: &str,
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
        let walk = Walk { root, known, rules, out, stopped: AtomicBool::new(false) };
        pool.scope(|scope| visit(scope, &walk, RelPath::root()));
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
    let absolute = walk.absolute(&dir);
    let mtime = match fs::metadata(&absolute) {
        Ok(metadata) => mtime_millis(&metadata),
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
        _ => match list(&absolute, &dir, walk.rules) {
            Ok(children) => {
                let subdirs = children.iter().filter(|c| c.kind == EntryKind::Dir).map(|c| c.path.clone()).collect();
                walk.send(ScanItem::Listed { dir, mtime, children });
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

/// Direct children of a folder. Ignored paths, symlinks and names that are not valid UTF-8 are skipped.
fn list(absolute: &Path, dir: &RelPath, rules: &CompiledRules) -> std::io::Result<Vec<Entry>> {
    let mut children = Vec::new();
    for item in fs::read_dir(absolute)? {
        let item = item?;
        let metadata = item.metadata()?;
        let Some(name) = item.file_name().to_str().map(str::to_owned) else { continue };
        let Ok(path) = dir.join(&name) else { continue };
        let kind = if metadata.is_dir() {
            if rules.is_ignored_dir(&path) {
                continue;
            }
            EntryKind::Dir
        } else if metadata.is_file() {
            if rules.is_ignored(&path) {
                continue;
            }
            EntryKind::File(classify(&item.path(), &name, metadata.len()))
        } else {
            continue;
        };
        children.push(Entry {
            path,
            kind,
            size: if metadata.is_file() { metadata.len() } else { 0 },
            mtime: mtime_millis(&metadata),
        });
    }
    Ok(children)
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
