#!/usr/bin/env bash
# Verify that the sidecars on disk match the hashes pinned in sidecars/versions.json.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VERSIONS="$ROOT/sidecars/versions.json"
fail=0

check() {
  local path="$1" expected="$2" label="$3"
  local absolute="$ROOT/$path"
  if [ ! -f "$absolute" ]; then
    echo "MISSING  $label ($path)"
    fail=1
    return
  fi
  local actual
  actual="$(sha256sum "$absolute" | cut -d' ' -f1)"
  if [ "$actual" = "$expected" ]; then
    echo "OK       $label"
  else
    echo "MISMATCH $label"
    echo "         expected $expected"
    echo "         actual   $actual"
    fail=1
  fi
}

check "sidecars/bin/hiddify-sing-box" "$(jq -r '.sing_box.binary.sha256' "$VERSIONS")" "hiddify-sing-box"
check "sidecars/bin/xray" "$(jq -r '.xray.binary.sha256' "$VERSIONS")" "xray"
check "sidecars/assets/geoip.dat" "$(jq -r '.geo_assets.files["geoip.dat"].sha256' "$VERSIONS")" "geoip.dat"
check "sidecars/assets/geosite.dat" "$(jq -r '.geo_assets.files["geosite.dat"].sha256' "$VERSIONS")" "geosite.dat"

if ! "$ROOT/sidecars/bin/hiddify-sing-box" version 2>/dev/null | grep -q 'with_awg'; then
  echo "MISSING  with_awg build tag in hiddify-sing-box"
  fail=1
fi

if [ "$fail" -ne 0 ]; then
  echo "sidecars are not reproducible with the pinned versions"
  exit 1
fi

echo "all sidecars match sidecars/versions.json"
