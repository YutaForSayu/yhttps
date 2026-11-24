# Building YhHTTPServer for every supported platform

The crate builds with the **stable** toolchain and has no build scripts beyond
`ring`/`aws-lc` style C dependencies pulled in by rustls. If you want a build
with **no C dependencies at all** (easiest for cross-compiling), disable TLS:

```bash
cargo build --release --no-default-features
```

That drops HTTPS support and the `ring`/`rcgen` C code, leaving a pure-Rust
binary that cross-compiles with nothing but the Rust target installed.

---

## Linux (native)

```bash
cargo build --release
```

### Static musl binary (runs anywhere, no glibc)

```bash
rustup target add x86_64-unknown-linux-musl
cargo build --release --target x86_64-unknown-linux-musl --no-default-features
# → target/x86_64-unknown-linux-musl/release/yhtps
```

### Linux ARM64 (Raspberry Pi 4/5, ARM servers)

```bash
sudo apt install gcc-aarch64-linux-gnu
rustup target add aarch64-unknown-linux-gnu
export CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc
cargo build --release --target aarch64-unknown-linux-gnu --no-default-features
```

---

## macOS

```bash
rustup target add x86_64-apple-darwin aarch64-apple-darwin

# Intel
cargo build --release --target x86_64-apple-darwin

# Apple Silicon
cargo build --release --target aarch64-apple-darwin

# Universal binary
lipo -create \
  target/x86_64-apple-darwin/release/yhtps \
  target/aarch64-apple-darwin/release/yhtps \
  -output yhtps-universal
```

Building for macOS from Linux requires `cargo-zigbuild` or `osxcross`; the
simplest reliable route is to build on a Mac or in CI (`.github/workflows/release.yml`).

---

## Windows

```powershell
rustup target add x86_64-pc-windows-msvc
cargo build --release --target x86_64-pc-windows-msvc
# → target\x86_64-pc-windows-msvc\release\yhtps.exe
```

Or build for GNU/MinGW (works from Linux with `mingw-w64`):

```bash
sudo apt install gcc-mingw-w64-x86-64
rustup target add x86_64-pc-windows-gnu
cargo build --release --target x86_64-pc-windows-gnu --no-default-features
```

---

## Android / Termux

Termux ships a Rust package; no NDK needed for a Termux-local binary:

```bash
pkg update
pkg install rust git clang make openssl
git clone <this repo> yhtps && cd yhtps
cargo build --release
cp target/release/yhtps $PREFIX/bin/yhtps
yhtps s ~/storage/shared/site
```

Notes:

* Building **with** TLS on Termux requires `openssl` + `clang` for `ring`; if
  that fails, build with `--no-default-features` and serve plain HTTP.
* `-o/--open` uses `termux-open-url` when available (`pkg install termux-tools`).
* Network URLs printed in the banner work on the local Wi-Fi.

---

## Cross-compilation shortcut: `cargo-zigbuild`

```bash
cargo install cargo-zigbuild
cargo zigbuild --release --target x86_64-unknown-linux-gnu      # glibc 2.17
cargo zigbuild --release --target aarch64-unknown-linux-musl
cargo zigbuild --release --target x86_64-pc-windows-gnu
```

---

## `make` targets

See the `Makefile` in the repository root:

```bash
make            # native release build
make musl       # static musl x86_64
make windows    # x86_64-pc-windows-gnu
make macos      # both macOS slices + universal binary (needs a Mac)
make arm64      # linux aarch64
make all        # everything that works on this host
make test       # unit + integration tests
```
