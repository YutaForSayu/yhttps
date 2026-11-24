#!/usr/bin/env bash
# Start the cross-built ARM64 binary under qemu-user and exercise it over HTTP.
set -u
cd "$(dirname "$0")/.."
BIN=dist/yhtps-linux-aarch64-static
PORT=5288
W=$(mktemp -d)
mkdir -p "$W/assets"
printf '<!doctype html><h1>arm64 under qemu</h1>' > "$W/index.html"
printf 'console.log("arm");\n' > "$W/assets/x.js"
head -c 3000 /dev/zero | tr '\0' 'a' > "$W/big.txt"

qemu-aarch64-static "$BIN" sf "$W" -p "$PORT" --gzip --clean >/tmp/qemu-serve.log 2>&1 &
P=$!
trap 'kill $P 2>/dev/null; wait $P 2>/dev/null; rm -rf "$W"' EXIT

for _ in $(seq 1 60); do
  curl -s -o /dev/null "http://127.0.0.1:$PORT/" && break
  sleep 0.2
done

echo "  GET /              -> $(curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1:$PORT/)"
echo "  body               -> $(curl -s http://127.0.0.1:$PORT/ | grep -o '<h1>.*</h1>')"
echo "  GET /assets/x.js   -> $(curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1:$PORT/assets/x.js)"
echo "  SPA fallback       -> $(curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1:$PORT/client/route)"
echo "  gzip on big.txt    -> $(curl -s -D- -o /dev/null -H 'Accept-Encoding: gzip' http://127.0.0.1:$PORT/big.txt | tr -d '\r' | grep -ci '^content-encoding: gzip')"
echo "  server header      -> $(curl -s -D- -o /dev/null http://127.0.0.1:$PORT/ | tr -d '\r' | grep -i '^server:' | tr -d '\r')"