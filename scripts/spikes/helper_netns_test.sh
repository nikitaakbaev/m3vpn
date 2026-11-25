#!/usr/bin/env bash
# Test m3vpn-helper inside an isolated user+network namespace.
# No host network changes are made.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
HELPER="$ROOT/crates/helper/target/debug/m3vpn-helper"
SINGBOX="$ROOT/sidecars/bin/hiddify-sing-box"

[ -x "$HELPER" ] || { echo "build helper first: (cd crates/helper && cargo build)"; exit 1; }
[ -x "$SINGBOX" ] || { echo "missing $SINGBOX"; exit 1; }

unshare -Urn bash -s <<INNER
set -euo pipefail
ROOT="$ROOT"
HELPER="$HELPER"
SINGBOX="$SINGBOX"
WORK="\$(mktemp -d)"
export M3VPN_RUN_DIR="\$WORK/run"
export M3VPN_HELPER_SOCKET="\$WORK/run/helper.sock"
export M3VPN_SINGBOX="\$SINGBOX"
mkdir -p "\$M3VPN_RUN_DIR"

cleanup() {
  "\$HELPER" stop >/dev/null 2>&1 || true
  [ -n "\${HELPER_PID:-}" ] && kill "\$HELPER_PID" 2>/dev/null || true
  [ -n "\${FRONT_PID:-}" ] && kill "\$FRONT_PID" 2>/dev/null || true
  rm -rf "\$WORK"
}
trap cleanup EXIT

ip link set lo up
ip link add dummy0 type dummy
ip addr add 10.10.0.1/24 dev dummy0
ip link set dummy0 up
ip route add default dev dummy0

cat > "\$WORK/front.json" <<'JSON'
{
  "log": { "level": "error" },
  "inbounds": [{
    "type": "mixed",
    "tag": "front-in",
    "listen": "127.0.0.1",
    "listen_port": 10808
  }],
  "outbounds": [{ "type": "direct", "tag": "direct" }],
  "route": { "final": "direct" }
}
JSON
"$SINGBOX" run -c "\$WORK/front.json" >"\$WORK/front.log" 2>&1 &
FRONT_PID=\$!
for _ in \$(seq 1 50); do
  if (exec 3<>/dev/tcp/127.0.0.1/10808) 2>/dev/null; then exec 3>&-; break; fi
  sleep 0.1
done

"$HELPER" run >"\$WORK/helper.log" 2>&1 &
HELPER_PID=\$!
for _ in \$(seq 1 50); do
  [ -S "\$M3VPN_HELPER_SOCKET" ] && break
  sleep 0.1
done

ip -j route show table all | jq -S 'sort_by(.dst, .dev)' > "\$WORK/routes.before"
ip rule show > "\$WORK/rules.before"
SELF_CGROUP="\$(sed -n 's/^0:://p' /proc/self/cgroup)"
"$HELPER" start-tun --app-pid \$\$ --front-pid \$FRONT_PID --proxy-pid \$FRONT_PID --split-cgroup "\$SELF_CGROUP" --socks-port 10808 >/dev/null

if ! ip link show m3vpn0 >/dev/null 2>&1; then
  echo "TUN interface missing" >&2
  cat "\$WORK/run/tun.log" >&2 || true
  exit 1
fi
echo "TUN interface and policy rules:"
ip rule show
if ! ip rule show | grep -q "fwmark 0x2022 lookup main"; then
  echo "front bypass rule is missing" >&2
  exit 1
fi
if ! nft list table inet m3vpn_tun 2>/dev/null | grep -q 'iifname "m3vpn0" accept'; then
  echo "TUN input accept rule is missing" >&2
  exit 1
fi
if ! nft list table inet m3vpn_split 2>/dev/null | grep -q 'meta mark set 0x00002022'; then
  echo "split tunneling rule is missing" >&2
  exit 1
fi

if ! timeout 10s "$HELPER" stop; then
  echo "helper stop failed" >&2
  cat "\$WORK/helper.log" >&2 || true
  cat "\$WORK/run/tun.log" >&2 || true
  exit 1
fi

if ip link show m3vpn0 >/dev/null 2>&1; then
  echo "TUN interface still exists after stop" >&2
  exit 1
fi
if nft list table inet m3vpn_tun >/dev/null 2>&1; then
  echo "TUN input table remains after stop" >&2
  exit 1
fi
if nft list table inet m3vpn_split >/dev/null 2>&1; then
  echo "split table remains after stop" >&2
  exit 1
fi

ip -j route show table all | jq -S 'sort_by(.dst, .dev)' > "\$WORK/routes.after"
ip rule show > "\$WORK/rules.after"
if ! diff -u "\$WORK/routes.before" "\$WORK/routes.after"; then
  echo "routes were not restored" >&2
  exit 1
fi
if ! diff -u "\$WORK/rules.before" "\$WORK/rules.after"; then
  echo "ip rules were not restored" >&2
  exit 1
fi

"$HELPER" start-tun --app-pid \$\$ --front-pid \$FRONT_PID --proxy-pid \$FRONT_PID --socks-port 10808 --kill-switch >/dev/null
for _ in \$(seq 1 100); do
  ip link show m3vpn0 >/dev/null 2>&1 && break
  sleep 0.1
done
ip link show m3vpn0 >/dev/null
if ! nft list table inet m3vpn 2>/dev/null | grep -q "0x00002022"; then
  echo "kill switch does not allow the front mark" >&2
  exit 1
fi
"$HELPER" stop >/dev/null
if nft list table inet m3vpn >/dev/null 2>&1; then
  echo "kill switch table remains after stop" >&2
  exit 1
fi
if ip rule show | grep -qE "fwmark 0x2022|8999|900[0-9]|9010"; then
  echo "policy rules remain after kill-switch stop" >&2
  exit 1
fi

echo "HELPER_NETNS_OK"
INNER
