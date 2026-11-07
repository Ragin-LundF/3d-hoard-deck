//! `hoard://` serves library files to the webview, read-only. Routes (after `hoard://localhost/`):
//! - `<library id>/<path>`: an image or mesh as it is on disk,
//! - `thumb/<library id>/<path>`: a cached 512 px cover thumbnail of an image,
//! - `plates/<library id>/<path>`: a 3MF's build plates, flattened for the viewer.
//!
//! The UI builds these URLs with `convertFileSrc(path, "hoard")`, which picks the right form per platform.

mod files;
mod pool;

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
    pool::run(
        lane(&path),
        Box::new(move |dropped| {
            // A request dropped from a full queue still gets an answer, so the webview never waits forever.
            let response =
                if dropped { status(StatusCode::SERVICE_UNAVAILABLE) } else { route(&app, &thumbnails, &path) };
            responder.respond(response);
        }),
    );
}

/// Meshes and plates are what the viewer waits for; everything else is a picture.
fn lane(uri_path: &str) -> pool::Lane {
    let lower = uri_path.to_ascii_lowercase();
    let mesh = [".stl", ".3mf", ".obj"].iter().any(|extension| lower.ends_with(extension)) || lower.contains(".stl.");
    if mesh { pool::Lane::Viewer } else { pool::Lane::Covers }
}

fn route(app: &App, thumbnails: &ThumbnailCache, uri_path: &str) -> Response<Vec<u8>> {
    let Some(decoded) = percent_decode_str(uri_path.trim_start_matches('/')).decode_utf8().ok() else {
        return status(StatusCode::BAD_REQUEST);
    };
    if let Some(rest) = decoded.strip_prefix("thumb/") {
        return serve_thumbnail(app, thumbnails, rest);
    }
    if let Some(rest) = decoded.strip_prefix("plates/") {
        return serve_plates(app, rest);
    }
    serve_file(app, &decoded)
}

fn serve_file(app: &App, path: &str) -> Response<Vec<u8>> {
    let Some(file) = resolve(app, path) else { return status(StatusCode::NOT_FOUND) };
    if fs::metadata(&file.path).map_or(true, |metadata| metadata.len() > file.kind.max_bytes()) {
        return status(StatusCode::PAYLOAD_TOO_LARGE);
    }
    read(&file.path, content_type(&file.path), "max-age=3600")
}

/// Covers in the grid: decoded and scaled once, then served from the cache folder.
fn serve_thumbnail(app: &App, thumbnails: &ThumbnailCache, path: &str) -> Response<Vec<u8>> {
    let Some(file) = resolve(app, path).filter(|file| file.kind == Kind::Image) else {
        return status(StatusCode::NOT_FOUND);
    };
    match thumbnails.get_or_create(&file.path) {
        // The URL names only the source, not its version: keep browser caching short so a replaced cover shows up.
        Ok(thumbnail) => read(&thumbnail, "image/jpeg", "max-age=60"),
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

fn ok(body: Vec<u8>, content_type: &str, cache: &str) -> Response<Vec<u8>> {
    Response::builder()
        .header(header::CONTENT_TYPE, content_type)
        .header(header::CACHE_CONTROL, cache)
        // The viewer fetch()es meshes from the app page's origin; <img> needs no CORS, fetch does.
        .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .body(body)
        .unwrap_or_else(|_| status(StatusCode::INTERNAL_SERVER_ERROR))
}

fn status(code: StatusCode) -> Response<Vec<u8>> {
    let mut response = Response::new(Vec::new());
    *response.status_mut() = code;
    // Without it the viewer sees a CORS failure instead of the status (e.g. 413 "too large").
    response.headers_mut().insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, header::HeaderValue::from_static("*"));
    response
}
