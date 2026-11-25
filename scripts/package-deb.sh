#!/usr/bin/env bash
# Build a Debian package from the shared staged tree without dpkg tooling.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
STAGE="${1:-$ROOT/packaging/arch/stage}"
DIST="${2:-$ROOT/dist}"
VERSION="${3:-$(python3 -c 'import json; print(json.load(open("apps/desktop/src-tauri/tauri.conf.json"))["version"])')}"

[[ -d "$STAGE" ]] || { echo "missing stage: $STAGE" >&2; exit 1; }
mkdir -p "$DIST"
DIST="$(cd "$DIST" && pwd)"
STAGE="$(cd "$STAGE" && pwd)"

arch="$(uname -m)"
case "$arch" in
  x86_64) deb_arch="amd64" ;;
  aarch64) deb_arch="arm64" ;;
  *) echo "unsupported architecture: $arch" >&2; exit 1 ;;
esac

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
mkdir -p "$work/control" "$work/data"
cp -a "$STAGE/." "$work/data/"

cat > "$work/control/control" <<EOF
Package: m3vpn
Version: $VERSION-1
Section: net
Priority: optional
Architecture: $deb_arch
Maintainer: m3vpn <m3vpn@localhost>
Installed-Size: $(du -sk "$work/data" | cut -f1)
Depends: libgtk-3-0, libwebkit2gtk-4.1-0, libayatana-appindicator3-1
Recommends: network-manager
Description: Material 3 VPN client for Linux
 m3vpn is a V2Ray/Xray and sing-box client with subscriptions,
 TUN mode, routing profiles and GitHub-based updates.
EOF

(
  cd "$work/data"
  find . -type f | sed 's|^\./||' | LC_ALL=C sort | while IFS= read -r file; do
    printf '%s  %s\n' "$(md5sum "$file" | cut -d' ' -f1)" "$file"
  done
) > "$work/control/md5sums"

cat > "$work/control/postinst" <<'SCRIPT'
#!/bin/sh
set -e
if ! getent group m3vpn >/dev/null; then
  groupadd --system m3vpn
fi
systemctl daemon-reload >/dev/null 2>&1 || true
systemctl enable m3vpn-helper.service >/dev/null 2>&1 || true
systemctl restart m3vpn-helper.service >/dev/null 2>&1 || true
if [ -n "${SUDO_USER:-}" ] && id "$SUDO_USER" >/dev/null 2>&1; then
  usermod -aG m3vpn "$SUDO_USER" || true
fi
exit 0
SCRIPT
cat > "$work/control/prerm" <<'SCRIPT'
#!/bin/sh
set -e
systemctl disable --now m3vpn-helper.service >/dev/null 2>&1 || true
exit 0
SCRIPT
cat > "$work/control/postrm" <<'SCRIPT'
#!/bin/sh
set -e
systemctl daemon-reload >/dev/null 2>&1 || true
exit 0
SCRIPT
chmod 0755 "$work/control/postinst" "$work/control/prerm" "$work/control/postrm"

tar --owner=0 --group=0 --numeric-owner -C "$work/control" -czf "$work/control.tar.gz" .
tar --owner=0 --group=0 --numeric-owner -C "$work/data" -czf "$work/data.tar.gz" .
printf '2.0\n' > "$work/debian-binary"

artifact="$DIST/m3vpn_${VERSION}-1_${deb_arch}.deb"
rm -f "$artifact"
(
  cd "$work"
  ar rcD "$artifact" debian-binary control.tar.gz data.tar.gz
)
echo "==> Debian package: $artifact"
