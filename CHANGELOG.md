# Changelog

All notable changes to YhHTTPServer are documented here.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.0.0] — 2026-11-24

First release. A single native binary that replaces the npm `http-server` and
`serve` packages for static sites and built framework output.

### Commands

- `yhtps s <path>` — serve a file or a directory (auto-detected).
- `yhtps sf <path>` — serve a framework build folder with SPA fallback and
  clean URLs (`/about` → `about.html`, `/blog/` → `blog/index.html`).

### Serving

- Default port **5050**, overridable with `-p/--port`; bind address via
  `-a/--address` (default `0.0.0.0`).
- Directory listings with sorting by name / size / modified time, rendered as a
  self-contained dark-theme HTML page.
- `301` redirects for directories requested without a trailing slash.
- Index files (`-i/--index`), extension fallback (`-e/--extensions`).
- Single-file mode: pointing at a file serves it at `/` while its sibling
  assets keep resolving.

### HTTP correctness

- Weak `ETag` (size + mtime) and `Last-Modified`, with
  `If-None-Match` / `If-Modified-Since` → `304`.
- Byte-range requests → `206` with `Content-Range`; unsatisfiable ranges → `416`.
- `Cache-Control` derived from `-c/--cache` (`-c-1` disables caching).
- Correct `Content-Type` for 50+ extensions, with overrides for `mjs`, `wasm`,
  `webmanifest`, `woff2`, and friends; `charset=utf-8` on text types.
- `HEAD` and `OPTIONS` handled properly; other methods get `405` + `Allow`.

### Compression

- `--gzip` and `--brotli`, brotli preferred, negotiated from `Accept-Encoding`
  with `q=` handling and `Vary: Accept-Encoding`.
- Skips already-compressed formats (images, fonts, video, zip, wasm…) and
  bodies below `--compress-threshold`.

### Logging

- Full request logging **on by default**: timestamp, client IP, method,
  status (colourised by class), response size, duration, path and User-Agent.
- `--log-format pretty|short|combined|json`.
- `--clean` disables request logging; `--silent` disables all output.
- `--trust-proxy` reads `X-Forwarded-For` / `X-Real-IP`.

### Extras

- `--cors` / `--cors-origin`, `--header "Name: Value"` (repeatable).
- `-P/--proxy` (fallback proxy) and `--proxy-prefix /api` (prefix proxy that
  leaves SPA routing intact).
- `--username` / `--password` HTTP Basic auth.
- `-S/--ssl` with automatic self-signed certificate generation (`rcgen`),
  or `-C/--cert` + `-K/--key` for your own PEM pair. HTTP/2 over TLS via ALPN.
- `-o/--open` launches the browser (`xdg-open`, `open`, `cmd /C start`,
  `termux-open`).
- Coloured startup banner listing every reachable local and network URL,
  IPv6 bracketing included.
- Graceful shutdown on `SIGINT`/`SIGTERM`; in-flight requests are drained.

### Portability

- Linux, macOS, Windows and Android/Termux from one codebase.
- `--no-default-features` drops TLS (and its C dependencies) for a pure-Rust
  binary that cross-compiles with only a Rust target installed.
- `Makefile`, `BUILDING.md` and a GitHub Actions release workflow for
  musl, aarch64, macOS and Windows artefacts.

### Testing

- 11 unit tests over path sanitisation, content-negotiation, MIME, auth and
  formatting.
- `scripts/smoke.sh`: 49 end-to-end checks against a real server over HTTP.
- `scripts/verify-extras.sh`: HTTPS/HTTP2, proxy, log formats, silent mode.

[1.0.0]: https://example.com/yhtps/releases/tag/v1.0.0
