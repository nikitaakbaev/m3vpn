#!/usr/bin/env bash
# Build an RPM from the shared staged tree. Requires rpmbuild (rpm-tools).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
STAGE="${1:-$ROOT/packaging/arch/stage}"
DIST="${2:-$ROOT/dist}"
VERSION="${3:-$(python3 -c 'import json; print(json.load(open("apps/desktop/src-tauri/tauri.conf.json"))["version"])')}"

command -v rpmbuild >/dev/null 2>&1 || {
  echo "rpmbuild is not installed; skipping RPM" >&2
  exit 0
}

[[ -d "$STAGE" ]] || { echo "missing stage: $STAGE" >&2; exit 1; }
mkdir -p "$DIST"
DIST="$(cd "$DIST" && pwd)"
STAGE="$(cd "$STAGE" && pwd)"

TOP="$ROOT/packaging/rpm/rpmbuild"
rm -rf "$TOP"
mkdir -p "$TOP"/{BUILD,BUILDROOT,RPMS,SOURCES,SPECS,SRPMS}
sed "s/@VERSION@/$VERSION/" "$ROOT/packaging/rpm/m3vpn.spec" > "$TOP/SPECS/m3vpn.spec"

rpmbuild -bb \
  --define "_topdir $TOP" \
  --define "stage $STAGE" \
  "$TOP/SPECS/m3vpn.spec" >/tmp/m3vpn-rpm.log 2>&1 || {
    tail -40 /tmp/m3vpn-rpm.log >&2
    exit 1
  }

cp "$TOP"/RPMS/*/m3vpn-"$VERSION"-1*.rpm "$DIST"/
echo "==> RPM package: $DIST/m3vpn-$VERSION-1.x86_64.rpm"
