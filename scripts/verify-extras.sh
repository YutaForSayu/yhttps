#!/usr/bin/env bash
# Manual feature verification: TLS, HTTP/2, proxy, log formats, banner.
set -u
cd "$(dirname "$0")/.."
BIN=./target/release/yhtps
W=$(mktemp -d)
mkdir -p "$W/site/assets" "$W/backend"
printf '<!doctype html><h1>hello https</h1>' > "$W/site/index.html"
printf 'console.log(1);' > "$W/site/assets/a.js"
printf '{"origin":"backend"}' > "$W/backend/api.json"
PIDS=()
cleanup() { for p in "${PIDS[@]:-}"; do kill "$p" 2>/dev/null; done; rm -rf "$W"; }
trap cleanup EXIT

echo "===== 1) HTTPS self-signed + HTTP/2 ====="
"$BIN" s "$W/site" -p 5199 -S --clean >/tmp/tls.log 2>&1 & PIDS+=($!)
sleep 1.5
curl -sk -o /dev/null -w "https GET /            -> %{http_code}  http_version=%{http_version}\n" https://127.0.0.1:5199/
curl -sk --http2 -o /dev/null -w "https GET / (forced h2)-> %{http_code}  http_version=%{http_version}\n" https://127.0.0.1:5199/
printf 'asset body: '; curl -sk https://127.0.0.1:5199/assets/a.js; echo
echo "--- certificate ---"
echo | openssl s_client -connect 127.0.0.1:5199 -servername localhost 2>/dev/null \
  | openssl x509 -noout -subject 2>/dev/null
kill "${PIDS[-1]}" 2>/dev/null; wait "${PIDS[-1]}" 2>/dev/null

echo
echo "===== 2) plain HTTP/1.1 cleartext works too ====="
"$BIN" s "$W/site" -p 5200 --clean >/dev/null 2>&1 & PIDS+=($!)
sleep 1.2
curl -s -o /dev/null -w "http GET / -> %{http_code} http_version=%{http_version}\n" http://127.0.0.1:5200/
kill "${PIDS[-1]}" 2>/dev/null; wait "${PIDS[-1]}" 2>/dev/null

echo
echo "===== 3) proxy ====="
python3 -m http.server 5300 --directory "$W/backend" >/dev/null 2>&1 & PIDS+=($!)
sleep 1.5
echo "-- 3a) bare --proxy acts as a fallback for missing files --"
"$BIN" sf "$W/site" -p 5301 --proxy http://127.0.0.1:5300 --clean >/tmp/proxy.log 2>&1 & PIDS+=($!)
sleep 1.2
echo "local file            : $(curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1:5301/index.html)  (served from disk)"
curl -s -D- -o /tmp/body.txt http://127.0.0.1:5301/api.json | tr -d '\r' | grep -Ei '^(HTTP/|x-proxied-by|server:)'
echo "proxied body          : $(cat /tmp/body.txt)"
echo "miss forwarded        : $(curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1:5301/does/not/exist)  (upstream 404)"
kill "${PIDS[-1]}" 2>/dev/null; wait "${PIDS[-1]}" 2>/dev/null

echo "-- 3b) --proxy-prefix keeps SPA routing local --"
"$BIN" sf "$W/site" -p 5302 --proxy http://127.0.0.1:5300 --proxy-prefix /api --clean >/tmp/proxy2.log 2>&1 & PIDS+=($!)
sleep 1.2
echo "/api.json -> proxied  : $(curl -s http://127.0.0.1:5302/api.json)"
echo "/client/route -> SPA  : $(curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1:5302/client/route)"
echo "/assets/a.js -> local : $(curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1:5302/assets/a.js)"
kill "${PIDS[-1]}" 2>/dev/null; wait "${PIDS[-1]}" 2>/dev/null
kill "${PIDS[-1]}" 2>/dev/null; wait "${PIDS[-1]}" 2>/dev/null

echo
echo "===== 4) log formats ====="
"$BIN" s "$W/site" -p 5401 --log-format json >/tmp/json.log 2>&1 & PIDS+=($!)
sleep 1.2
curl -s -o /dev/null "http://127.0.0.1:5401/assets/a.js?x=1" -A "TestAgent/9"
sleep 0.4
echo "json    : $(grep -m1 '^{' /tmp/json.log)"
kill "${PIDS[-1]}" 2>/dev/null; wait "${PIDS[-1]}" 2>/dev/null

"$BIN" s "$W/site" -p 5402 --log-format combined >/tmp/comb.log 2>&1 & PIDS+=($!)
sleep 1.2
curl -s -o /dev/null -e "http://ref.example/" -A "TestAgent/9" "http://127.0.0.1:5402/"
sleep 0.4
echo "combined: $(grep -m1 'TestAgent' /tmp/comb.log)"
kill "${PIDS[-1]}" 2>/dev/null; wait "${PIDS[-1]}" 2>/dev/null

"$BIN" s "$W/site" -p 5403 --log-format short >/tmp/short.log 2>&1 & PIDS+=($!)
sleep 1.2
curl -s -o /dev/null "http://127.0.0.1:5403/"
sleep 0.4
echo "short   : $(grep -m1 'GET' /tmp/short.log)"
kill "${PIDS[-1]}" 2>/dev/null; wait "${PIDS[-1]}" 2>/dev/null

echo
echo "===== 5) --silent produces nothing ====="
"$BIN" s "$W/site" -p 5404 --silent >/tmp/silent.log 2>&1 & PIDS+=($!)
sleep 1.2
curl -s -o /dev/null "http://127.0.0.1:5404/"
sleep 0.3
echo "bytes written by --silent: $(wc -c < /tmp/silent.log)"
kill "${PIDS[-1]}" 2>/dev/null; wait "${PIDS[-1]}" 2>/dev/null
