#!/usr/bin/env bash
# Run one full Xray config from the test subscription and verify traffic.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
IDX="${1:-4}"
CONFIG="$ROOT/.secrets/xray-test.json"
LOG="$ROOT/.secrets/xray-test.log"
BIN="$ROOT/sidecars/bin/xray"
export XRAY_LOCATION_ASSET="$ROOT/sidecars/assets"
export M3_SOCKS_PORT="${M3_SOCKS_PORT:-20808}"
export M3_HTTP_PORT="${M3_HTTP_PORT:-20809}"

python3 "$ROOT/scripts/spikes/inspect_config.py" "$IDX" >/dev/null

"$BIN" run -test -c "$CONFIG" >/dev/null 2>"$LOG"
"$BIN" run -c "$CONFIG" >>"$LOG" 2>&1 &
PID=$!
trap 'kill "$PID" 2>/dev/null || true; wait "$PID" 2>/dev/null || true' EXIT

for _ in $(seq 1 50); do
  if (exec 3<>/dev/tcp/127.0.0.1/${M3_SOCKS_PORT}) 2>/dev/null; then exec 3>&-; break; fi
  if ! kill -0 "$PID" 2>/dev/null; then echo "xray exited early"; tail -20 "$LOG"; exit 1; fi
  sleep 0.2
done

echo "direct:  $(curl -fsS --max-time 10 https://api.ipify.org || echo FAIL)"
echo "proxied: $(curl -fsS --max-time 25 --socks5-hostname 127.0.0.1:${M3_SOCKS_PORT} https://api.ipify.org || echo FAIL)"
echo "cf trace: $(curl -fsS --max-time 25 --socks5-hostname 127.0.0.1:${M3_SOCKS_PORT} https://www.cloudflare.com/cdn-cgi/trace | grep -E '^(ip|loc)=' | tr '\n' ' ' || echo FAIL)"
