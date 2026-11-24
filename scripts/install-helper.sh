#!/usr/bin/env bash
# Install the privileged TUN helper and enable its systemd service.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
USER_NAME="${SUDO_USER:-${USER:-}}"

if [ "$(id -u)" -ne 0 ]; then
  echo "run as root: sudo $0" >&2
  exit 1
fi
if [ ! -x "$ROOT/sidecars/bin/hiddify-sing-box" ]; then
  echo "sidecars are missing; run scripts/build-sidecars.sh first" >&2
  exit 1
fi

echo "building m3vpn-helper (release)"
(cd "$ROOT/crates/helper" && cargo build --release)

install -d -m 755 /usr/lib/m3vpn
install -m 755 "$ROOT/crates/helper/target/release/m3vpn-helper" /usr/lib/m3vpn/m3vpn-helper
rm -rf /usr/lib/m3vpn/sidecars
cp -a "$ROOT/sidecars" /usr/lib/m3vpn/sidecars

install -m 644 "$ROOT/packaging/systemd/m3vpn-helper.service" /etc/systemd/system/m3vpn-helper.service

if ! getent group m3vpn >/dev/null; then
  groupadd --system m3vpn
fi
if [ -n "$USER_NAME" ] && [ "$USER_NAME" != "root" ]; then
  usermod -aG m3vpn "$USER_NAME"
fi

systemctl daemon-reload
systemctl enable --now m3vpn-helper.service
systemctl --no-pager --full status m3vpn-helper.service || true

echo
echo "m3vpn-helper installed."
echo "Group membership was updated for ${USER_NAME:-the current user}; log out and back in, then restart the app."
