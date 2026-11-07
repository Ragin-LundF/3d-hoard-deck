//! `hoard://localhost/<library id>/<relative path>`: serves library images to the webview, read-only.
//! The UI builds these URLs with `convertFileSrc(path, "hoard")`, which picks the right form per platform.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Sender, channel};
use std::sync::{Arc, Mutex, OnceLock};

use hoarddeck_app::App;
use hoarddeck_domain::{LibraryId, RelPath};
use percent_encoding::percent_decode_str;
use tauri::http::{Request, Response, StatusCode, header};
use tauri::{Manager, Runtime, UriSchemeContext, UriSchemeResponder};

pub const SCHEME: &str = "hoard";

/// Parallel file reads; a grid page asks for ~100 covers at once and the NAS should not get them all at the same time.
const READERS: usize = 4;
/// shortcut: images are served at full size up to this limit; replace with the thumbnail cache in milestone 3.
const MAX_BYTES: u64 = 40 * 1024 * 1024;

type Job = Box<dyn FnOnce() + Send>;

#[allow(clippy::needless_pass_by_value)] // signature required by Tauri's protocol registration
pub fn handle<R: Runtime>(context: UriSchemeContext<'_, R>, request: Request<Vec<u8>>, responder: UriSchemeResponder) {
    let app = context.app_handle().state::<Arc<App>>().inner().clone();
    let path = request.uri().path().to_owned();
    run(Box::new(move || responder.respond(serve(&app, &path))));
}

/// Queues work on a small fixed pool of reader threads, started on first use.
fn run(job: Job) {
    static QUEUE: OnceLock<Mutex<Sender<Job>>> = OnceLock::new();
    let queue = QUEUE.get_or_init(|| {
        let (sender, receiver) = channel::<Job>();
        let receiver = Arc::new(Mutex::new(receiver));
        for _ in 0..READERS {
            let receiver = Arc::clone(&receiver);
            std::thread::spawn(move || {
                loop {
                    // Take the next job and release the lock before running it, so readers work in parallel.
                    let next = match receiver.lock() {
                        Ok(queue) => queue.recv(),
                        Err(_) => return,
                    };
                    match next {
                        Ok(job) => job(),
                        Err(_) => return,
                    }
                }
            });
        }
        Mutex::new(sender)
    });
    if let Ok(sender) = queue.lock() {
        // The workers live as long as the app; a failed send only happens during shutdown.
        let _ = sender.send(job);
    }
}

fn serve(app: &App, uri_path: &str) -> Response<Vec<u8>> {
    let Some((file, content_type)) = resolve(app, uri_path) else { return status(StatusCode::NOT_FOUND) };
    match fs::metadata(&file) {
        Ok(metadata) if metadata.len() > MAX_BYTES => return status(StatusCode::PAYLOAD_TOO_LARGE),
        Err(_) => return status(StatusCode::NOT_FOUND),
        Ok(_) => {}
    }
    match fs::read(&file) {
        Ok(bytes) => Response::builder()
            .header(header::CONTENT_TYPE, content_type)
            .header(header::CACHE_CONTROL, "max-age=3600")
            .body(bytes)
            .unwrap_or_else(|_| status(StatusCode::INTERNAL_SERVER_ERROR)),
        Err(_) => status(StatusCode::NOT_FOUND),
    }
}

/// Maps the URL to an image inside the library root. Rejects other file types, traversal and symlinks that
/// point outside the root.
fn resolve(app: &App, uri_path: &str) -> Option<(PathBuf, &'static str)> {
    let decoded = percent_decode_str(uri_path.trim_start_matches('/')).decode_utf8().ok()?;
    let (library, relative) = decoded.split_once('/')?;
    let relative = RelPath::new(relative).ok().filter(|path| !path.is_root())?;
    let content_type = image_type(Path::new(relative.name()))?;
    let library = app.libraries.get(LibraryId(library.parse().ok()?)).ok()??;
    let root = fs::canonicalize(&library.root).ok()?;
    let mut file = root.clone();
    file.extend(relative.segments());
    let file = fs::canonicalize(file).ok()?;
    (file.starts_with(&root) && file.is_file()).then_some((file, content_type))
}

fn image_type(file: &Path) -> Option<&'static str> {
    let extension = file.extension()?.to_str()?.to_ascii_lowercase();
    match extension.as_str() {
        "jpg" | "jpeg" => Some("image/jpeg"),
        "png" => Some("image/png"),
        "webp" => Some("image/webp"),
        "gif" => Some("image/gif"),
        "bmp" => Some("image/bmp"),
        _ => None,
    }
}

fn status(code: StatusCode) -> Response<Vec<u8>> {
    let mut response = Response::new(Vec::new());
    *response.status_mut() = code;
    response
}
