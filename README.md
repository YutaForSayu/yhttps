# YhHTTPServer (`yhtps`)

A **zero-config HTTP file server** for static sites and built framework output —
written in Rust, shipped as one small native binary.

It replaces the two npm packages most people reach for:

| npm package    | what it gives you                                  | `yhtps` equivalent |
| -------------- | -------------------------------------------------- | ------------------ |
| `http-server`  | serve a file/dir, directory listing, a bit of TLS  | `yhtps s <path>`   |
| `serve`        | serve a build folder, SPA fallback, clean URLs     | `yhtps sf <path>`  |

Runs on **Linux**, **macOS**, **Windows** and **Android/Termux**.

---

## Install

### Prebuilt binary

Download the bare binary for your platform from
[GitHub releases](https://github.com/YutaForSayu/yhttps/releases):

```bash
curl -L -O https://github.com/YutaForSayu/yhttps/releases/latest/download/yhtps-linux-x86_64
chmod +x yhtps-linux-x86_64
sudo install -m 0755 yhtps-linux-x86_64 /usr/local/bin/yhtps
```

Available: `yhtps-linux-x86_64`, `yhtps-linux-x86_64-static` (no glibc),
`yhtps-linux-aarch64-static`, `yhtps-windows-x86_64.exe`, `yhtps-macos-x86_64`,
`yhtps-macos-aarch64`, plus `SHA256SUMS.txt`.

### From source (any platform)

```bash
git clone <this repo> yhtps && cd yhtps
cargo build --release
# binary: target/release/yhtps   (yhtps.exe on Windows)
```

### Put it on your PATH

```bash
# Linux / macOS
sudo install -m 0755 target/release/yhtps /usr/local/bin/yhtps

# Windows (PowerShell)
copy target\release\yhtps.exe C:\Windows\System32\yhtps.exe

# Android / Termux
cp target/release/yhtps $PREFIX/bin/yhtps
```

…or skip the install entirely and call the binary by relative path:

```bash
./target/release/yhtps s ./public
```

---

## Usage

```
yhtps <COMMAND> [PATH] [OPTIONS]
```

Two commands:

| command | purpose                                                                 |
| ------- | ----------------------------------------------------------------------- |
| `s`     | **serve** a file **or** a directory, statically (auto-detected)         |
| `sf`    | **serve folder** — a framework build, with SPA fallback + clean URLs    |

### Serve a single static HTML file

```bash
yhtps s /var/www/html/index.html
```

### Serve a Vite / Next / SvelteKit build

```bash
yhtps sf /var/www/html/blog/dist/
```

`sf` adds what framework builds need: unknown paths fall back to `index.html`
(client-side routing), and `/about` is served from `about.html` when present.

---

## Options

```
-p, --port <PORT>          port to listen on                    [default: 5050]
-a, --address <ADDRESS>    address to bind                [default: 0.0.0.0]
    --clean                disable request logging
    --silent               suppress all output (banner + logs)
-c, --cache <SECONDS>      Cache-Control max-age; -1 disables  [default: 3600]
-g, --gzip                 enable gzip compression
-b, --brotli               enable brotli compression
    --compress-threshold   min bytes to compress                [default: 1024]
    --cors                 send permissive CORS headers
    --cors-origin <ORIGIN> custom Access-Control-Allow-Origin
-o, --open                 open the served URL in the browser
    --spa                  force SPA fallback (single index.html)
    --fallback <FILE>      SPA fallback file             [default: index.html]
    --no-listing           disable directory listings
-i, --index <FILE,...>     index file names to try
-e, --extensions <EXT,..>  extensions to try for clean URLs
-P, --proxy <URL>          proxy requests to this origin
    --proxy-prefix <PATH>  only proxy paths under this prefix
    --username <USER>      HTTP Basic auth user
    --password <PASS>      HTTP Basic auth password
-S, --ssl                  serve over HTTPS (self-signed if no cert given)
-C, --cert <FILE>          TLS certificate chain (PEM)
-K, --key <FILE>           TLS private key (PEM)
    --no-range             disable byte-range requests
    --timeout <SECONDS>    header read timeout
    --header "Name: Value" extra response header (repeatable)
    --log-format <FMT>     pretty | short | combined | json   [default: pretty]
    --no-color             disable coloured output
    --trust-proxy          trust X-Forwarded-For for the client IP
-h, --help                 full help
-V, --version              version
```

---

## Logging

Every request is logged **by default**, in full, including the client IP:

```
15:23:01  192.168.1.24  GET      200 OK      1.2 kB     1.4ms  /assets/app.js  curl/8.5.0
15:23:02  192.168.1.24  GET      304 Not …         -     0.2ms  /favicon.ico    Mozilla/5.0
15:23:09  127.0.0.1     GET      404 Not F…     316 B     0.3ms  /nope           curl/8.5.0
```

Turn it off with `--clean`:

```bash
yhtps s ./public --clean
```

Other formats:

```bash
yhtps s ./public --log-format json       # one JSON object per request
yhtps s ./public --log-format combined   # Apache/nginx combined access log
yhtps s ./public --log-format short      # GET /index.html 200
```

---

## Examples

```bash
# custom port, open the browser
yhtps sf ./dist -p 8080 -o

# compression + CORS for a CDN-style static host
yhtps s ./public --gzip --brotli --cors

# SPA behind a dev API
yhtps sf ./dist --proxy http://127.0.0.1:3000

# SPA plus API, keeping client-side routes local
yhtps sf ./dist -P http://127.0.0.1:3000 --proxy-prefix /api

# HTTPS with a throwaway certificate
yhtps s ./public -S

# HTTPS with your own certificate
yhtps s ./public -C cert.pem -K key.pem

# password protect a directory
yhtps s ./private --username admin --password s3cret

# clean URLs: /about -> /about.html, /pricing -> /pricing/index.html
yhtps s ./site -e html

# custom headers
yhtps s ./public --header "X-Frame-Options: DENY" --header "Referrer-Policy: no-referrer"
```

---

## Behaviour notes

* **Directory requests without a trailing slash** are answered with `301` to the
  slashed URL, so relative links work in listings.
* **Conditional requests** (`If-None-Match`, `If-Modified-Since`) return `304`;
  files carry a weak `ETag` derived from size + mtime.
* **Byte ranges** (`Range: bytes=…`) return `206` (unless compression kicks in,
  in which case the full compressed body is sent).
* **Compression** is negotiated from `Accept-Encoding`, brotli preferred over
  gzip, and skips already-compressed formats (images, fonts, zip, wasm…).
* **Path traversal** (`..`, NUL bytes, `\`, drive letters, control characters)
  is rejected with `400`.
* **The right cache headers** are sent: `Cache-Control`, `ETag`, `Last-Modified`.
* **Graceful shutdown** on `Ctrl+C`/`SIGTERM` — in-flight requests finish first.
* **`--proxy` without a prefix** forwards every request that is not a file on
  disk. With `--proxy-prefix /api` only that subtree is proxied, so SPA routing
  (`/about`, `/settings/…`) keeps returning `index.html`.
* **`--clean`** turns request logging off but keeps the startup banner;
  **`--silent`** suppresses everything.

---

## Platform support

| Platform        | Status | Notes                                                      |
| --------------- | ------ | ---------------------------------------------------------- |
| Linux x86_64    | ✅     | static musl build available (`make musl`, or `make dist`)   |
| Linux aarch64   | ✅     | `make arm64`                                                |
| macOS (x86/ARM) | ✅     | `make macos`, or `make dist` via zig                        |
| Windows         | ✅     | `make windows`                                              |
| Android/Termux  | ✅     | `pkg install rust` then `cargo build --release`             |

Prebuilt binaries for all of the above are attached to
[GitHub releases](https://github.com/YutaForSayu/yhttps/releases).
See [`BUILDING.md`](BUILDING.md) for cross-compilation recipes, and
`bash scripts/release-matrix.sh dist` (or `make dist`) to produce the whole
matrix yourself.

---

## License

MIT — see [LICENSE](LICENSE).
