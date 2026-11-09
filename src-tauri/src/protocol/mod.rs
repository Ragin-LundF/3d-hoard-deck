//! `hoard://` serves library files to the webview, read-only. Routes (after `hoard://localhost/`):
//! - `<library id>/<path>`: an image or mesh as it is on disk,
//! - `thumb/<library id>/<path>`: a cached 512 px cover thumbnail of an image,
//! - `plates/<library id>/<path>`: a 3MF's build plates, flattened for the viewer,
//! - `<library id>/<path>.zip!/<entry>`: an image, STL or OBJ from inside a ZIP archive.
//!
//! The UI builds these URLs with `convertFileSrc(path, "hoard")`, which picks the right form per platform.

mod files;
mod pool;
mod simplified;

use std::fs;
use std::path::Path;
use std::sync::Arc;

use hoarddeck_app::App;
use hoarddeck_infra_thumbs::ThumbnailCache;
use percent_encoding::percent_decode_str;
use tauri::http::{Request, Response, StatusCode, header};
use tauri::{Manager, Runtime, UriSchemeContext, UriSchemeResponder};

use files::{Kind, content_type, resolve};

pub const SCHEME: &str = "hoard";

#[allow(clippy::needless_pass_by_value)] // signature required by Tauri's protocol registration
pub fn handle<R: Runtime>(context: UriSchemeContext<'_, R>, request: Request<Vec<u8>>, responder: UriSchemeResponder) {
    let handle = context.app_handle();
    let app = handle.state::<Arc<App>>().inner().clone();
    let thumbnails = handle.state::<Arc<ThumbnailCache>>().inner().clone();
    let path = request.uri().path().to_owned();
    // While the user waits for a picture or 3D file from the library, a running scan steps back. Covers already in the
    // thumbnail cache need no library read, so they take the guard only when the thumbnail has to be made.
    let local = ["thumb/", "rendered/"].iter().any(|prefix| path.trim_start_matches('/').starts_with(prefix));
    let foreground = (!local).then(|| app.io_priority.foreground());
    pool::run(
        lane(&path),
        Box::new(move |dropped| {
            let _foreground = foreground;
            // A request dropped from a full queue still gets an answer, so the webview never waits forever.
            if dropped {
                return responder.respond(status(StatusCode::SERVICE_UNAVAILABLE));
            }
            match route(&app, &thumbnails, &path) {
                Routed::Now(response) => responder.respond(response),
                // Simplifying takes long; it runs on its own thread, so this reader is free again.
                Routed::Later(job) => simplified::queue(job, responder),
            }
        }),
    );
}

/// Meshes and plates are what the viewer waits for; everything else is a picture.
fn lane(uri_path: &str) -> pool::Lane {
    let lower = uri_path.to_ascii_lowercase();
    let mesh = [".stl", ".3mf", ".obj"].iter().any(|extension| lower.ends_with(extension)) || lower.contains(".stl.");
    if mesh { pool::Lane::Viewer } else { pool::Lane::Covers }
}

/// An answer now, or a simplification job that answers when it is done.
enum Routed {
    Now(Response<Vec<u8>>),
    Later(simplified::Job),
}

fn route(app: &App, thumbnails: &ThumbnailCache, uri_path: &str) -> Routed {
    let Some(decoded) = percent_decode_str(uri_path.trim_start_matches('/')).decode_utf8().ok() else {
        return Routed::Now(status(StatusCode::BAD_REQUEST));
    };
    if let Some(id) = decoded.strip_prefix("rendered/") {
        // Previews rendered in the app live in its own folder; only a model id is accepted, never a path.
        return Routed::Now(match id.parse::<i64>() {
            Ok(id) => read(&thumbnails.rendered_path(hoarddeck_domain::ModelId(id)), "image/png", "no-cache"),
            Err(_) => status(StatusCode::BAD_REQUEST),
        });
    }
    if let Some(rest) = decoded.strip_prefix("thumb/") {
        return Routed::Now(serve_thumbnail(app, thumbnails, rest));
    }
    if let Some(rest) = decoded.strip_prefix("plates/") {
        return Routed::Now(serve_plates(app, rest));
    }
    if let Some((archive, entry)) = split_archive(&decoded) {
        return Routed::Now(serve_archive_entry(app, archive, entry));
    }
    serve_file(app, &decoded)
}

fn serve_file(app: &App, path: &str) -> Routed {
    let Some(file) = resolve(app, path).filter(|file| file.kind != Kind::Zip) else {
        return Routed::Now(status(StatusCode::NOT_FOUND));
    };
    let Ok(metadata) = fs::metadata(&file.path) else { return Routed::Now(status(StatusCode::NOT_FOUND)) };
    if metadata.len() <= file.kind.max_bytes() {
        return Routed::Now(read(&file.path, content_type(&file.path), "max-age=3600"));
    }
    if file.kind == Kind::Stl {
        return simplified::serve(app, &file, &metadata);
    }
    Routed::Now(status(StatusCode::PAYLOAD_TOO_LARGE))
}

/// Covers in the grid: decoded and scaled once, then served from the cache folder.
fn serve_thumbnail(app: &App, thumbnails: &ThumbnailCache, path: &str) -> Response<Vec<u8>> {
    // A 3MF stands for the preview picture inside it.
    let Some(file) = resolve(app, path).filter(|file| matches!(file.kind, Kind::Image | Kind::ThreeMf)) else {
        return status(StatusCode::NOT_FOUND);
    };
    let cached = thumbnails.cached(&file.path).ok().flatten();
    let _foreground = cached.is_none().then(|| app.io_priority.foreground());
    match cached.map_or_else(|| thumbnails.get_or_create(&file.path), Ok) {
        // The URL names only the source, not its version: keep browser caching short so a replaced cover shows up.
        Ok(thumbnail) => read(&thumbnail, "image/jpeg", "max-age=60"),
        Err(_) => status(StatusCode::UNPROCESSABLE_ENTITY),
    }
}

/// `pack.zip!/dragon.stl` → (`pack.zip`, `dragon.stl`). Only after `.zip`, so a folder like `Halloween!/` stays a folder.
fn split_archive(decoded: &str) -> Option<(&str, &str)> {
    let at = decoded.to_ascii_lowercase().find(".zip!/")?;
    Some((&decoded[..at + 4], &decoded[at + 6..]))
}

/// 3MF inside a ZIP is not served: the plate reader needs the file on disk.
fn serve_archive_entry(app: &App, archive: &str, entry: &str) -> Response<Vec<u8>> {
    let Some(file) = resolve(app, archive).filter(|file| file.kind == Kind::Zip) else {
        return status(StatusCode::NOT_FOUND);
    };
    let Some(kind) = Kind::of(entry).filter(|kind| matches!(kind, Kind::Image | Kind::Stl | Kind::Obj)) else {
        return status(StatusCode::NOT_FOUND);
    };
    match hoarddeck_infra_mesh::read_archive_entry(&file.path, entry, kind.max_bytes()) {
        Ok(bytes) => ok(bytes, content_type(Path::new(entry)), "max-age=3600"),
        Err(_) => status(StatusCode::UNPROCESSABLE_ENTITY),
    }
}

fn serve_plates(app: &App, path: &str) -> Response<Vec<u8>> {
    let Some(file) = resolve(app, path).filter(|file| file.kind == Kind::ThreeMf) else {
        return status(StatusCode::NOT_FOUND);
    };
    if fs::metadata(&file.path).map_or(true, |metadata| metadata.len() > file.kind.max_bytes()) {
        return status(StatusCode::PAYLOAD_TOO_LARGE);
    }
    match hoarddeck_infra_mesh::read_plates(&file.path) {
        Ok(plates) => ok(hoarddeck_infra_mesh::encode_plates(&plates), "application/octet-stream", "no-cache"),
        Err(_) => status(StatusCode::UNPROCESSABLE_ENTITY),
    }
}

fn read(file: &Path, content_type: &str, cache: &str) -> Response<Vec<u8>> {
    fs::read(file).map_or_else(|_| status(StatusCode::NOT_FOUND), |bytes| ok(bytes, content_type, cache))
}

pub(super) fn ok(body: Vec<u8>, content_type: &str, cache: &str) -> Response<Vec<u8>> {
    Response::builder()
        .header(header::CONTENT_TYPE, content_type)
        .header(header::CACHE_CONTROL, cache)
        // The viewer fetch()es meshes from the app page's origin; <img> needs no CORS, fetch does.
        .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .body(body)
        .unwrap_or_else(|_| status(StatusCode::INTERNAL_SERVER_ERROR))
}

pub(super) fn status(code: StatusCode) -> Response<Vec<u8>> {
    let mut response = Response::new(Vec::new());
    *response.status_mut() = code;
    // Without it the viewer sees a CORS failure instead of the status (e.g. 413 "too large").
    response.headers_mut().insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, header::HeaderValue::from_static("*"));
    response
}

#[cfg(test)]
mod tests {
    use super::split_archive;

    #[test]
    fn only_zip_paths_are_split() {
        assert_eq!(split_archive("1/Packs/Pack.ZIP!/a/dragon.stl"), Some(("1/Packs/Pack.ZIP", "a/dragon.stl")));
        assert_eq!(split_archive("1/Halloween Special!/dragon.stl"), None);
    }
}
