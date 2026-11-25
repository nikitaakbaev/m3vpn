#!/usr/bin/env bash
# Fill packaging/aur/PKGBUILD from a downloaded release SHA256SUMS.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VERSION="${1:-$(python3 -c 'import json; print(json.load(open("apps/desktop/src-tauri/tauri.conf.json"))["version"])')}"
SUMS="${2:-$ROOT/dist/SHA256SUMS}"
ASSET="m3vpn-${VERSION}-1-x86_64.pkg.tar.zst"
SUM="$(awk -v name="$ASSET" '$2 == name { print $1 }' "$SUMS")"
[[ -n "$SUM" ]] || { echo "missing $ASSET in $SUMS" >&2; exit 1; }
sed -i "s/^pkgver=.*/pkgver=$VERSION/" "$ROOT/packaging/aur/PKGBUILD"
sed -i "s/^sha256sums=.*/sha256sums=('$SUM')/" "$ROOT/packaging/aur/PKGBUILD"
echo "AUR PKGBUILD updated for $VERSION ($SUM)"
