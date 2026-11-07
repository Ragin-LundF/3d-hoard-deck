//! Previews of STL files too large for the viewer: simplified once in Rust, then served from the index folder.
//! One simplification runs at a time, on its own thread; the reader that got the request is free right away.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Sender, channel};
use std::sync::{Mutex, OnceLock, PoisonError};
use std::thread;
use std::time::UNIX_EPOCH;

use hoarddeck_app::App;
use hoarddeck_application::ports::{StorageInspector, StorageKind};
use hoarddeck_infra_fs::SystemStorage;
use tauri::UriSchemeResponder;
use tauri::http::{HeaderValue, Response, StatusCode};

use super::files::LibraryFile;
use super::{Routed, ok, status};

/// Larger local files are not read at all; a preview of them would take minutes.
const MAX_SOURCE_BYTES: u64 = 8 * 1024 * 1024 * 1024;
/// A network share is read twice for a preview, so its limit is lower.
const MAX_SHARE_SOURCE_BYTES: u64 = 2 * 1024 * 1024 * 1024;
/// What the viewer still turns and zooms smoothly.
const TARGET_TRIANGLES: usize = 1_500_000;
/// Written into the 80-byte STL header, so a cached preview still knows how large its source was.
const HEADER_PREFIX: &str = "3dhoarddeck preview of ";
/// Tells the viewer that it shows a simplified preview, and of how many triangles.
const SIMPLIFIED_HEADER: &str = "x-simplified-from";

/// A preview still to be made.
pub(super) struct Job {
    source: PathBuf,
    cached: PathBuf,
}

static TEMPORARY: AtomicU64 = AtomicU64::new(0);
/// Library roots already checked for being a network share.
static STORAGE: Mutex<Option<HashMap<PathBuf, StorageKind>>> = Mutex::new(None);
static WORKER: OnceLock<Mutex<Sender<(Job, UriSchemeResponder)>>> = OnceLock::new();

/// A cached preview right away, a refusal, or the job that makes it.
pub(super) fn serve(app: &App, file: &LibraryFile, metadata: &fs::Metadata) -> Routed {
    // shortcut: previews are never cleaned up; delete the folder to reclaim the space.
    let cached = app.index_dir.join("previews").join(format!("{:016x}.stl", key(&file.path, metadata)));
    if let Ok(stl) = fs::read(&cached) {
        return Routed::Now(respond(stl));
    }
    let limit = if storage(&file.root) == StorageKind::Local { MAX_SOURCE_BYTES } else { MAX_SHARE_SOURCE_BYTES };
    if metadata.len() > limit {
        return Routed::Now(status(StatusCode::PAYLOAD_TOO_LARGE));
    }
    Routed::Later(Job { source: file.path.clone(), cached })
}

/// Queues the job for the one simplification thread, which answers the request when it is done.
pub(super) fn queue(job: Job, responder: UriSchemeResponder) {
    let worker = WORKER.get_or_init(|| {
        let (sender, receiver) = channel::<(Job, UriSchemeResponder)>();
        thread::spawn(move || {
            for (job, responder) in receiver {
                // An earlier job may have made this preview while this one waited.
                let made = fs::read(&job.cached).or_else(|_| create(&job.source, &job.cached));
                responder.respond(made.map_or_else(|_| status(StatusCode::UNPROCESSABLE_ENTITY), respond));
            }
        });
        Mutex::new(sender)
    });
    if let Err(failed) = worker.lock().unwrap_or_else(PoisonError::into_inner).send((job, responder)) {
        let (_, responder) = failed.0;
        responder.respond(status(StatusCode::INTERNAL_SERVER_ERROR));
    }
}

fn respond(stl: Vec<u8>) -> Response<Vec<u8>> {
    let original = String::from_utf8_lossy(&stl[..stl.len().min(80)])
        .strip_prefix(HEADER_PREFIX)
        .map(|rest| rest.trim_end_matches('\0').to_owned())
        .unwrap_or_default();
    let mut response = ok(stl, "application/octet-stream", "no-cache");
    if let Ok(value) = HeaderValue::from_str(&original) {
        response.headers_mut().insert(SIMPLIFIED_HEADER, value);
        // fetch() only shows a custom header the response names here.
        response.headers_mut().insert("access-control-expose-headers", HeaderValue::from_static(SIMPLIFIED_HEADER));
    }
    response
}

fn storage(root: &Path) -> StorageKind {
    let mut known = STORAGE.lock().unwrap_or_else(PoisonError::into_inner);
    *known.get_or_insert_with(HashMap::new).entry(root.to_path_buf()).or_insert_with(|| SystemStorage.kind(root))
}

/// FNV-1a over path, size and modification time: stable across Rust versions, unlike `DefaultHasher`. A changed file
/// gets a new preview.
fn key(source: &Path, metadata: &fs::Metadata) -> u64 {
    let modified =
        metadata.modified().ok().and_then(|time| time.duration_since(UNIX_EPOCH).ok()).map_or(0, |d| d.as_nanos());
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    let bytes = source.to_string_lossy();
    for byte in bytes.as_bytes().iter().chain(&metadata.len().to_le_bytes()).chain(&modified.to_le_bytes()) {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    hash
}

fn create(source: &Path, cached: &Path) -> Result<Vec<u8>, String> {
    let simplified = hoarddeck_infra_mesh::simplify_stl(source, TARGET_TRIANGLES).map_err(|error| error.0)?;
    let mut stl = simplified.stl;
    let label = format!("{HEADER_PREFIX}{}", simplified.original_triangles);
    stl[..label.len().min(80)].copy_from_slice(&label.as_bytes()[..label.len().min(80)]);
    if let Some(dir) = cached.parent() {
        // A preview that cannot be cached is still shown; it is only made again next time. The temporary name is
        // unique, so two writers never mix their bytes.
        let temporary = cached.with_extension(format!("{}.tmp", TEMPORARY.fetch_add(1, Ordering::Relaxed)));
        let written = fs::create_dir_all(dir).and_then(|()| fs::write(&temporary, &stl));
        if written.and_then(|()| fs::rename(&temporary, cached)).is_err() {
            let _ = fs::remove_file(&temporary);
        }
    }
    Ok(stl)
}
