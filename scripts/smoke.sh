#!/usr/bin/env bash
#
# End-to-end smoke test for YhHTTPServer.
#
#   bash scripts/smoke.sh [path-to-yhtps-binary]
#
# Starts a real server against a throwaway fixture tree and exercises the
# documented behaviour with curl. Exits non-zero on the first failure.

set -uo pipefail

BIN="${1:-target/release/yhtps}"
PORT="${PORT:-5099}"
BASE="http://127.0.0.1:${PORT}"
PASS=0
FAIL=0

if [[ ! -x "$BIN" ]]; then
  echo "error: binary '$BIN' not found — run 'make release' first" >&2
  exit 2
fi

WORK="$(mktemp -d)"
trap 'cleanup' EXIT

SERVER_PID=""
cleanup() {
  [[ -n "$SERVER_PID" ]] && kill "$SERVER_PID" 2>/dev/null
  wait "$SERVER_PID" 2>/dev/null
  rm -rf "$WORK"
}

# ---------------------------------------------------------------- fixtures
mkdir -p "$WORK/site/assets" "$WORK/site/blog"
cat > "$WORK/site/index.html" <<'HTML'
<!doctype html><title>home</title><h1>home</h1>
HTML
cat > "$WORK/site/about.html" <<'HTML'
<!doctype html><title>about</title><h1>about</h1>
HTML
printf 'body{color:red}' > "$WORK/site/assets/app.css"
# a big "binary" (never compressed, regardless of size)
head -c 4096 /dev/urandom > "$WORK/site/assets/photo.png"
# something big + compressible so the compression path is exercised
python3 - "$WORK/site/assets/big.js" <<'PY'
import sys
open(sys.argv[1], 'w').write("console.log('x');\n" * 4000)
PY
cat > "$WORK/site/blog/index.html" <<'HTML'
<!doctype html><title>blog</title>
HTML

start_server() {
  # shellcheck disable=SC2086
  "$BIN" "$@" >"$WORK/server.log" 2>&1 &
  SERVER_PID=$!
  for _ in $(seq 1 100); do
    if curl -s -o /dev/null "$BASE/"; then return 0; fi
    if ! kill -0 "$SERVER_PID" 2>/dev/null; then
      echo "server died on startup:"; cat "$WORK/server.log"; exit 2
    fi
    sleep 0.05
  done
  echo "server did not come up"; cat "$WORK/server.log"; exit 2
}

stop_server() {
  [[ -n "$SERVER_PID" ]] && kill "$SERVER_PID" 2>/dev/null
  wait "$SERVER_PID" 2>/dev/null
  SERVER_PID=""
}

check() {
  local label="$1" expected="$2" actual="$3"
  if [[ "$expected" == "$actual" ]]; then
    printf '  \033[32m✓\033[0m %s\n' "$label"
    PASS=$((PASS + 1))
  else
    printf '  \033[31m✗\033[0m %s (expected %s, got %s)\n' "$label" "$expected" "$actual"
    FAIL=$((FAIL + 1))
  fi
}

code() { curl -s -o /dev/null -w '%{http_code}' "$@"; }

# ================================================================= suite 1
echo "== static mode: yhtps s <dir>"
start_server s "$WORK/site" -p "$PORT" --clean

check "GET / -> 200"                 200 "$(code "$BASE/")"
check "GET /index.html -> 200"       200 "$(code "$BASE/index.html")"
check "GET /assets/app.css -> 200"   200 "$(code "$BASE/assets/app.css")"
check "GET /missing -> 404"          404 "$(code "$BASE/missing")"
check "GET /blog -> 301 (slash)"     301 "$(code "$BASE/blog")"
check "GET /blog/ -> 200"            200 "$(code "$BASE/blog/")"
check "POST / -> 405"                405 "$(code -X POST "$BASE/")"
check "OPTIONS / -> 204"             204 "$(code -X OPTIONS "$BASE/")"
check "HEAD / -> 200"                200 "$(code -I "$BASE/")"
check "traversal blocked -> 400"     400 "$(code --path-as-is "$BASE/../../etc/passwd")"
check "listing page generated"       1   "$(curl -s "$BASE/assets/" | grep -c '<h1>Index of')"
check "listing shows a file"         1   "$(curl -s "$BASE/assets/" | grep -c 'photo.png')"
check "listing sorts by size"        1   "$(curl -s "$BASE/assets/?sort=size&order=desc" | grep -c 'big.js')"
check "css content-type"             "text/css; charset=utf-8" \
      "$(curl -s -D- -o /dev/null "$BASE/assets/app.css" | tr -d '\r' | awk 'tolower($1)=="content-type:"{print $2" "$3}' | sed 's/ $//')"
check "etag present"                 1   "$(curl -s -D- -o /dev/null "$BASE/assets/app.css" | tr -d '\r' | grep -ci '^etag:')"
check "accept-ranges present"        1   "$(curl -s -D- -o /dev/null "$BASE/assets/app.css" | tr -d '\r' | grep -ci '^accept-ranges: bytes')"
check "server header"                1   "$(curl -s -D- -o /dev/null "$BASE/" | tr -d '\r' | grep -ci '^server: yhtps/')"

ETAG=$(curl -s -D- -o /dev/null "$BASE/assets/app.css" | tr -d '\r' | awk 'tolower($1)=="etag:"{print $2}')
check "if-none-match -> 304"         304 "$(code -H "If-None-Match: $ETAG" "$BASE/assets/app.css")"

check "range -> 206"                 206 "$(code -r 0-9 "$BASE/assets/app.css")"
check "range content-range"          1   "$(curl -s -D- -o /dev/null -r 0-9 "$BASE/assets/app.css" | tr -d '\r' | grep -ci '^content-range: bytes 0-9/')"
check "bad range -> 416"             416 "$(code -r 999999-1000000 "$BASE/assets/app.css")"

stop_server

# ================================================================= suite 2
echo "== framework mode: yhtps sf <dir> (SPA + clean URLs)"
start_server sf "$WORK/site" -p "$PORT" --clean

check "clean URL /about -> 200"      200 "$(code "$BASE/about")"
check "clean URL body is about"      1   "$(curl -s "$BASE/about" | grep -c '<h1>about</h1>')"
check "SPA fallback /deep/route"     200 "$(code "$BASE/deep/route")"
check "SPA fallback returns index"   1   "$(curl -s "$BASE/nope/nope" | grep -c '<h1>home</h1>')"
check "existing asset still 200"     200 "$(code "$BASE/assets/app.css")"

stop_server

# ================================================================= suite 3
echo "== compression, cors, custom headers"
start_server s "$WORK/site" -p "$PORT" --clean --gzip --brotli --cors \
             --header "X-Test-Header: yes"

check "gzip honoured"                1   "$(curl -s -D- -o /dev/null -H 'Accept-Encoding: gzip' "$BASE/assets/big.js" | tr -d '\r' | grep -ci '^content-encoding: gzip')"
check "brotli preferred"             1   "$(curl -s -D- -o /dev/null -H 'Accept-Encoding: gzip, br' "$BASE/assets/big.js" | tr -d '\r' | grep -ci '^content-encoding: br')"
check "vary accept-encoding"         1   "$(curl -s -D- -o /dev/null -H 'Accept-Encoding: gzip' "$BASE/assets/big.js" | tr -d '\r' | grep -ci '^vary: accept-encoding')"
check "gzip body decodes"            1   "$(curl -s --compressed "$BASE/assets/big.js" | grep -m1 -c "console.log")"
check "brotli body decodes"          1   "$(curl -s -H 'Accept-Encoding: br' --compressed "$BASE/assets/big.js" | grep -m1 -c "console.log")"
check "cors header"                  1   "$(curl -s -D- -o /dev/null "$BASE/" | tr -d '\r' | grep -ci '^access-control-allow-origin: \*')"
check "custom header"                1   "$(curl -s -D- -o /dev/null "$BASE/" | tr -d '\r' | grep -ci '^x-test-header: yes')"
check "no compression below threshold" 0 "$(curl -s -D- -o /dev/null -H 'Accept-Encoding: gzip' "$BASE/assets/app.css" | tr -d '\r' | grep -ci '^content-encoding:')"
check "no compression for png"       0   "$(curl -s -D- -o /dev/null -H 'Accept-Encoding: gzip' "$BASE/assets/photo.png" | tr -d '\r' | grep -ci '^content-encoding:')"
check "compressed body is smaller"   1   "$( [ "$(curl -s -o /dev/null -w '%{size_download}' -H 'Accept-Encoding: gzip' "$BASE/assets/big.js")" -lt 60000 ] && echo 1 || echo 0 )"

stop_server

# ================================================================= suite 4
echo "== basic auth"
start_server s "$WORK/site" -p "$PORT" --clean --username admin --password s3cret

check "no creds -> 401"              401 "$(code "$BASE/")"
check "www-authenticate"             1   "$(curl -s -D- -o /dev/null "$BASE/" | tr -d '\r' | grep -ci '^www-authenticate: basic')"
check "bad creds -> 401"             401 "$(code -u admin:wrong "$BASE/")"
check "good creds -> 200"            200 "$(code -u admin:s3cret "$BASE/")"

stop_server

# ================================================================= suite 5
echo "== single file mode"
start_server s "$WORK/site/index.html" -p "$PORT" --clean

check "GET / serves the file"        200 "$(code "$BASE/")"
check "body is the file"             1   "$(curl -s "$BASE/" | grep -c '<h1>home</h1>')"
check "sibling asset works"          200 "$(code "$BASE/assets/app.css")"
check "unknown path -> 404"          404 "$(code "$BASE/nope")"

stop_server

# ================================================================= suite 6
echo "== clean mode is quiet / logs are on by default"
start_server s "$WORK/site" -p "$PORT" --clean
curl -s -o /dev/null "$BASE/assets/app.css"
sleep 0.2
LOG_REQUESTS=$(grep -c 'assets/app.css' "$WORK/server.log" 2>/dev/null || true)
stop_server
check "clean mode: no request logs"  0 "$LOG_REQUESTS"

start_server s "$WORK/site" -p "$PORT"
curl -s -o /dev/null "$BASE/assets/app.css"
sleep 0.3
stop_server
check "default mode logs request"    1 "$(grep -c 'assets/app.css' "$WORK/server.log")"
check "log contains client ip"       1 "$(grep 'assets/app.css' "$WORK/server.log" | grep -c '127.0.0.1')"
check "log contains user agent"      1 "$(grep 'assets/app.css' "$WORK/server.log" | grep -c 'curl/')"
check "log contains status"          1 "$(grep 'assets/app.css' "$WORK/server.log" | grep -c '200')"

# ================================================================= done
echo
if [[ $FAIL -eq 0 ]]; then
  printf '\033[32m%s checks passed\033[0m\n' "$PASS"
  exit 0
else
  printf '\033[31m%s failed\033[0m, %s passed\n' "$FAIL" "$PASS"
  exit 1
fi
