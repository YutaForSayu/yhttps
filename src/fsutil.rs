//! Filesystem / URL path helpers.

use std::path::{Path, PathBuf};

use percent_encoding::{percent_decode_str, utf8_percent_encode, AsciiSet, CONTROLS};

/// Characters that must be escaped inside a single URL path segment.
const SEGMENT: &AsciiSet = &CONTROLS
    .add(b' ')
    .add(b'"')
    .add(b'#')
    .add(b'<')
    .add(b'>')
    .add(b'?')
    .add(b'`')
    .add(b'{')
    .add(b'}')
    .add(b'%')
    .add(b'\\')
    .add(b'^')
    .add(b'|')
    .add(b'[')
    .add(b']')
    .add(b'\'');

/// Percent-encode a single path segment for use inside an href.
pub fn url_encode_segment(s: &str) -> String {
    utf8_percent_encode(s, SEGMENT).to_string()
}

pub fn html_escape(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            '\'' => o.push_str("&#39;"),
            c => o.push(c),
        }
    }
    o
}

/// Decode a URL path and split it into safe path segments.
///
/// Returns `Err(())` when the URL contains something that must never be used
/// to build a filesystem path (directory traversal, NUL, Windows drive / ADS).
pub fn safe_segments(url_path: &str) -> Result<Vec<String>, ()> {
    let decoded = percent_decode_str(url_path).decode_utf8().map_err(|_| ())?;
    let mut out = Vec::new();
    for raw in decoded.split('/') {
        if raw.is_empty() || raw == "." {
            continue;
        }
        if raw.contains('\0') || raw.contains('\\') {
            return Err(());
        }
        if raw.len() >= 2 && raw.as_bytes()[1] == b':' {
            // `C:` style drive letters / NTFS alternate data streams.
            return Err(());
        }
        for ch in raw.chars() {
            if (ch as u32) < 0x20 {
                return Err(());
            }
        }
        if raw == ".." {
            return Err(());
        }
        out.push(raw.to_string());
    }
    Ok(out)
}

/// Join segments onto `root`.
pub fn join_segments(root: &Path, segments: &[String]) -> PathBuf {
    let mut p = root.to_path_buf();
    for s in segments {
        p.push(s);
    }
    p
}

/// Append an extension to a path that has none (`/about` -> `/about.html`).
pub fn append_ext(path: &Path, ext: &str) -> PathBuf {
    let mut s = path.as_os_str().to_os_string();
    s.push(".");
    s.push(ext);
    PathBuf::from(s)
}

/// True when the final path component has a file extension.
///
/// Follows Rust's own semantics: a leading-dot name (`.hidden`) has none.
pub fn has_extension(path: &Path) -> bool {
    path.extension().is_some()
}

/// Directory listing sort key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortKey {
    Name,
    Size,
    Time,
}

impl SortKey {
    pub fn parse(s: &str) -> SortKey {
        match s {
            "size" => SortKey::Size,
            "time" | "mtime" | "date" => SortKey::Time,
            _ => SortKey::Name,
        }
    }
}
