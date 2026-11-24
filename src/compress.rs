//! Content-Encoding negotiation and compressibility heuristics.

use hyper::header::{HeaderMap, ACCEPT_ENCODING};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Encoding {
    Brotli,
    Gzip,
}

impl Encoding {
    pub fn header_value(&self) -> &'static str {
        match self {
            Encoding::Brotli => "br",
            Encoding::Gzip => "gzip",
        }
    }
}

/// Choose an encoding the client accepts, preferring brotli.
pub fn negotiate(headers: &HeaderMap, gzip: bool, brotli: bool) -> Option<Encoding> {
    if !gzip && !brotli {
        return None;
    }
    let accept = headers.get(ACCEPT_ENCODING)?.to_str().ok()?;
    let mut allows_br = false;
    let mut allows_gzip = false;
    let mut br_q0 = false;
    let mut gz_q0 = false;

    for part in accept.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let mut bits = part.split(';');
        let name = bits.next().unwrap_or("").trim().to_ascii_lowercase();
        let mut q: f32 = 1.0;
        for p in bits {
            let p = p.trim();
            if let Some(v) = p.strip_prefix("q=") {
                q = v.trim().parse().unwrap_or(1.0);
            }
        }
        match name.as_str() {
            "br" => {
                allows_br = true;
                if q == 0.0 {
                    br_q0 = true;
                }
            }
            "gzip" | "x-gzip" => {
                allows_gzip = true;
                if q == 0.0 {
                    gz_q0 = true;
                }
            }
            "*" if q > 0.0 => {
                allows_br = true;
                allows_gzip = true;
            }
            _ => {}
        }
    }

    if brotli && allows_br && !br_q0 {
        return Some(Encoding::Brotli);
    }
    if gzip && allows_gzip && !gz_q0 {
        return Some(Encoding::Gzip);
    }
    None
}

/// Does it make sense to compress this file?
pub fn is_compressible(mime: &str, path: &Path) -> bool {
    // Already-compressed formats: never worth re-compressing.
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    const SKIP_EXT: &[&str] = &[
        "gz", "br", "zip", "7z", "rar", "tar", "tgz", "bz2", "xz", "zst", "png", "jpg", "jpeg",
        "gif", "webp", "avif", "ico", "mp3", "mp4", "m4a", "m4v", "mov", "avi", "webm", "ogg",
        "ogv", "opus", "flac", "woff", "woff2", "eot", "ttf", "otf", "pdf", "wasm", "exe", "dll",
        "so", "dylib", "bin", "iso", "img", "apk", "jar", "class", "db", "sqlite",
    ];
    if SKIP_EXT.contains(&ext.as_str()) {
        return false;
    }

    if mime.starts_with("text/") {
        return true;
    }
    matches!(
        mime,
        "application/json"
            | "application/ld+json"
            | "application/manifest+json"
            | "application/javascript"
            | "application/x-javascript"
            | "application/ecmascript"
            | "application/xml"
            | "application/xhtml+xml"
            | "application/rss+xml"
            | "application/atom+xml"
            | "application/wasm"
            | "image/svg+xml"
            | "application/x-httpd-php"
            | "application/vnd.api+json"
    ) || mime.ends_with("+json")
        || mime.ends_with("+xml")
}
