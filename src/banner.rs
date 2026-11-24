//! Startup banner, URL discovery and "open in browser" support.

use std::net::{IpAddr, SocketAddr};
use std::process::{Command, Stdio};

use anyhow::Result;

use crate::state::State;
use crate::style::Style;

pub fn local_url(state: &State, addr: SocketAddr) -> String {
    let host = match state.cfg.host.as_str() {
        "0.0.0.0" | "::" | "[::]" | "" => "localhost",
        h => h,
    };
    format!("{}://{host}:{}", state.scheme(), addr.port())
}

pub fn print_startup(state: &State, addr: SocketAddr) -> Result<()> {
    if state.cfg.silent {
        return Ok(());
    }
    let s = Style::new(state.log.color_enabled());
    let port = addr.port();
    let scheme = state.scheme();

    let mut feat: Vec<String> = Vec::new();
    if state.cfg.gzip {
        feat.push("gzip".into());
    }
    if state.cfg.brotli {
        feat.push("brotli".into());
    }
    if state.cfg.cors.is_some() {
        feat.push("cors".into());
    }
    if state.cfg.spa {
        feat.push("spa".into());
    }
    if state.cfg.clean_urls && !state.cfg.spa {
        feat.push("clean-urls".into());
    }
    if state.cfg.auth.is_some() {
        feat.push("basic-auth".into());
    }
    if state.cfg.range {
        feat.push("ranges".into());
    }
    if !state.cfg.listing {
        feat.push("no-listing".into());
    }
    if state.cfg.proxy.is_some() {
        match &state.cfg.proxy_prefix {
            Some(p) => feat.push(format!(
                "proxy{}→{}",
                p,
                state.cfg.proxy.as_deref().unwrap_or("")
            )),
            None => feat.push(format!(
                "proxy(fallback)→{}",
                state.cfg.proxy.as_deref().unwrap_or("")
            )),
        }
    }
    if !state.cfg.log.enabled {
        feat.push("no-log".into());
    }

    let title = format!("YhHTTPServer v{}", env!("CARGO_PKG_VERSION"));
    let mut lines: Vec<String> = Vec::new();
    lines.push(format!(
        "{}  {}",
        s.dim("serving"),
        s.bold(&state.cfg.display_path)
    ));
    lines.push(format!("{}  {}", s.dim("mode   "), state.cfg.mode.label()));
    if let Some(single) = &state.cfg.single_file {
        lines.push(format!("{}  {}", s.dim("file   "), single.display()));
    }
    lines.push(format!(
        "{}  {}",
        s.dim("cache  "),
        if state.cfg.cache_seconds < 0 {
            "disabled".to_string()
        } else {
            format!("{}s", state.cfg.cache_seconds)
        }
    ));
    if let Some(note) = &state.tls_note {
        lines.push(format!("{}  {}", s.dim("tls    "), note));
    }
    if !feat.is_empty() {
        lines.push(format!("{}  {}", s.dim("flags  "), feat.join(", ")));
    }

    let urls = collect_urls(&state.cfg.host, port, scheme);

    let width = lines
        .iter()
        .chain(std::iter::once(&title))
        .map(|l| visible_len(l))
        .chain(urls.iter().map(|(_, u)| u.len() + 2))
        .max()
        .unwrap_or(60)
        .max(46);

    println!();
    println!("{}", s.cyan(&format!("╭{}╮", "─".repeat(width + 2))));
    println!(
        "{} {} {}",
        s.cyan("│"),
        pad(&s.bold(&title), &title, width),
        s.cyan("│")
    );
    println!("{}", s.cyan(&format!("├{}┤", "─".repeat(width + 2))));
    for l in &lines {
        println!(
            "{} {} {}",
            s.cyan("│"),
            pad(l, &strip_ansi(l), width),
            s.cyan("│")
        );
    }
    println!("{}", s.cyan(&format!("├{}┤", "─".repeat(width + 2))));
    for (label, url) in &urls {
        let text = format!("{label:<8} {url}");
        let colored = format!("{:<8} {}", s.dim(label), s.bold(url));
        println!(
            "{} {} {}",
            s.cyan("│"),
            pad(&colored, &text, width),
            s.cyan("│")
        );
    }
    println!("{}", s.cyan(&format!("╰{}╯", "─".repeat(width + 2))));
    if !state.cfg.log.enabled {
        println!("{}", s.dim("  clean mode — request logging is off"));
    }
    println!("{}", s.dim("  press Ctrl+C to stop"));
    println!();
    Ok(())
}

pub fn print_shutdown(state: &State) {
    if state.cfg.silent {
        return;
    }
    let s = Style::new(state.log.color_enabled());
    println!("\n{}", s.dim("yhtps: shut down cleanly. bye 👋"));
}

fn collect_urls(bind_host: &str, port: u16, scheme: &str) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    if bind_host == "0.0.0.0" || bind_host == "::" || bind_host == "[::]" || bind_host.is_empty() {
        out.push(("Local".into(), format!("{scheme}://localhost:{port}")));
        out.push(("Local".into(), format!("{scheme}://127.0.0.1:{port}")));
        for ip in network_ips() {
            out.push((
                "Network".into(),
                format!("{scheme}://{}:{port}", bracket(&ip)),
            ));
        }
    } else {
        out.push((
            "Local".into(),
            format!("{scheme}://{}:{port}", bracket_str(bind_host)),
        ));
    }
    out
}

/// IPv6 addresses must be bracketed before a `:port` may follow them.
fn bracket(ip: &IpAddr) -> String {
    match ip {
        IpAddr::V6(_) => format!("[{ip}]"),
        IpAddr::V4(_) => ip.to_string(),
    }
}

fn bracket_str(host: &str) -> String {
    if host.contains(':') && !host.starts_with('[') {
        format!("[{host}]")
    } else {
        host.to_string()
    }
}

fn network_ips() -> Vec<IpAddr> {
    let mut ips: Vec<IpAddr> = Vec::new();
    if let Ok(ifaces) = local_ip_address::list_afinet_netifas() {
        for (_name, ip) in ifaces {
            match ip {
                IpAddr::V4(v4) if !v4.is_loopback() && !v4.is_link_local() => ips.push(ip),
                IpAddr::V6(v6) if !v6.is_loopback() && (v6.segments()[0] & 0xffc0) != 0xfe80 => {
                    ips.push(ip)
                }
                _ => {}
            }
        }
    }
    ips.sort();
    ips.dedup();
    ips.truncate(6);
    ips
}

// ---- tiny helpers for the box drawing ------------------------------------

fn visible_len(s: &str) -> usize {
    strip_ansi(s).chars().count()
}

fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_esc = false;
    for c in s.chars() {
        if c == '\x1b' {
            in_esc = true;
            continue;
        }
        if in_esc {
            if c == 'm' {
                in_esc = false;
            }
            continue;
        }
        out.push(c);
    }
    out
}

fn pad(colored: &str, plain: &str, width: usize) -> String {
    let len = plain.chars().count();
    if len >= width {
        colored.to_string()
    } else {
        format!("{colored}{}", " ".repeat(width - len))
    }
}

/// Open a URL in the user's default browser, best effort.
pub fn open_in_browser(url: &str) {
    let candidates: Vec<(&str, Vec<&str>)> = {
        #[cfg(target_os = "macos")]
        {
            vec![("open", vec![url])]
        }
        #[cfg(target_os = "windows")]
        {
            vec![("cmd", vec!["/C", "start", "", url])]
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            vec![
                ("xdg-open", vec![url]),
                ("termux-open-url", vec![url]),
                ("termux-open", vec![url]),
                ("wslview", vec![url]),
            ]
        }
    };

    for (cmd, args) in candidates {
        let spawned = Command::new(cmd)
            .args(&args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
        if spawned.is_ok() {
            return;
        }
    }
}
