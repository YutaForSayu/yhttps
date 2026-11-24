//! HTML directory listings (the `http-server -d` / `serve` equivalent).

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use hyper::{Response, StatusCode};

use crate::body::{self, ResBody};
use crate::fsutil::{html_escape, url_encode_segment, SortKey};
use crate::logging::human_bytes;

const PAGE_CSS: &str = r#"
:root{color-scheme:dark light}
*{box-sizing:border-box}
body{margin:0;padding:2.5rem 1.5rem;font:15px/1.5 ui-sans-serif,system-ui,-apple-system,"Segoe UI",Roboto,sans-serif;
  background:#0f1115;color:#e6e8ee}
.wrap{max-width:900px;margin:0 auto}
h1{font-size:1.15rem;font-weight:600;margin:0 0 .35rem}
.sub{color:#8b93a7;font-size:.85rem;margin-bottom:1.25rem;word-break:break-all}
table{width:100%;border-collapse:collapse}
th,td{text-align:left;padding:.5rem .65rem;border-bottom:1px solid #232733}
th{font-size:.72rem;letter-spacing:.06em;text-transform:uppercase;color:#8b93a7;font-weight:600}
th a{color:inherit;text-decoration:none}
th a:hover{color:#e6e8ee}
td.size,th.size{text-align:right;font-variant-numeric:tabular-nums;white-space:nowrap}
td.time,th.time{color:#8b93a7;white-space:nowrap;font-size:.85rem}
a{color:#7cc4ff;text-decoration:none}
a:hover{text-decoration:underline}
.icon{display:inline-block;width:1.35rem;color:#8b93a7}
.dir .name{font-weight:600}
.up{color:#8b93a7}
footer{margin-top:1.5rem;color:#5c6480;font-size:.8rem}
"#;

struct Entry {
    name: String,
    is_dir: bool,
    size: u64,
    modified: Option<SystemTime>,
}

pub async fn respond(
    dir: &Path,
    url_path: &str,
    query: Option<&str>,
    listing_enabled: bool,
) -> Response<ResBody> {
    if !listing_enabled {
        return error_html(
            StatusCode::FORBIDDEN,
            "403 Forbidden",
            "Directory listing is disabled.",
        );
    }

    let (sort_key, asc) = parse_sort(query);

    let mut entries: Vec<Entry> = Vec::new();
    let mut rd = match tokio::fs::read_dir(dir).await {
        Ok(rd) => rd,
        Err(e) => {
            return error_html(
                StatusCode::INTERNAL_SERVER_ERROR,
                "500 Internal Server Error",
                &format!("cannot read directory: {e}"),
            )
        }
    };
    while let Ok(Some(entry)) = rd.next_entry().await {
        let name = entry.file_name().to_string_lossy().to_string();
        let meta = match entry.metadata().await {
            Ok(m) => m,
            Err(_) => continue,
        };
        entries.push(Entry {
            name,
            is_dir: meta.is_dir(),
            size: meta.len(),
            modified: meta.modified().ok(),
        });
    }

    entries.sort_by(|a, b| {
        // Directories always float to the top.
        if a.is_dir != b.is_dir {
            return b.is_dir.cmp(&a.is_dir);
        }
        let ord = match sort_key {
            SortKey::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
            SortKey::Size => a.size.cmp(&b.size),
            SortKey::Time => a.modified.cmp(&b.modified),
        };
        if asc {
            ord
        } else {
            ord.reverse()
        }
    });

    let base = if url_path.ends_with('/') {
        url_path.to_string()
    } else {
        format!("{url_path}/")
    };

    let mut html = String::with_capacity(4096 + entries.len() * 160);
    html.push_str("<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\">");
    html.push_str("<meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">");
    html.push_str("<title>Index of ");
    html.push_str(&html_escape(url_path));
    html.push_str("</title><style>");
    html.push_str(PAGE_CSS);
    html.push_str("</style></head><body><div class=\"wrap\">");

    html.push_str("<h1>Index of ");
    html.push_str(&html_escape(&base));
    html.push_str("</h1>");
    html.push_str("<div class=\"sub\">");
    html.push_str(&html_escape(&dir.display().to_string()));
    html.push_str("</div>");

    html.push_str("<table><thead><tr>");
    html.push_str(&th_link("Name", "name", &base, sort_key, asc));
    html.push_str(&th_link("Size", "size", &base, sort_key, asc));
    html.push_str(&th_link("Last modified", "time", &base, sort_key, asc));
    html.push_str("</tr></thead><tbody>");

    if url_path != "/" && !url_path.is_empty() {
        let parent = parent_url(&base);
        html.push_str("<tr class=\"up\"><td colspan=\"3\"><span class=\"icon\">⬆</span>");
        html.push_str("<a href=\"");
        html.push_str(&html_escape(&parent));
        html.push_str("\">..</a></td></tr>");
    }

    for e in &entries {
        let href = format!(
            "{base}{}{}",
            url_encode_segment(&e.name),
            if e.is_dir { "/" } else { "" }
        );
        let (icon, class) = if e.is_dir {
            ("📁", " class=\"dir\"")
        } else {
            (icon_for(&e.name), "")
        };
        html.push_str("<tr");
        html.push_str(class);
        html.push_str("><td><span class=\"icon\">");
        html.push_str(icon);
        html.push_str("</span><a href=\"");
        html.push_str(&html_escape(&href));
        html.push_str("\">");
        html.push_str(&html_escape(&e.name));
        html.push_str(if e.is_dir { "/" } else { "" });
        html.push_str("</a></td><td class=\"size\">");
        if e.is_dir {
            html.push('—');
        } else {
            html.push_str(&human_bytes(e.size));
        }
        html.push_str("</td><td class=\"time\">");
        html.push_str(&fmt_time(e.modified));
        html.push_str("</td></tr>");
    }

    html.push_str("</tbody></table>");
    html.push_str("<footer>YhHTTPServer · ");
    html.push_str(&entries.len().to_string());
    html.push_str(" entries</footer></div></body></html>");

    let mut resp = Response::new(body::full(html));
    *resp.status_mut() = StatusCode::OK;
    resp.headers_mut().insert(
        hyper::header::CONTENT_TYPE,
        hyper::header::HeaderValue::from_static("text/html; charset=utf-8"),
    );
    resp
}

fn parse_sort(query: Option<&str>) -> (SortKey, bool) {
    let mut key = SortKey::Name;
    let mut asc = true;
    if let Some(q) = query {
        for pair in q.split('&') {
            let mut it = pair.splitn(2, '=');
            let k = it.next().unwrap_or("");
            let v = it.next().unwrap_or("");
            match k {
                "sort" | "C" => key = SortKey::parse(v),
                "order" | "O" => asc = v.eq_ignore_ascii_case("asc"),
                _ => {}
            }
        }
    }
    (key, asc)
}

fn th_link(label: &str, key: &str, base: &str, cur: SortKey, asc: bool) -> String {
    let is_cur = cur == SortKey::parse(key);
    let next_order = if is_cur && asc { "desc" } else { "asc" };
    let arrow = if is_cur {
        if asc {
            " ▲"
        } else {
            " ▼"
        }
    } else {
        ""
    };
    let class = if key == "size" {
        " class=\"size\""
    } else if key == "time" {
        " class=\"time\""
    } else {
        ""
    };
    format!(
        "<th{class}><a href=\"{}?sort={key}&order={next_order}\">{label}{arrow}</a></th>",
        html_escape(base)
    )
}

fn parent_url(base: &str) -> String {
    // base always ends with '/'
    let trimmed = base.trim_end_matches('/');
    match trimmed.rfind('/') {
        Some(0) | None => "/".to_string(),
        Some(i) => format!("{}/", &trimmed[..i]),
    }
}

fn fmt_time(t: Option<SystemTime>) -> String {
    match t {
        Some(t) => {
            let dt: chrono::DateTime<chrono::Local> = t.into();
            dt.format("%Y-%m-%d %H:%M").to_string()
        }
        None => "—".to_string(),
    }
}

fn icon_for(name: &str) -> &'static str {
    let ext = PathBuf::from(name)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match ext.as_str() {
        "html" | "htm" => "🌐",
        "css" => "🎨",
        "js" | "mjs" | "cjs" | "ts" => "📜",
        "json" | "map" => "🧾",
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "avif" | "svg" | "ico" => "🖼",
        "mp4" | "webm" | "mov" | "mkv" => "🎬",
        "mp3" | "wav" | "ogg" | "flac" => "🎵",
        "zip" | "gz" | "tar" | "7z" | "rar" => "🗜",
        "md" | "txt" | "log" => "📄",
        "woff" | "woff2" | "ttf" | "otf" => "🔤",
        "wasm" => "🧩",
        "pdf" => "📕",
        _ => "📄",
    }
}

pub fn error_html(status: StatusCode, title: &str, detail: &str) -> Response<ResBody> {
    let html = format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\">\
<meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">\
<title>{title}</title><style>{css}</style></head><body><div class=\"wrap\">\
<h1>{title_esc}</h1><p class=\"sub\">{detail}</p>\
<footer><a href=\"/\">← back to index</a> &nbsp;·&nbsp; YhHTTPServer</footer>\
</div></body></html>",
        title = html_escape(title),
        title_esc = html_escape(title),
        detail = html_escape(detail),
        css = r#":root{color-scheme:dark light}
body{margin:0;padding:3rem 1.5rem;font:15px/1.6 ui-sans-serif,system-ui,-apple-system,"Segoe UI",sans-serif;background:#0f1115;color:#e6e8ee}
.wrap{max-width:640px;margin:0 auto}
h1{font-size:1.4rem;margin:0 0 .5rem}
.sub{color:#8b93a7;word-break:break-word}
footer{margin-top:2rem;color:#5c6480;font-size:.85rem}
a{color:#7cc4ff;text-decoration:none}a:hover{text-decoration:underline}"#
    );
    let mut resp = Response::new(body::full(html));
    *resp.status_mut() = status;
    resp.headers_mut().insert(
        hyper::header::CONTENT_TYPE,
        hyper::header::HeaderValue::from_static("text/html; charset=utf-8"),
    );
    resp
}
