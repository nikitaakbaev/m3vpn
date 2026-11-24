#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

VERSION="$(
  python3 -c 'import json; print(json.load(open("apps/desktop/src-tauri/tauri.conf.json"))["version"])'
)"
BASE_URL="${M3VPN_UPDATE_URL:-}"
if [[ -z "$BASE_URL" ]]; then
  echo "M3VPN_UPDATE_URL is required, e.g. https://updates.example.com/m3vpn" >&2
  exit 2
fi
BASE_URL="${BASE_URL%/}"
if [[ "$BASE_URL" != https://* && "$BASE_URL" != http://127.0.0.1* && "$BASE_URL" != http://localhost* ]]; then
  echo "updates must be served over HTTPS (or localhost for testing)" >&2
  exit 2
fi

if [[ "${M3VPN_SKIP_BUILD:-0}" != "1" ]]; then
  bash scripts/package-arch.sh
fi

ARCH_PKG="$ROOT/dist/m3vpn-$VERSION-1-x86_64.pkg.tar.zst"
PORTABLE="$ROOT/dist/m3vpn-$VERSION-linux-x86_64.tar.gz"
[[ -f "$ARCH_PKG" ]] || { echo "missing $ARCH_PKG" >&2; exit 1; }
[[ -f "$PORTABLE" ]] || { echo "missing $PORTABLE" >&2; exit 1; }

OUT="$ROOT/dist/update"
rm -rf "$OUT"
mkdir -p "$OUT"
cp "$ARCH_PKG" "$PORTABLE" "$OUT/"

ARCH_NAME="$(basename "$ARCH_PKG")"
PORTABLE_NAME="$(basename "$PORTABLE")"
ARCH_SHA="$(sha256sum "$ARCH_PKG" | awk '{print $1}')"
PORTABLE_SHA="$(sha256sum "$PORTABLE" | awk '{print $1}')"
ARCH_SIZE="$(stat -c '%s' "$ARCH_PKG")"
PORTABLE_SIZE="$(stat -c '%s' "$PORTABLE")"
NOTES="${M3VPN_UPDATE_NOTES:-m3vpn $VERSION}"

python3 - "$OUT/latest.json" "$VERSION" "$BASE_URL" "$ARCH_NAME" "$ARCH_SHA" "$ARCH_SIZE" \
  "$PORTABLE_NAME" "$PORTABLE_SHA" "$PORTABLE_SIZE" "$NOTES" <<'PY'
import json
import sys
from datetime import datetime, timezone

out, version, base, arch_name, arch_sha, arch_size, portable_name, portable_sha, portable_size, notes = sys.argv[1:]
manifest = {
    "version": version,
    "publishedAt": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
    "notes": notes,
    "artifacts": {
        "arch": {
            "url": f"{base}/{arch_name}",
            "sha256": arch_sha,
            "size": int(arch_size),
        },
        "portable": {
            "url": f"{base}/{portable_name}",
            "sha256": portable_sha,
            "size": int(portable_size),
        },
    },
}
with open(out, "w", encoding="utf-8") as handle:
    json.dump(manifest, handle, ensure_ascii=False, indent=2)
    handle.write("\n")
PY

(
  cd "$OUT"
  sha256sum ./* > sha256sums.txt
)

if [[ -n "${M3VPN_UPDATE_HOST:-}" ]]; then
  : "${M3VPN_UPDATE_PATH:?M3VPN_UPDATE_PATH is required with M3VPN_UPDATE_HOST}"
  rsync -av --chmod=F644,D755 "$OUT/" "${M3VPN_UPDATE_HOST}:${M3VPN_UPDATE_PATH}/"
  echo "uploaded to ${M3VPN_UPDATE_HOST}:${M3VPN_UPDATE_PATH}"
fi

echo
echo "manifest: $OUT/latest.json"
cat "$OUT/latest.json"
echo
echo "public URL: $BASE_URL/latest.json"
