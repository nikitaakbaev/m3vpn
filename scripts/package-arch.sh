#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

VERSION="$(
  python3 -c 'import json; print(json.load(open("apps/desktop/src-tauri/tauri.conf.json"))["version"])'
)"
ICON_DIR="$ROOT/apps/desktop/src-tauri/icons"
STAGE="$ROOT/packaging/arch/stage"
DIST="$ROOT/dist"

echo "==> m3vpn $VERSION"
bash scripts/check-sidecars.sh >/dev/null

echo "==> release app"
(
  cd apps/desktop
  npm run tauri build -- --no-bundle
)

echo "==> release helper"
HELPER_TARGET="$ROOT/packaging/arch/helper-target"
(
  cd crates/helper
  CARGO_TARGET_DIR="$HELPER_TARGET" cargo build --release
)

APP_BIN="$ROOT/apps/desktop/src-tauri/target/release/m3vpn"
HELPER_BIN="$HELPER_TARGET/release/m3vpn-helper"
[[ -x "$APP_BIN" ]] || { echo "missing $APP_BIN" >&2; exit 1; }
[[ -x "$HELPER_BIN" ]] || { echo "missing $HELPER_BIN" >&2; exit 1; }

echo "==> stage package tree"
rm -rf "$STAGE"
install -Dm755 "$APP_BIN" "$STAGE/usr/bin/m3vpn"
install -Dm755 "$HELPER_BIN" "$STAGE/usr/lib/m3vpn/m3vpn-helper"
cp -a "$ROOT/sidecars" "$STAGE/usr/lib/m3vpn/sidecars"
install -Dm644 "$ROOT/packaging/systemd/m3vpn-helper.service" \
  "$STAGE/usr/lib/systemd/system/m3vpn-helper.service"
install -Dm644 "$ROOT/packaging/linux/cc.m3vpn.desktop" \
  "$STAGE/usr/share/applications/cc.m3vpn.desktop"
install -Dm644 "$ROOT/packaging/linux/cc.m3vpn.desktop.metainfo.xml" \
  "$STAGE/usr/share/metainfo/cc.m3vpn.desktop.metainfo.xml"
install -Dm644 "$ROOT/LICENSE" \
  "$STAGE/usr/share/licenses/m3vpn/LICENSE"
install -Dm644 "$ROOT/packaging/linux/THIRD-PARTY.md" \
  "$STAGE/usr/share/doc/m3vpn/THIRD-PARTY.md"
install -Dm644 "$ROOT/.tools/hiddify-sing-box/LICENSE" \
  "$STAGE/usr/share/licenses/m3vpn/LICENSE.sing-box"
for size in 32x32 128x128 512x512; do
  install -Dm644 "$ICON_DIR/$size.png" \
    "$STAGE/usr/share/icons/hicolor/$size/apps/cc.m3vpn.desktop.png"
done

desktop-file-validate "$ROOT/packaging/linux/cc.m3vpn.desktop"
if command -v appstreamcli >/dev/null 2>&1; then
  appstreamcli validate --no-net "$ROOT/packaging/linux/cc.m3vpn.desktop.metainfo.xml" >/dev/null
fi

echo "==> portable tarball"
mkdir -p "$DIST"
tar -C "$STAGE" -czf "$DIST/m3vpn-$VERSION-linux-x86_64.tar.gz" .

echo "==> Debian package"
bash scripts/package-deb.sh "$STAGE" "$DIST" "$VERSION"

echo "==> Arch package"
sed -i "s/^pkgver=.*/pkgver=$VERSION/" "$ROOT/packaging/arch/PKGBUILD"
(
  cd "$ROOT/packaging/arch"
  rm -f m3vpn-*.pkg.tar.zst
  makepkg -f --nodeps
  cp m3vpn-*.pkg.tar.zst "$DIST/"
)

echo "==> checksums"
(
  cd "$DIST"
  sha256sum m3vpn-*.pkg.tar.zst m3vpn-*.tar.gz m3vpn_*.deb > SHA256SUMS
)

echo "==> artifacts"
ls -lh "$DIST"
