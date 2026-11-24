#!/usr/bin/env bash
# Build and package yhtps for every target we can reach from Linux, then
# write SHA256SUMS.txt. Records a TSV status table so failures are explicit.
#
#   bash scripts/release-matrix.sh <out-dir>
#
# Each target is first attempted WITH TLS (default features); if that fails the
# target is retried with --no-default-features and the result is recorded as
# "ok (no tls)" rather than silently claiming success.

set -uo pipefail
cd "$(dirname "$0")/.."
export PATH="$HOME/.cargo/bin:$PATH"

OUT="${1:-dist}"
mkdir -p "$OUT"
STATUS="$OUT/build-status.tsv"
: > "$STATUS"

TARGETS=(
  x86_64-unknown-linux-gnu
  x86_64-unknown-linux-musl
  aarch64-unknown-linux-musl
  x86_64-pc-windows-gnu
  x86_64-apple-darwin
  aarch64-apple-darwin
)

# rustup targets, minus the native one which is already installed.
for t in "${TARGETS[@]}"; do
  rustup target add "$t" >/dev/null 2>&1 || true
done

# cargo-zigbuild sets CC/CXX/AR and the linker itself (to `zig cc`), which is
# what lets ring — and therefore TLS — cross-compile. Do not override them here.

build_one() {
  local t="$1" tls="$2"
  local dir="target/$t/release"
  if [ "$t" = "x86_64-unknown-linux-gnu" ]; then
    if [ "$tls" = tls ]; then
      cargo build --release --target "$t"
    else
      cargo build --release --target "$t" --no-default-features
    fi
  else
    if [ "$tls" = tls ]; then
      cargo zigbuild --release --target "$t"
    else
      cargo zigbuild --release --target "$t" --no-default-features
    fi
  fi
  [ -f "$dir/yhtps" ] || [ -f "$dir/yhtps.exe" ]
}

package() {
  local t="$1" flavor="$2"
  local dir="target/$t/release"
  local stage; stage="$(mktemp -d)"
  cp README.md LICENSE BUILDING.md "$stage/"
  local binname=yhtps
  [[ "$t" == *windows* ]] && binname=yhtps.exe
  cp "$dir/$binname" "$stage/"
  chmod +x "$stage/$binname" 2>/dev/null
  local archive="$OUT/yhtps-$t.tar.gz"
  tar -czf "$archive" -C "$stage" .
  rm -rf "$stage"
  local size; size=$(du -h "$archive" | cut -f1)
  printf '%s\t%s\t%s\t%s\n' "$t" "$flavor" "ok" "$size" >> "$STATUS"
  echo "  packaged $archive ($size)"
}

for t in "${TARGETS[@]}"; do
  echo "=== $t ==="
  if build_one "$t" tls; then
    package "$t" "tls"
  elif build_one "$t" notls; then
    echo "  (TLS build failed; shipping the no-TLS binary for this target)"
    package "$t" "no-tls"
  else
    printf '%s\t-\tFAILED\t-\n' "$t" >> "$STATUS"
    echo "  !! BUILD FAILED for $t"
  fi
done

# ---- source tarball ------------------------------------------------------
SRC="$OUT/yhttps-1.0.0-src.tar.gz"
git archive --format=tar.gz --prefix=yhttps-1.0.0/ -o "$SRC" HEAD
echo "  packaged $SRC ($(du -h "$SRC" | cut -f1))"

# ---- checksums -----------------------------------------------------------
( cd "$OUT" && sha256sum ./*.tar.gz > SHA256SUMS.txt )
echo
echo "=== status ==="
column -t -s$'\t' "$STATUS" 2>/dev/null || cat "$STATUS"
echo
echo "=== assets in $OUT ==="
ls -la "$OUT"
