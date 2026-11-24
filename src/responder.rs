//! Request routing and the file responder — the heart of YhHTTPServer.

use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use hyper::body::Incoming;
use hyper::header::{
    HeaderName, HeaderValue, ACCEPT_RANGES, ALLOW, CACHE_CONTROL, CONTENT_ENCODING, CONTENT_LENGTH,
    CONTENT_RANGE, CONTENT_TYPE, ETAG, IF_MODIFIED_SINCE, IF_NONE_MATCH, LAST_MODIFIED, RANGE,
    SERVER, VARY,
};
use hyper::{HeaderMap, Method, Request, Response, StatusCode};
use tokio::io::{AsyncReadExt, AsyncSeekExt};

use crate::body::{self, ResBody};
use crate::compress::{self, Encoding};
use crate::fsutil::{self, append_ext, has_extension};
use crate::listing::error_html;
use crate::logging::Record;
use crate::state::State;

/// hyper entry point for a single request.
pub async fn handle(
    state: Arc<State>,
    req: Request<Incoming>,
    peer: SocketAddr,
) -> Response<ResBody> {
    let started = Instant::now();
    let method = req.method().clone();
    let path = req.uri().path().to_string();
    let query = req.uri().query().map(str::to_string);
    let version = match req.version() {
        hyper::Version::HTTP_09 => "0.9",
        hyper::Version::HTTP_10 => "1.0",
        hyper::Version::HTTP_11 => "1.1",
        hyper::Version::HTTP_2 => "2",
        hyper::Version::HTTP_3 => "3",
        _ => "1.1",
    };
    let user_agent = req
        .headers()
        .get(hyper::header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);
    let referer = req
        .headers()
        .get(hyper::header::REFERER)
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);
    let client_ip = state.client_ip(req.headers(), &peer.ip().to_string());

    let mut resp = route(&state, req).await;
    decorate(&state, &mut resp);

    if state.log.enabled() {
        let length = resp
            .headers()
            .get(CONTENT_LENGTH)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u64>().ok());
        state.log.log(&Record {
            method: method.as_str(),
            path: &path,
            query: query.as_deref(),
            version,
            status: resp.status(),
            length,
            elapsed: started.elapsed(),
            client_ip: &client_ip,
            user_agent: user_agent.as_deref(),
            referer: referer.as_deref(),
        });
    }

    resp
}

/// Headers added to every response (Server, CORS, user supplied).
fn decorate(state: &State, resp: &mut Response<ResBody>) {
    let h = resp.headers_mut();
    if state.cfg.server_header {
        h.insert(
            SERVER,
            HeaderValue::from_static(concat!("yhtps/", env!("CARGO_PKG_VERSION"))),
        );
    }
    if let Some(origin) = state.cors_origin() {
        if let Ok(v) = HeaderValue::from_str(&origin) {
            h.insert(hyper::header::ACCESS_CONTROL_ALLOW_ORIGIN, v);
            h.insert(
                hyper::header::ACCESS_CONTROL_ALLOW_METHODS,
                HeaderValue::from_static("GET, HEAD, OPTIONS"),
            );
            h.insert(
                hyper::header::ACCESS_CONTROL_ALLOW_HEADERS,
                HeaderValue::from_static("*"),
            );
        }
    }
    for (name, value) in &state.cfg.headers {
        if let (Ok(n), Ok(v)) = (
            HeaderName::from_bytes(name.as_bytes()),
            HeaderValue::from_str(value),
        ) {
            h.insert(n, v);
        }
    }
}

/// What a URL resolves to on disk.
enum Target {
    /// Directory without a trailing slash: tell the client to re-ask with `/`.
    Redirect(String),
    Dir(PathBuf),
    File(PathBuf),
    Missing,
}

/// Map a URL path onto the served tree.
fn resolve(state: &State, url_path: &str) -> Result<Target, ()> {
    let segments = fsutil::safe_segments(url_path)?;
    let fs_path = fsutil::join_segments(&state.cfg.root, &segments);
    let cfg = &state.cfg;

    if let Ok(meta) = std::fs::metadata(&fs_path) {
        if meta.is_dir() {
            if !url_path.ends_with('/') {
                return Ok(Target::Redirect(format!("{url_path}/")));
            }
            return Ok(Target::Dir(fs_path));
        }
        if meta.is_file() {
            return Ok(Target::File(fs_path));
        }
    }

    // `/about` -> `/about.html`
    if !has_extension(&fs_path) {
        for ext in &cfg.extensions {
            let cand = append_ext(&fs_path, ext);
            if cand.is_file() {
                return Ok(Target::File(cand));
            }
        }
    }

    // `/blog/` -> `/blog/index.html`
    if cfg.clean_urls {
        for idx in &cfg.index_files {
            let cand = fs_path.join(idx);
            if cand.is_file() {
                return Ok(Target::File(cand));
            }
        }
    }

    Ok(Target::Missing)
}

async fn route(state: &State, req: Request<Incoming>) -> Response<ResBody> {
    let cfg = &state.cfg;

    if let Some((user, pass)) = &cfg.auth {
        if !crate::auth::authorized(req.headers(), user, pass) {
            return crate::auth::challenge("yhtps");
        }
    }

    match *req.method() {
        Method::GET | Method::HEAD => {}
        Method::OPTIONS => {
            let mut resp = Response::new(body::empty());
            *resp.status_mut() = StatusCode::NO_CONTENT;
            resp.headers_mut()
                .insert(ALLOW, HeaderValue::from_static("GET, HEAD, OPTIONS"));
            return resp;
        }
        _ => {
            let mut resp = error_html(
                StatusCode::METHOD_NOT_ALLOWED,
                "405 Method Not Allowed",
                &format!(
                    "The {} method is not supported by this static file server.",
                    req.method()
                ),
            );
            resp.headers_mut()
                .insert(ALLOW, HeaderValue::from_static("GET, HEAD, OPTIONS"));
            return resp;
        }
    }

    let head_only = req.method() == Method::HEAD;
    let url_path = req.uri().path().to_string();
    let query = req.uri().query().map(str::to_string);
    let headers = req.headers().clone();

    // A proxy with an explicit prefix wins immediately (e.g. `--proxy-prefix /api`).
    if let (Some(target), Some(prefix)) = (&cfg.proxy, &cfg.proxy_prefix) {
        if url_path.starts_with(prefix.as_str()) {
            return crate::proxy::forward(state, target, req).await;
        }
    }

    match resolve(state, &url_path) {
        Err(()) => error_html(
            StatusCode::BAD_REQUEST,
            "400 Bad Request",
            "The requested path contains an illegal segment.",
        ),
        Ok(Target::Redirect(to)) => {
            let loc = match query {
                Some(q) => format!("{to}?{q}"),
                None => to,
            };
            redirect(&loc)
        }
        Ok(Target::Dir(fs_path)) => {
            for idx in &cfg.index_files {
                let cand = fs_path.join(idx);
                if cand.is_file() {
                    return respond_file(state, cand, &headers, head_only).await;
                }
            }
            crate::listing::respond(&fs_path, &url_path, query.as_deref(), cfg.listing).await
        }
        Ok(Target::File(f)) => respond_file(state, f, &headers, head_only).await,
        Ok(Target::Missing) => {
            // Bare `--proxy` acts as a fallback for anything not on disk.
            if let (Some(target), None) = (&cfg.proxy, &cfg.proxy_prefix) {
                return crate::proxy::forward(state, target, req).await;
            }
            if cfg.spa {
                let fallback = cfg.root.join(&cfg.fallback);
                if fallback.is_file() {
                    return respond_file(state, fallback, &headers, head_only).await;
                }
            }
            error_html(
                StatusCode::NOT_FOUND,
                "404 Not Found",
                &format!("Cannot find {url_path} on this server."),
            )
        }
    }
}

fn redirect(location: &str) -> Response<ResBody> {
    let html = format!(
        "<!doctype html><meta charset=\"utf-8\"><title>301 Moved Permanently</title>\
<meta http-equiv=\"refresh\" content=\"0; url={loc}\">\
<p>Moved to <a href=\"{loc}\">{loc}</a></p>",
        loc = fsutil::html_escape(location)
    );
    let mut resp = Response::new(body::full(html));
    *resp.status_mut() = StatusCode::MOVED_PERMANENTLY;
    if let Ok(v) = HeaderValue::from_str(location) {
        resp.headers_mut().insert(hyper::header::LOCATION, v);
    }
    resp
}

/// Serve one file: conditional requests, byte ranges, compression, streaming.
async fn respond_file(
    state: &State,
    path: PathBuf,
    req_headers: &HeaderMap,
    head: bool,
) -> Response<ResBody> {
    let meta = match tokio::fs::metadata(&path).await {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return error_html(StatusCode::NOT_FOUND, "404 Not Found", "File not found.")
        }
        Err(e) => {
            return error_html(
                StatusCode::INTERNAL_SERVER_ERROR,
                "500 Internal Server Error",
                &format!("cannot stat file: {e}"),
            )
        }
    };
    if !meta.is_file() {
        return error_html(
            StatusCode::NOT_FOUND,
            "404 Not Found",
            "Not a regular file.",
        );
    }

    let size = meta.len();
    let modified = meta.modified().unwrap_or(UNIX_EPOCH);
    let etag = make_etag(size, modified);
    let last_mod = httpdate::fmt_http_date(modified);
    let mime = guess_mime(&path);

    // ---- conditional GET ---------------------------------------------
    if cond_not_modified(req_headers, &etag, modified) {
        let mut resp = Response::new(body::empty());
        *resp.status_mut() = StatusCode::NOT_MODIFIED;
        let h = resp.headers_mut();
        h.insert(ETAG, HeaderValue::from_str(&etag).unwrap());
        h.insert(LAST_MODIFIED, HeaderValue::from_str(&last_mod).unwrap());
        h.insert(CACHE_CONTROL, cache_control(state.cfg.cache_seconds));
        h.insert(ACCEPT_RANGES, HeaderValue::from_static("bytes"));
        return resp;
    }

    // ---- compression --------------------------------------------------
    let compressible = compress::is_compressible(&mime, &path);
    let want_enc = if compressible && size >= state.cfg.compress_threshold {
        compress::negotiate(req_headers, state.cfg.gzip, state.cfg.brotli)
    } else {
        None
    };

    // ---- range --------------------------------------------------------
    let range = if state.cfg.range && want_enc.is_none() {
        match parse_range(req_headers, size) {
            RangeResult::None => None,
            RangeResult::Satisfiable(a, b) => Some((a, b)),
            RangeResult::Unsatisfiable => {
                let mut resp = error_html(
                    StatusCode::RANGE_NOT_SATISFIABLE,
                    "416 Range Not Satisfiable",
                    "The requested byte range cannot be satisfied.",
                );
                resp.headers_mut().insert(
                    CONTENT_RANGE,
                    HeaderValue::from_str(&format!("bytes */{size}")).unwrap(),
                );
                return resp;
            }
        }
    } else {
        None
    };

    // ---- build ---------------------------------------------------------
    let mut builder = Response::builder()
        .header(CONTENT_TYPE, mime_header(&mime))
        .header(ETAG, etag)
        .header(LAST_MODIFIED, last_mod)
        .header(ACCEPT_RANGES, "bytes")
        .header(CACHE_CONTROL, cache_control(state.cfg.cache_seconds));

    if state.cfg.compression_enabled() && compressible {
        builder = builder.header(VARY, "Accept-Encoding");
    }

    let body = if head {
        if let Some(enc) = want_enc {
            builder = builder.header(CONTENT_ENCODING, enc.header_value());
        } else if let Some((a, b)) = range {
            builder = builder
                .status(StatusCode::PARTIAL_CONTENT)
                .header(CONTENT_RANGE, format!("bytes {a}-{b}/{size}"))
                .header(CONTENT_LENGTH, (b - a + 1).to_string());
        } else {
            builder = builder.header(CONTENT_LENGTH, size.to_string());
        }
        body::empty()
    } else if let Some(enc) = want_enc {
        builder = builder.header(CONTENT_ENCODING, enc.header_value());
        match open_compressed(&path, enc).await {
            Ok(b) => b,
            Err(e) => return io_error(e),
        }
    } else if let Some((a, b)) = range {
        let len = b - a + 1;
        builder = builder
            .status(StatusCode::PARTIAL_CONTENT)
            .header(CONTENT_RANGE, format!("bytes {a}-{b}/{size}"))
            .header(CONTENT_LENGTH, len.to_string());
        match open_range(&path, a, len).await {
            Ok(b) => b,
            Err(e) => return io_error(e),
        }
    } else {
        builder = builder.header(CONTENT_LENGTH, size.to_string());
        match tokio::fs::File::open(&path).await {
            Ok(f) => body::stream(f),
            Err(e) => return io_error(e),
        }
    };

    builder.body(body).unwrap_or_else(|_| {
        error_html(
            StatusCode::INTERNAL_SERVER_ERROR,
            "500 Internal Server Error",
            "Cannot build the response.",
        )
    })
}

async fn open_compressed(path: &Path, enc: Encoding) -> std::io::Result<ResBody> {
    let file = tokio::fs::File::open(path).await?;
    let reader = tokio::io::BufReader::new(file);
    Ok(match enc {
        Encoding::Brotli => body::stream(async_compression::tokio::bufread::BrotliEncoder::new(
            reader,
        )),
        Encoding::Gzip => body::stream(async_compression::tokio::bufread::GzipEncoder::new(reader)),
    })
}

async fn open_range(path: &Path, start: u64, len: u64) -> std::io::Result<ResBody> {
    use std::io::SeekFrom;
    let mut file = tokio::fs::File::open(path).await?;
    file.seek(SeekFrom::Start(start)).await?;
    Ok(body::stream(file.take(len)))
}

fn io_error(e: std::io::Error) -> Response<ResBody> {
    let (status, title) = if e.kind() == std::io::ErrorKind::PermissionDenied {
        (StatusCode::FORBIDDEN, "403 Forbidden")
    } else {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "500 Internal Server Error",
        )
    };
    error_html(status, title, &e.to_string())
}

// --------------------------------------------------------------------------
// helpers
// --------------------------------------------------------------------------

fn make_etag(size: u64, modified: SystemTime) -> String {
    let secs = modified
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("W/\"{size:x}-{secs:x}\"")
}

fn cond_not_modified(headers: &HeaderMap, etag: &str, modified: SystemTime) -> bool {
    if let Some(inm) = headers.get(IF_NONE_MATCH).and_then(|v| v.to_str().ok()) {
        let inm = inm.trim();
        if inm == "*" {
            return true;
        }
        let want = etag.trim_start_matches("W/");
        for tag in inm.split(',') {
            if tag.trim().trim_start_matches("W/") == want {
                return true;
            }
        }
        return false;
    }
    if let Some(ims) = headers.get(IF_MODIFIED_SINCE).and_then(|v| v.to_str().ok()) {
        if let Ok(t) = httpdate::parse_http_date(ims.trim()) {
            let m = modified
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            let i = t
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            return m <= i;
        }
    }
    false
}

enum RangeResult {
    None,
    Satisfiable(u64, u64),
    Unsatisfiable,
}

fn parse_range(headers: &HeaderMap, size: u64) -> RangeResult {
    let Some(raw) = headers.get(RANGE).and_then(|v| v.to_str().ok()) else {
        return RangeResult::None;
    };
    let Some(spec) = raw.trim().strip_prefix("bytes=") else {
        return RangeResult::None;
    };
    let Some(first) = spec.split(',').next() else {
        return RangeResult::None;
    };
    let Some((a, b)) = first.trim().split_once('-') else {
        return RangeResult::None;
    };
    let (a, b) = (a.trim(), b.trim());

    if size == 0 {
        return RangeResult::Unsatisfiable;
    }

    if a.is_empty() {
        let Ok(n) = b.parse::<u64>() else {
            return RangeResult::None;
        };
        if n == 0 {
            return RangeResult::Unsatisfiable;
        }
        return RangeResult::Satisfiable(size.saturating_sub(n), size - 1);
    }

    let Ok(start) = a.parse::<u64>() else {
        return RangeResult::None;
    };
    if start >= size {
        return RangeResult::Unsatisfiable;
    }
    let end = if b.is_empty() {
        size - 1
    } else {
        match b.parse::<u64>() {
            Ok(e) => e.min(size - 1),
            Err(_) => return RangeResult::None,
        }
    };
    if end < start {
        return RangeResult::Unsatisfiable;
    }
    RangeResult::Satisfiable(start, end)
}

fn mime_header(mime: &str) -> String {
    let needs_charset = mime.starts_with("text/")
        || mime == "application/javascript"
        || mime == "application/json"
        || mime == "application/xml"
        || mime == "image/svg+xml";
    if needs_charset {
        format!("{mime}; charset=utf-8")
    } else {
        mime.to_string()
    }
}

pub fn guess_mime(path: &Path) -> String {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match ext.as_str() {
        "mjs" | "cjs" => return "application/javascript".into(),
        "map" => return "application/json".into(),
        "webmanifest" => return "application/manifest+json".into(),
        "wasm" => return "application/wasm".into(),
        "avif" => return "image/avif".into(),
        "webp" => return "image/webp".into(),
        "woff2" => return "font/woff2".into(),
        "woff" => return "font/woff".into(),
        "ttf" => return "font/ttf".into(),
        "otf" => return "font/otf".into(),
        "md" => return "text/markdown".into(),
        _ => {}
    }
    mime_guess::from_path(path)
        .first_or_octet_stream()
        .essence_str()
        .to_string()
}

fn cache_control(seconds: i64) -> HeaderValue {
    let s = if seconds < 0 {
        "no-store, no-cache, must-revalidate".to_string()
    } else if seconds == 0 {
        "no-cache, no-store, must-revalidate".to_string()
    } else {
        format!("public, max-age={seconds}")
    };
    HeaderValue::from_str(&s).unwrap_or(HeaderValue::from_static("no-cache"))
}
