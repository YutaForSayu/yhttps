//! Command line interface for `yhtps` (YhHTTPServer).
//!
//! Two sub-commands mirror the two npm packages this tool replaces:
//!
//! * `s` — serve a file **or** a directory, statically (like `http-server`)
//! * `sf` — serve a framework **build folder** with SPA fallback + clean URLs,
//!   i.e. what `serve` does in front of a Vite / Next / SvelteKit `dist/`

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

/// Log output format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum LogFormat {
    /// Colourised, human friendly, one line per request (default)
    Pretty,
    /// Minimal: `METHOD PATH STATUS`
    Short,
    /// Apache/nginx "combined" access log format
    Combined,
    /// One JSON object per line (machine readable)
    Json,
}

const AFTER_HELP: &str = "\
EXAMPLES:
  yhtps s /var/www/html/index.html          serve a single static HTML file
  yhtps s /var/www/html                     serve a directory (with listing)
  yhtps sf /var/www/html/blog/dist          serve a Vite/Next build (SPA fallback)
  yhtps sf ./dist -p 8080 -o                custom port + open in browser
  yhtps s . --gzip --brotli --cors          compression + CORS
  yhtps s ./public --clean                  serve quietly (no request logs)
  yhtps s ./public -S                       HTTPS with a self-signed certificate
  yhtps sf ./dist --proxy http://127.0.0.1:3000
                                            proxy API calls to a dev backend
";

#[derive(Parser, Debug)]
#[command(
    name = "yhtps",
    bin_name = "yhtps",
    version,
    about = "YhHTTPServer — zero-config static & framework HTTP server (http-server + serve, in Rust)",
    long_about = "YhHTTPServer (`yhtps`) serves static sites and built framework output \
                  (Vite, Next.js export, SvelteKit, Astro, plain HTML…) straight from the \
                  filesystem. It is a single, dependency-free binary that runs on Linux, \
                  macOS, Windows and Android/Termux.",
    after_help = AFTER_HELP,
    disable_help_subcommand = true,
    arg_required_else_help = true
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Cmd,
}

#[derive(Subcommand, Debug)]
pub enum Cmd {
    /// Serve a file or a directory (auto-detected)
    #[command(name = "s", visible_aliases = ["serve", "static"])]
    S(ServeArgs),

    /// Serve a built framework folder (SPA fallback + clean URLs)
    #[command(name = "sf", visible_aliases = ["serve-folder", "framework", "spa"])]
    Sf(ServeArgs),
}

#[derive(Args, Debug, Clone)]
pub struct ServeArgs {
    /// File or directory to serve
    #[arg(value_name = "PATH")]
    pub path: PathBuf,

    /// Port to listen on
    #[arg(short = 'p', long, default_value_t = 5050, value_name = "PORT")]
    pub port: u16,

    /// Address to bind (use 0.0.0.0 for all interfaces)
    #[arg(short = 'a', long, default_value = "0.0.0.0", value_name = "ADDRESS")]
    pub address: String,

    /// Disable request logging (clean, quiet access log)
    #[arg(long)]
    pub clean: bool,

    /// Suppress *all* output (banner + logs)
    #[arg(long)]
    pub silent: bool,

    /// Cache-Control max-age in seconds; -1 disables caching entirely
    #[arg(
        short = 'c',
        long,
        default_value_t = 3600,
        value_name = "SECONDS",
        allow_hyphen_values = true
    )]
    pub cache: i64,

    /// Enable gzip compression
    #[arg(short = 'g', long)]
    pub gzip: bool,

    /// Enable brotli compression (preferred when the client supports it)
    #[arg(short = 'b', long)]
    pub brotli: bool,

    /// Minimum response size (bytes) worth compressing
    #[arg(long, default_value_t = 1024, value_name = "BYTES")]
    pub compress_threshold: u64,

    /// Send permissive CORS headers
    #[arg(long)]
    pub cors: bool,

    /// Custom `Access-Control-Allow-Origin` value (implies --cors)
    #[arg(long, value_name = "ORIGIN")]
    pub cors_origin: Option<String>,

    /// Open the served URL in the default browser
    #[arg(short = 'o', long)]
    pub open: bool,

    /// Single-page-app mode: unknown paths fall back to the fallback file
    #[arg(long)]
    pub spa: bool,

    /// File used as the SPA fallback (default: index.html)
    #[arg(long, default_value = "index.html", value_name = "FILE")]
    pub fallback: String,

    /// Disable directory listings
    #[arg(long)]
    pub no_listing: bool,

    /// Index file names to look for in directories
    #[arg(
        short = 'i',
        long = "index",
        value_name = "FILE",
        value_delimiter = ','
    )]
    pub index: Option<Vec<String>>,

    /// Extra extension to try for extension-less requests (e.g. `html`)
    #[arg(
        short = 'e',
        long = "extensions",
        value_name = "EXT",
        value_delimiter = ','
    )]
    pub extensions: Option<Vec<String>>,

    /// Proxy every request to this origin (bare `--proxy` = fallback for
    /// anything not on disk).
    #[arg(short = 'P', long, value_name = "URL")]
    pub proxy: Option<String>,

    /// Only proxy paths under this prefix (e.g. `/api`); everything else is
    /// served locally, so SPA routing keeps working.
    #[arg(long, value_name = "PATH")]
    pub proxy_prefix: Option<String>,

    /// HTTP Basic auth user name
    #[arg(long, value_name = "USER")]
    pub username: Option<String>,

    /// HTTP Basic auth password
    #[arg(long, value_name = "PASS")]
    pub password: Option<String>,

    /// Serve over HTTPS (generates a self-signed certificate when none is given)
    #[arg(short = 'S', long, visible_alias = "tls")]
    pub ssl: bool,

    /// TLS certificate chain (PEM)
    #[arg(short = 'C', long, value_name = "FILE")]
    pub cert: Option<PathBuf>,

    /// TLS private key (PEM)
    #[arg(short = 'K', long, value_name = "KEY")]
    pub key: Option<PathBuf>,

    /// Disable byte-range requests
    #[arg(long)]
    pub no_range: bool,

    /// Connection header read timeout, in seconds
    #[arg(long, value_name = "SECONDS")]
    pub timeout: Option<u64>,

    /// Extra response header; repeatable, format `Name: Value`
    #[arg(long = "header", value_name = "NAME: VALUE")]
    pub headers: Vec<String>,

    /// Log format
    #[arg(long, value_enum, default_value_t = LogFormat::Pretty)]
    pub log_format: LogFormat,

    /// Disable coloured output
    #[arg(long)]
    pub no_color: bool,

    /// Trust `X-Forwarded-For` when determining the client IP
    #[arg(long)]
    pub trust_proxy: bool,
}
