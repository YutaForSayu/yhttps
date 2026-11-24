#!/usr/bin/env bash
# Render the startup banner (both normal and clean mode) for visual review.
set -u
cd "$(dirname "$0")/.."
BIN=./target/release/yhtps
W=$(mktemp -d); mkdir -p "$W/site/assets"
printf '<!doctype html><h1>hi</h1>' > "$W/site/index.html"
PIDS=()
cleanup() { for p in "${PIDS[@]:-}"; do kill "$p" 2>/dev/null; done; rm -rf "$W"; }
trap cleanup EXIT

echo "############ plain default banner ############"
"$BIN" s "$W/site" -p 5050 >/tmp/b1.log 2>&1 & PIDS+=($!)
sleep 1.0
cat /tmp/b1.log
kill "${PIDS[-1]}" 2>/dev/null; wait "${PIDS[-1]}" 2>/dev/null

echo
echo "############ framework mode, all the flags ############"
"$BIN" sf "$W/site" -p 8080 --gzip --brotli --cors -o -S \
      --proxy http://127.0.0.1:3000 --proxy-prefix /api \
      --username u --password p >/tmp/b2.log 2>&1 & PIDS+=($!)
sleep 1.0
cat /tmp/b2.log
kill "${PIDS[-1]}" 2>/dev/null; wait "${PIDS[-1]}" 2>/dev/null

echo
echo "############ clean mode ############"
"$BIN" s "$W/site" -p 5050 --clean >/tmp/b3.log 2>&1 & PIDS+=($!)
sleep 1.0
cat /tmp/b3.log
curl -s -o /dev/null "http://127.0.0.1:5050/"
sleep 0.3
echo "--- (a request was made; log is still empty below) ---"
cat /tmp/b3.log
kill "${PIDS[-1]}" 2>/dev/null; wait "${PIDS[-1]}" 2>/dev/null

echo
echo "############ logged request line, coloured ############"
"$BIN" s "$W/site" -p 5050 >/tmp/b4.log 2>&1 & PIDS+=($!)
sleep 1.0
curl -s -o /dev/null "http://127.0.0.1:5050/index.html" -A "Mozilla/5.0 (X11; Linux x86_64)"
curl -s -o /dev/null "http://127.0.0.1:5050/missing.txt"
sleep 0.4
sed -n '/serving/,$p' /tmp/b4.log | grep -v '^  press' | grep -v '│' | tail -6
kill "${PIDS[-1]}" 2>/dev/null; wait "${PIDS[-1]}" 2>/dev/null
