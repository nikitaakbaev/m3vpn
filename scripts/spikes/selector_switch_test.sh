#!/usr/bin/env bash
# M0 spike: switching the selector via Clash API must not interrupt an active download.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
BIN="$ROOT/sidecars/bin/hiddify-sing-box"
WORK="$(mktemp -d)"
LOG="$WORK/sing-box.log"
SECRET="m0-spike-secret"
PORT=29090

cleanup() {
  [ -n "${CURL_PID:-}" ] && kill "$CURL_PID" 2>/dev/null || true
  [ -n "${BOX_PID:-}" ] && kill "$BOX_PID" 2>/dev/null || true
  [ -n "${HTTP_PID:-}" ] && kill "$HTTP_PID" 2>/dev/null || true
  rm -rf "$WORK"
}
trap cleanup EXIT

# --- local origin server and test payload ---------------------------------
dd if=/dev/zero of="$WORK/payload.bin" bs=1M count=8 status=none
( cd "$WORK" && python3 -m http.server 18080 --bind 127.0.0.1 >/dev/null 2>&1 ) &
HTTP_PID=$!

# --- sing-box config with two direct outbounds and a selector -------------
cat > "$WORK/config.json" <<EOF
{
  "log": { "level": "info", "timestamp": true },
  "inbounds": [
    { "type": "mixed", "tag": "mixed-in", "listen": "127.0.0.1", "listen_port": 20808 }
  ],
  "outbounds": [
    { "type": "selector", "tag": "proxy", "outbounds": ["srv-a", "srv-b"],
      "default": "srv-a", "interrupt_exist_connections": false },
    { "type": "direct", "tag": "srv-a" },
    { "type": "direct", "tag": "srv-b" }
  ],
  "route": { "final": "proxy" },
  "experimental": {
    "clash_api": { "external_controller": "127.0.0.1:${PORT}", "secret": "${SECRET}" }
  }
}
EOF

"$BIN" check -c "$WORK/config.json" >/dev/null
"$BIN" run -c "$WORK/config.json" >"$LOG" 2>&1 &
BOX_PID=$!

api() { curl -fsS --max-time 5 -H "Authorization: Bearer $SECRET" "$@"; }

for _ in $(seq 1 50); do
  api "http://127.0.0.1:${PORT}/proxies/proxy" >/dev/null 2>&1 && break
  if ! kill -0 "$BOX_PID" 2>/dev/null; then echo "sing-box exited"; cat "$LOG"; exit 1; fi
  sleep 0.2
done

before=$(api "http://127.0.0.1:${PORT}/proxies/proxy" | jq -r '.now')
echo "selector before: $before"

# --- start a throttled download through the proxy -------------------------
curl -fsS --max-time 60 --limit-rate 400k --socks5-hostname 127.0.0.1:20808 \
  "http://127.0.0.1:18080/payload.bin" -o "$WORK/downloaded.bin" &
CURL_PID=$!

sleep 3
[ -s "$WORK/downloaded.bin" ] || { echo "download did not start"; exit 1; }
partial=$(stat -c %s "$WORK/downloaded.bin")
echo "downloaded before switch: $partial bytes"

# --- switch the selector while the download is running --------------------
api -X PUT -H 'Content-Type: application/json' \
  -d '{"name":"srv-b"}' "http://127.0.0.1:${PORT}/proxies/proxy" >/dev/null
after=$(api "http://127.0.0.1:${PORT}/proxies/proxy" | jq -r '.now')
echo "selector after: $after"
[ "$after" = "srv-b" ] || { echo "selector did not change"; exit 1; }

# --- verify the download survived -----------------------------------------
curl_rc=0
wait "$CURL_PID" || curl_rc=$?
CURL_PID=""
size=$(stat -c %s "$WORK/downloaded.bin" 2>/dev/null || echo 0)
echo "curl exit: $curl_rc, downloaded: $size bytes"

if [ "$curl_rc" -ne 0 ]; then
  echo "DOWNLOAD_INTERRUPTED"
  exit 1
fi
if [ "$size" -ne $((8 * 1024 * 1024)) ]; then
  echo "DOWNLOAD_INCOMPLETE"
  exit 1
fi
if cmp -s "$WORK/payload.bin" "$WORK/downloaded.bin"; then
  echo "SELECTOR_SWITCH_OK"
else
  echo "PAYLOAD_MISMATCH"
  exit 1
fi
