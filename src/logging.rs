//! Request logging in several formats.

use std::time::Duration;

use chrono::Local;
use hyper::header::HeaderMap;
use hyper::StatusCode;

use crate::cli::LogFormat;
use crate::config::LogOptions;
use crate::style::Style;

/// Everything we know about a finished request.
pub struct Record<'a> {
    pub method: &'a str,
    pub path: &'a str,
    pub query: Option<&'a str>,
    pub version: &'a str,
    pub status: StatusCode,
    pub length: Option<u64>,
    pub elapsed: Duration,
    pub client_ip: &'a str,
    pub user_agent: Option<&'a str>,
    pub referer: Option<&'a str>,
}

pub struct Logger {
    opts: LogOptions,
    style: Style,
}

impl Logger {
    pub fn new(mut opts: LogOptions) -> Self {
        let style = Style::new(opts.color);
        if matches!(opts.format, LogFormat::Json | LogFormat::Combined) {
            // Machine readable formats must never contain escape codes.
            opts.color = false;
        }
        Logger { opts, style }
    }

    pub fn enabled(&self) -> bool {
        self.opts.enabled
    }

    pub fn color_enabled(&self) -> bool {
        self.opts.color
    }

    pub fn log(&self, r: &Record<'_>) {
        if !self.opts.enabled {
            return;
        }
        let line = match self.opts.format {
            LogFormat::Pretty => self.pretty(r),
            LogFormat::Short => self.short(r),
            LogFormat::Combined => self.combined(r),
            LogFormat::Json => self.json(r),
        };
        println!("{line}");
    }

    fn target(r: &Record<'_>) -> String {
        match r.query {
            Some(q) if !q.is_empty() => format!("{}?{q}", r.path),
            _ => r.path.to_string(),
        }
    }

    fn size_str(r: &Record<'_>) -> String {
        match r.length {
            Some(n) => human_bytes(n),
            None => "-".to_string(),
        }
    }

    fn dur_str(d: Duration) -> String {
        let ms = d.as_secs_f64() * 1000.0;
        if ms < 1.0 {
            format!("{:.0}µs", d.as_micros())
        } else if ms < 1000.0 {
            format!("{ms:.1}ms")
        } else {
            format!("{:.2}s", d.as_secs_f64())
        }
    }

    fn pretty(&self, r: &Record<'_>) -> String {
        let time = Local::now().format("%H:%M:%S");
        let s = &self.style;
        let mut out = format!(
            "{} {} {} {} {} {}",
            s.dim(&time.to_string()),
            format_args!("{:>15}", r.client_ip),
            s.method(r.method),
            s.status(r.status.as_u16(), r.status.canonical_reason().unwrap_or("")),
            s.dim(&format!("{:>9}", Self::size_str(r))),
            s.dim(&format!("{:>8}", Self::dur_str(r.elapsed))),
        );
        out.push(' ');
        out.push_str(&Self::target(r));
        if let Some(ua) = r.user_agent {
            out.push(' ');
            out.push_str(&s.dim(&compress_ua(ua)));
        }
        out
    }

    fn short(&self, r: &Record<'_>) -> String {
        format!("{} {} {}", r.method, Self::target(r), r.status.as_u16())
    }

    fn combined(&self, r: &Record<'_>) -> String {
        let now = Local::now().format("%d/%b/%Y:%H:%M:%S %z");
        format!(
            "{} - - [{}] \"{} {} HTTP/{}\" {} {} \"{}\" \"{}\"",
            r.client_ip,
            now,
            r.method,
            Self::target(r),
            r.version,
            r.status.as_u16(),
            r.length
                .map(|n| n.to_string())
                .unwrap_or_else(|| "-".into()),
            r.referer.unwrap_or("-"),
            r.user_agent.unwrap_or("-"),
        )
    }

    fn json(&self, r: &Record<'_>) -> String {
        let now = Local::now().to_rfc3339();
        let target = Self::target(r);
        let mut o = String::from("{");
        push_kv(&mut o, "time", &now);
        push_kv(&mut o, "ip", r.client_ip);
        push_kv(&mut o, "method", r.method);
        push_kv(&mut o, "path", r.path);
        if let Some(q) = r.query {
            push_kv(&mut o, "query", q);
        }
        push_kv(&mut o, "http", r.version);
        o.push_str(&format!(",\"status\":{}", r.status.as_u16()));
        o.push_str(&format!(
            ",\"length\":{}",
            r.length
                .map(|n| n.to_string())
                .unwrap_or_else(|| "null".into())
        ));
        o.push_str(&format!(
            ",\"duration_ms\":{:.3}",
            r.elapsed.as_secs_f64() * 1000.0
        ));
        push_kv(&mut o, "ua", r.user_agent.unwrap_or(""));
        if let Some(rf) = r.referer {
            push_kv(&mut o, "referer", rf);
        }
        push_kv(&mut o, "url", &target);
        o.push('}');
        o
    }
}

fn push_kv(out: &mut String, key: &str, value: &str) {
    if out.len() > 1 {
        out.push(',');
    }
    out.push('"');
    out.push_str(key);
    out.push_str("\":\"");
    out.push_str(&json_escape(value));
    out.push('"');
}

fn json_escape(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)),
            c => o.push(c),
        }
    }
    o
}

/// Shorten the server name in a User-Agent so the log stays readable.
fn compress_ua(ua: &str) -> String {
    let head = ua.split_whitespace().next().unwrap_or(ua);
    if head.is_empty() {
        return ua.chars().take(40).collect();
    }
    if ua.len() <= 48 {
        ua.to_string()
    } else if head.len() <= 48 {
        head.to_string()
    } else {
        head.chars().take(45).collect::<String>() + "..."
    }
}

/// Human readable byte size.
pub fn human_bytes(n: u64) -> String {
    const UNITS: [&str; 5] = ["B", "kB", "MB", "GB", "TB"];
    let mut v = n as f64;
    let mut i = 0;
    while v >= 1024.0 && i < UNITS.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    if i == 0 {
        format!("{n} B")
    } else {
        format!("{v:.1} {}", UNITS[i])
    }
}

/// Pull the client IP from the socket / `X-Forwarded-For`.
pub fn client_ip(headers: &HeaderMap, peer_ip: &str, trust_proxy: bool) -> String {
    if trust_proxy {
        if let Some(fwd) = headers
            .get("x-forwarded-for")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.split(',').next())
            .map(|v| v.trim())
        {
            if !fwd.is_empty() {
                return fwd.to_string();
            }
        }
        if let Some(real) = headers.get("x-real-ip").and_then(|v| v.to_str().ok()) {
            return real.trim().to_string();
        }
    }
    peer_ip.to_string()
}
