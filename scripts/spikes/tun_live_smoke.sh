#!/usr/bin/env bash
# Live TUN smoke test. This changes host routes while it runs.
# Run only with an installed helper, a running m3vpn front proxy and the
# m3vpn group active in the current session.
#
#   M3VPN_TUN_SMOKE=1 bash scripts/spikes/tun_live_smoke.sh
#
# Optional: M3VPN_TUN_KILL_SWITCH=1, APP_PID=..., FRONT_PID=..., BACKEND_PID=...,
# SOCKS_PORT=...
set -euo pipefail

if [[ "${M3VPN_TUN_SMOKE:-0}" != "1" ]]; then
  echo "Refusing to change host routes. Re-run with M3VPN_TUN_SMOKE=1." >&2
  exit 2
fi

HELPER="${M3VPN_HELPER_BIN:-/usr/lib/m3vpn/m3vpn-helper}"
SOCKET="${M3VPN_HELPER_SOCKET:-/run/m3vpn/helper.sock}"
[[ -x "$HELPER" ]] || { echo "missing helper: $HELPER" >&2; exit 1; }
[[ -S "$SOCKET" ]] || { echo "helper socket missing: $SOCKET" >&2; exit 1; }

APP_PID="${APP_PID:-$(pgrep -n -x m3vpn || true)}"
FRONT_PID="${FRONT_PID:-$(pgrep -n -f 'hiddify-sing-box run -c .*front.json' || true)}"
BACKEND_PID="${BACKEND_PID:-$(pgrep -n -f '/(hiddify-sing-box|xray) run -c .*/(singbox|xray)\.json' || true)}"
SOCKS_PORT="${SOCKS_PORT:-10808}"
KILL_SWITCH="${M3VPN_TUN_KILL_SWITCH:-0}"
[[ -n "$APP_PID" ]] || { echo "m3vpn app is not running" >&2; exit 1; }
[[ -n "$FRONT_PID" ]] || { echo "front proxy is not running" >&2; exit 1; }
[[ -n "$BACKEND_PID" ]] || { echo "proxy backend is not running (connect first)" >&2; exit 1; }

trap '"$HELPER" stop >/dev/null 2>&1 || true' EXIT

echo "app=$APP_PID front=$FRONT_PID backend=$BACKEND_PID socks=$SOCKS_PORT kill_switch=$KILL_SWITCH"
args=(start-tun --app-pid "$APP_PID" --front-pid "$FRONT_PID" --proxy-pid "$FRONT_PID" --proxy-pid "$BACKEND_PID" --socks-port "$SOCKS_PORT")
[[ "$KILL_SWITCH" == "1" ]] && args+=(--kill-switch)
"$HELPER" "${args[@]}"

for _ in $(seq 1 100); do
  ip link show m3vpn0 >/dev/null 2>&1 && break
  sleep 0.1
done
ip link show m3vpn0 >/dev/null
echo "TUN interface:"
ip -brief addr show m3vpn0

if ! ip rule show | grep -q "fwmark 0x2022 lookup main"; then
  echo "front bypass mark rule missing" >&2
  exit 1
fi
ip rule show | grep -E '8999|900[0-9]|9010' || true

echo "route:"; ip route get 1.1.1.1 || true
echo "DNS:"; getent hosts example.com | head -2
echo "HTTP via TUN:"; curl -fsS --max-time 20 https://api.ipify.org; echo
if getent ahostsv6 example.com >/dev/null 2>&1; then
  echo "IPv6 HTTP:"
  curl -6 -fsS --max-time 15 https://api64.ipify.org || echo "IPv6 unavailable"
fi
if [[ "$KILL_SWITCH" == "1" ]]; then
  echo "kill switch:"
  nft list table inet m3vpn | head -20
fi

"$HELPER" stop
trap - EXIT

if ip link show m3vpn0 >/dev/null 2>&1; then
  echo "m3vpn0 still exists after stop" >&2
  exit 1
fi
if ip rule show | grep -qE 'fwmark 0x2022|8999|900[0-9]|9010'; then
  echo "policy rules were not cleaned up" >&2
  exit 1
fi
if nft list table inet m3vpn >/dev/null 2>&1; then
  echo "kill switch table was not removed" >&2
  exit 1
fi

echo "Post-TUN HTTP:"; curl -fsS --max-time 15 https://api.ipify.org; echo
echo "TUN_LIVE_SMOKE_OK"
