#!/usr/bin/env bash
# Collect the raw binaries produced by release-matrix.sh into dist/ under
# simple platform names, write SHA256SUMS.txt, and (with --upload) attach
# them to a GitHub release.
#
#   bash scripts/collect-binaries.sh              # collect + checksums
#   bash scripts/collect-binaries.sh --upload v1.0.0
#
# Only targets whose binary actually exists are collected, so this is safe to
# run while a build is still in flight.

set -uo pipefail
cd "$(dirname "$0")/.."
OUT=dist
RELEASE=""
if [ "${1:-}" = "--upload" ]; then
  RELEASE="${2:?usage: collect-binaries.sh --upload <tag>}"
fi

# target triple -> asset name
map_target() {
  case "$1" in
    x86_64-unknown-linux-gnu)    echo "yhtps-linux-x86_64" ;;
    x86_64-unknown-linux-musl)   echo "yhtps-linux-x86_64-static" ;;
    aarch64-unknown-linux-musl)  echo "yhtps-linux-aarch64-static" ;;
    aarch64-unknown-linux-gnu)   echo "yhtps-linux-aarch64" ;;
    x86_64-pc-windows-gnu)       echo "yhtps-windows-x86_64.exe" ;;
    x86_64-pc-windows-msvc)      echo "yhtps-windows-x86_64.exe" ;;
    x86_64-apple-darwin)         echo "yhtps-macos-x86_64" ;;
    aarch64-apple-darwin)        echo "yhtps-macos-aarch64" ;;
    *)                           echo "" ;;
  esac
}

TARGETS=(
  x86_64-unknown-linux-gnu
  x86_64-unknown-linux-musl
  aarch64-unknown-linux-musl
  aarch64-unknown-linux-gnu
  x86_64-pc-windows-gnu
  x86_64-apple-darwin
  aarch64-apple-darwin
)

mkdir -p "$OUT"
collected=()

for t in "${TARGETS[@]}"; do
  name="$(map_target "$t")"
  [ -n "$name" ] || continue
  src=""
  for cand in "target/$t/release/yhtps" "target/$t/release/yhtps.exe"; do
    [ -f "$cand" ] && { src="$cand"; break; }
  done
  [ -n "$src" ] || continue
  cp "$src" "$OUT/$name"
  chmod +x "$OUT/$name" 2>/dev/null
  collected+=("$name")
  printf '  collected %-28s %s bytes  (-%s)\n' "$name" "$(stat -c%s "$OUT/$name")" "$t"
done

if [ ${#collected[@]} -eq 0 ]; then
  echo "no binaries found under target/ — build first" >&2
  exit 1
fi

( cd "$OUT" && sha256sum "${collected[@]}" > SHA256SUMS.txt )
echo
echo "=== SHA256SUMS.txt ==="
cat "$OUT/SHA256SUMS.txt"

if [ -n "$RELEASE" ]; then
  echo
  echo "=== uploading to release $RELEASE ==="
  ( cd "$OUT" && gh release upload "$RELEASE" "${collected[@]}" SHA256SUMS.txt --clobber )
  echo
  gh release view "$RELEASE" --json assets \
    --jq '.assets[] | "  \(.name)  \(.size) bytes  \(.state)"'
fi
