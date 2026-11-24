#!/usr/bin/env bash
# M0 spike: prove that TUN state can be restored after a crash.
# Runs entirely inside an isolated user+network namespace, so the host is untouched.
set -euo pipefail

unshare -Urn bash -s <<'INNER'
set -euo pipefail

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

# --- baseline system state -------------------------------------------------
ip link set lo up
ip link add dum0 type dummy
ip addr add 10.10.0.1/24 dev dum0
ip link set dum0 up
ip route add 10.20.0.0/24 via 10.10.0.1
nft add table inet baseline
nft add chain inet baseline out '{ type filter hook output priority 0; }'
nft add rule inet baseline out meta mark 777 accept

ip route save table main > "$WORK/routes.before"
nft list ruleset > "$WORK/nft.before"
ip -j route show table all | jq -S 'sort_by(.dst, .dev)' > "$WORK/routes.before.json"

echo "baseline routes: $(jq length "$WORK/routes.before.json")"

# --- a process sets up a tunnel and then dies hard -------------------------
(
  ip tuntap add dev tun9 mode tun
  ip addr add 172.19.0.1/30 dev tun9
  ip link set tun9 up
  ip route add default dev tun9
  nft add table inet m3vpn
  nft add chain inet m3vpn out '{ type filter hook output priority 0; }'
  nft add rule inet m3vpn out drop
  echo "tunnel set up, simulating SIGKILL" >&2
  kill -9 "$BASHPID"
) &
CRASHED=$!
wait "$CRASHED" 2>/dev/null || true

echo "after crash, default route: $(ip route show default || true)"
echo "after crash, nft tables: $(nft list tables | tr '\n' ' ')"

# --- restore (what the helper / systemd ExecStopPost will do) --------------
ip link del tun9 2>/dev/null || true
nft delete table inet m3vpn 2>/dev/null || true
ip route flush table main
ip route restore table main < "$WORK/routes.before"
nft flush ruleset
nft -f "$WORK/nft.before"

# --- verify ----------------------------------------------------------------
ip -j route show table all | jq -S 'sort_by(.dst, .dev)' > "$WORK/routes.after.json"
nft list ruleset > "$WORK/nft.after"

echo "restored routes: $(jq length "$WORK/routes.after.json")"

fail=0
if ! diff -u "$WORK/routes.before.json" "$WORK/routes.after.json"; then
  echo "ROUTES DIFFER" >&2
  fail=1
fi
if ! diff -u <(grep -v '^#' "$WORK/nft.before" | sed '/^$/d') <(grep -v '^#' "$WORK/nft.after" | sed '/^$/d'); then
  echo "NFT DIFFERS" >&2
  fail=1
fi

if ip link show tun9 >/dev/null 2>&1; then
  echo "tun9 still exists" >&2
  fail=1
fi

if [ "$fail" -eq 0 ]; then
  echo "TUN_RESTORE_OK"
else
  echo "TUN_RESTORE_FAIL"
  exit 1
fi
INNER
