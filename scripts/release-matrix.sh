#!/usr/bin/env bash
# Build yhtps for every target reachable from THIS host.
#
#   bash scripts/release-matrix.sh [out-dir]
#
# Linux/Windows targets use `cargo zigbuild`: zig supplies the cross C/linker
# toolchain, which is what lets ring — and therefore TLS — cross-compile.
#
# Apple targets can only be built on macOS. zig has no macOS SDK, so
# `-framework CoreFoundation` cannot resolve from Linux; on a non-macOS host
# they are skipped rather than reported as failures. CI builds them on a macOS
# runner.
#
# Nothing is packaged here — scripts/collect-binaries.sh turns the built
# binaries into release assets. A TSV table records what happened, so a target
# that only built without TLS is visible instead of silently downgraded.

set -uo pipefail
cd "$(dirname "$0")/.."
export PATH="$HOME/.cargo/bin:$PATH"

OUT="${1:-dist}"
mkdir -p "$OUT"
STATUS="$OUT/build-status.tsv"
: > "$STATUS"

HOST_OS="$(uname -s)"

if [ "$HOST_OS" = "Darwin" ]; then
  TARGETS=(x86_64-apple-darwin aarch64-apple-darwin)
  MODE=native
else
  TARGETS=(
    x86_64-unknown-linux-gnu
    x86_64-unknown-linux-musl
    aarch64-unknown-linux-musl
    x86_64-pc-windows-gnu
  )
  MODE=cross
  echo "note: Apple targets are skipped here (no macOS SDK); CI builds them on macOS."
fi

# Allow the caller to narrow the set (CI runs one job per Apple target).
if [ -n "${YHTPS_TARGETS:-}" ]; then
  read -r -a TARGETS <<< "$YHTPS_TARGETS"
  echo "restricting to: ${TARGETS[*]}"
fi

for t in "${TARGETS[@]}"; do
  rustup target add "$t" >/dev/null 2>&1 || true
done

# On macOS the native toolchain is all that is needed; elsewhere zig does the
# cross-linking. Never override CC/linker here — cargo-zigbuild sets them.
build_one() {
  local t="$1" flavor="$2"
  local args=(--release --target "$t")
  [ "$flavor" = notls ] && args+=(--no-default-features)
  if [ "$MODE" = native ]; then
    cargo build "${args[@]}"
  else
    cargo zigbuild "${args[@]}"
  fi
  [ -f "target/$t/release/yhtps" ] || [ -f "target/$t/release/yhtps.exe" ]
}

for t in "${TARGETS[@]}"; do
  echo "=== $t ==="
  if build_one "$t" tls; then
    printf '%s\ttls\tok\t-\n' "$t" >> "$STATUS"
    echo "  ok (tls)"
  elif build_one "$t" notls; then
    printf '%s\tno-tls\tok\t-\n' "$t" >> "$STATUS"
    echo "  ok (no-tls fallback — https flags unavailable in this binary)"
  else
    printf '%s\t-\tFAILED\t-\n' "$t" >> "$STATUS"
    echo "  !! BUILD FAILED for $t"
  fi
done

echo
echo "=== status ==="
while IFS=$'\t' read -r a b c _; do printf '  %-28s %-7s %s\n' "$a" "$b" "$c"; done < "$STATUS"

if grep -q FAILED "$STATUS"; then
  echo
  echo "error: at least one target failed to build" >&2
  exit 1
fi