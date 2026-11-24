//! Resolved, validated runtime configuration.

use std::io::IsTerminal;
use std::path::PathBuf;

use anyhow::{bail, Context, Result};

use crate::cli::{Cli, Cmd, LogFormat, ServeArgs};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// `yhtps s` — plain static serving.
    Static,
    /// `yhtps sf` — framework build (SPA fallback + clean URLs).
    Framework,
}

impl Mode {
    pub fn label(&self) -> &'static str {
        match self {
            Mode::Static => "static",
            Mode::Framework => "framework (SPA)",
        }
    }
}

#[derive(Debug, Clone)]
#[cfg_attr(not(feature = "tls"), allow(dead_code))]
pub struct SslOptions {
    pub cert: Option<PathBuf>,
    pub key: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct LogOptions {
    pub enabled: bool,
    pub format: LogFormat,
    pub color: bool,
    pub trust_proxy: bool,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub mode: Mode,

    /// Directory that requests are resolved against.
    pub root: PathBuf,
    /// Set when the user pointed at a single file.
    pub single_file: Option<PathBuf>,
    /// Value shown in the startup banner.
    pub display_path: String,

    pub host: String,
    pub port: u16,

    pub index_files: Vec<String>,
    pub listing: bool,
    pub cache_seconds: i64,
    pub cors: Option<String>,
    pub gzip: bool,
    pub brotli: bool,
    pub compress_threshold: u64,
    pub open_browser: bool,
    pub spa: bool,
    pub clean_urls: bool,
    pub fallback: String,
    pub proxy: Option<String>,
    pub proxy_prefix: Option<String>,
    pub auth: Option<(String, String)>,
    pub ssl: Option<SslOptions>,
    pub log: LogOptions,
    pub headers: Vec<(String, String)>,
    pub range: bool,
    pub timeout: Option<u64>,
    pub extensions: Vec<String>,
    pub server_header: bool,
    pub silent: bool,
}

impl Config {
    pub fn from_cli(cli: Cli) -> Result<Config> {
        let (args, mode) = match cli.command {
            Cmd::S(a) => (a, Mode::Static),
            Cmd::Sf(a) => (a, Mode::Framework),
        };
        Self::build(args, mode)
    }

    fn build(args: ServeArgs, mode: Mode) -> Result<Config> {
        // ---- resolve the served path -------------------------------------
        let mut path = args.path.clone();
        if path.as_os_str().is_empty() {
            path = PathBuf::from(".");
        }
        let abs = if path.is_absolute() {
            path.clone()
        } else {
            std::env::current_dir()
                .context("cannot read the current directory")?
                .join(&path)
        };
        let abs = absolute_lexical(&abs);

        let meta = std::fs::metadata(&abs)
            .with_context(|| format!("cannot access '{}' — does the path exist?", abs.display()))?;

        let (root, single_file) = if meta.is_dir() {
            (abs.clone(), None)
        } else {
            let parent = abs
                .parent()
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|| PathBuf::from("."));
            (parent, Some(abs.clone()))
        };

        let root = root.canonicalize().unwrap_or(root);

        // ---- index files -------------------------------------------------
        let mut index_files = args
            .index
            .clone()
            .unwrap_or_else(|| vec!["index.html".to_string(), "index.htm".to_string()]);
        if let Some(single) = &single_file {
            if let Some(name) = single.file_name().and_then(|n| n.to_str()) {
                if !index_files.iter().any(|i| i == name) {
                    index_files.insert(0, name.to_string());
                }
            }
        }

        // ---- framework niceties -----------------------------------------
        let mut extensions: Vec<String> = args
            .extensions
            .clone()
            .unwrap_or_default()
            .into_iter()
            .map(|e| e.trim_start_matches('.').to_string())
            .filter(|e| !e.is_empty())
            .collect();
        let mut clean_urls = false;
        let mut spa = args.spa;
        if mode == Mode::Framework {
            clean_urls = true;
            spa = true;
        }
        if !extensions.is_empty() {
            clean_urls = true;
        }
        if clean_urls && extensions.is_empty() {
            extensions = vec!["html".to_string()];
        }

        // ---- CORS --------------------------------------------------------
        let cors = if let Some(origin) = args.cors_origin.clone() {
            Some(origin)
        } else if args.cors {
            Some("*".to_string())
        } else {
            None
        };

        // ---- basic auth --------------------------------------------------
        let auth = match (&args.username, &args.password) {
            (Some(u), Some(p)) => Some((u.clone(), p.clone())),
            (Some(u), None) => Some((u.clone(), String::new())),
            (None, Some(_)) => bail!("--password requires --username"),
            (None, None) => None,
        };

        // ---- TLS ---------------------------------------------------------
        let ssl = if args.ssl || args.cert.is_some() || args.key.is_some() {
            if args.cert.is_some() ^ args.key.is_some() {
                bail!("--cert and --key must be given together");
            }
            Some(SslOptions {
                cert: args.cert.clone(),
                key: args.key.clone(),
            })
        } else {
            None
        };

        // ---- extra headers ----------------------------------------------
        let mut headers = Vec::new();
        for raw in &args.headers {
            let (name, value) = raw
                .split_once(':')
                .with_context(|| format!("--header '{raw}' must look like 'Name: Value'"))?;
            headers.push((name.trim().to_string(), value.trim().to_string()));
        }

        // ---- logging -----------------------------------------------------
        let color = !args.no_color
            && std::env::var_os("NO_COLOR").is_none()
            && std::io::stdout().is_terminal();
        let log = LogOptions {
            enabled: !(args.clean || args.silent),
            format: args.log_format,
            color,
            trust_proxy: args.trust_proxy,
        };

        let display_path = match &single_file {
            Some(f) => f.display().to_string(),
            None => root.display().to_string(),
        };

        Ok(Config {
            mode,
            root,
            single_file,
            display_path,
            host: args.address.clone(),
            port: args.port,
            index_files,
            listing: !args.no_listing,
            cache_seconds: args.cache,
            cors,
            gzip: args.gzip,
            brotli: args.brotli,
            compress_threshold: args.compress_threshold,
            open_browser: args.open,
            spa,
            clean_urls,
            fallback: args.fallback.clone(),
            proxy: args.proxy.clone(),
            proxy_prefix: args
                .proxy_prefix
                .clone()
                .map(|p| {
                    if p.starts_with('/') {
                        p
                    } else {
                        format!("/{p}")
                    }
                })
                .filter(|_| args.proxy.is_some()),
            auth,
            ssl,
            log,
            headers,
            range: !args.no_range,
            timeout: args.timeout,
            extensions,
            server_header: true,
            silent: args.silent,
        })
    }

    pub fn compression_enabled(&self) -> bool {
        self.gzip || self.brotli
    }
}

/// Lexically normalise a path (no filesystem access, so it works for
/// not-yet-existing paths and on every platform).
fn absolute_lexical(p: &std::path::Path) -> PathBuf {
    use std::path::Component;
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => {
                if !out.pop() {
                    out.push("..");
                }
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}
