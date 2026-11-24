#!/usr/bin/env bash
# Reproduce the exact invocation examples from the request.
set -u
cd "$(dirname "$0")/.."
BIN="$(pwd)/target/release/yhtps"
PIDS=()
cleanup() { for p in "${PIDS[@]:-}"; do kill "$p" 2>/dev/null; done; }
trap cleanup EXIT

# --- an install by copying the binary onto PATH ---------------------------
install -m 0755 "$BIN" /usr/local/bin/yhtps
echo "installed: $(command -v yhtps)"

# --- build the two document roots the request names -----------------------
mkdir -p /var/www/html/blog/dist/assets
cat > /var/www/html/index.html <<'HTML'
<!doctype html>
<html lang="en"><head><meta charset="utf-8"><title>Static site</title></head>
<body><h1>Static HTML served by YhHTTPServer</h1></body></html>
HTML

# a Vite-shaped build output
cat > /var/www/html/blog/dist/index.html <<'HTML'
<!doctype html>
<html lang="en"><head><meta charset="utf-8"><title>Blog (Vite build)</title>
<script type="module" crossorigin src="/assets/index-a1b2c3.js"></script></head>
<body><div id="app">Vite build output</div></body></html>
HTML
printf 'console.log("vite bundle");\n' > /var/www/html/blog/dist/assets/index-a1b2c3.js
printf 'body{margin:0}\n' > /var/www/html/blog/dist/assets/index-d4e5f6.css
printf '<!doctype html><title>about</title><h1>About</h1>' > /var/www/html/blog/dist/about.html

echo
echo "############  yhtps s /var/www/html/index.html  ############"
yhtps s /var/www/html/index.html >/tmp/d1.log 2>&1 & PIDS+=($!)
sleep 1.1
cat /tmp/d1.log
echo "-- requests --"
curl -s http://127.0.0.1:5050/ | grep -o '<h1>.*</h1>'
curl -s -o /dev/null -w "  GET /      -> %{http_code}\n" http://127.0.0.1:5050/
curl -s -o /dev/null -w "  GET /nope  -> %{http_code}  (no SPA fallback in static mode)\n" http://127.0.0.1:5050/nope
kill "${PIDS[-1]}" 2>/dev/null; wait "${PIDS[-1]}" 2>/dev/null

echo
echo "############  yhtps sf /var/www/html/blog/dist/  ############"
yhtps sf /var/www/html/blog/dist/ >/tmp/d2.log 2>&1 & PIDS+=($!)
sleep 1.1
cat /tmp/d2.log
echo "-- requests --"
echo "  /                     -> $(curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1:5050/)   (index.html)"
echo "  /assets/index-a1b2c3.js -> $(curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1:5050/assets/index-a1b2c3.js)   $(curl -s -o /dev/null -w '%{content_type}' http://127.0.0.1:5050/assets/index-a1b2c3.js)"
echo "  /about                -> $(curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1:5050/about)   (clean URL -> about.html)"
echo "  /article/42           -> $(curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1:5050/article/42)   (SPA fallback)"
echo "  /missing.js           -> $(curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1:5050/missing.js)   (SPA fallback)"
echo
echo "-- the default request log (client IP, status, size, duration, UA) --"
sed -n '/press Ctrl/,$p' /tmp/d2.log | grep 'GET'
kill "${PIDS[-1]}" 2>/dev/null; wait "${PIDS[-1]}" 2>/dev/null

echo
echo "############  custom port + --clean  ############"
yhtps sf /var/www/html/blog/dist/ -p 8080 --clean >/tmp/d3.log 2>&1 & PIDS+=($!)
sleep 1.1
echo "  listening on 8080 -> $(curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1:8080/)"
curl -s -o /dev/null http://127.0.0.1:8080/assets/index-d4e5f6.css
sleep 0.3
echo "  request lines in --clean log: $(grep -c 'assets/index-d4e5f6.css' /tmp/d3.log || true)"
kill "${PIDS[-1]}" 2>/dev/null; wait "${PIDS[-1]}" 2>/dev/null

echo
echo "############  relative-path invocation  ############"
./target/release/yhtps --version
