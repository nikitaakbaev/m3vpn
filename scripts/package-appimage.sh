#!/usr/bin/env bash
# Build an AppImage from the staged package tree. AppImage runs in proxy mode;
# TUN still requires the system-wide m3vpn-helper package.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
STAGE="${1:-$ROOT/packaging/arch/stage}"
DIST="${2:-$ROOT/dist}"
VERSION="${3:-$(python3 -c 'import json; print(json.load(open("apps/desktop/src-tauri/tauri.conf.json"))["version"])')}"

[[ -d "$STAGE" ]] || { echo "missing stage: $STAGE" >&2; exit 1; }
mkdir -p "$DIST"
DIST="$(cd "$DIST" && pwd)"
STAGE="$(cd "$STAGE" && pwd)"

TOOL="$ROOT/.tools/appimagetool-x86_64.AppImage"
if [[ ! -x "$TOOL" ]]; then
  mkdir -p "$ROOT/.tools"
  curl -fL --retry 3 -o "$TOOL" \
    "https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-x86_64.AppImage"
  chmod +x "$TOOL"
fi

APPDIR="$ROOT/packaging/appimage/AppDir"
rm -rf "$APPDIR"
mkdir -p "$APPDIR/usr/bin" "$APPDIR/usr/lib/m3vpn" "$APPDIR/usr/share/applications"

install -Dm755 "$STAGE/usr/bin/m3vpn" "$APPDIR/usr/bin/m3vpn"
cp -a "$STAGE/usr/lib/m3vpn/sidecars" "$APPDIR/usr/lib/m3vpn/sidecars"
install -Dm644 "$STAGE/usr/share/applications/cc.m3vpn.desktop" \
  "$APPDIR/usr/share/applications/cc.m3vpn.desktop"
install -Dm644 "$STAGE/usr/share/applications/cc.m3vpn.desktop" \
  "$APPDIR/cc.m3vpn.desktop"
install -Dm644 "$STAGE/usr/share/icons/hicolor/512x512/apps/cc.m3vpn.desktop.png" \
  "$APPDIR/cc.m3vpn.desktop.png"

cat > "$APPDIR/AppRun" <<'SH'
#!/bin/sh
HERE="$(dirname "$(readlink -f "$0")")"
export M3VPN_SIDECARS="$HERE/usr/lib/m3vpn/sidecars"
exec "$HERE/usr/bin/m3vpn" "$@"
SH
chmod +x "$APPDIR/AppRun"

ARTIFACT="$DIST/m3vpn-$VERSION-x86_64.AppImage"
ARCH=x86_64 "$TOOL" --appimage-extract-and-run "$APPDIR" "$ARTIFACT" >/tmp/m3vpn-appimage.log 2>&1 || {
  tail -40 /tmp/m3vpn-appimage.log >&2
  exit 1
}
echo "==> AppImage: $ARTIFACT"
