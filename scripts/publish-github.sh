#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

VERSION="$(
  python3 -c 'import json; print(json.load(open("apps/desktop/src-tauri/tauri.conf.json"))["version"])'
)"
REPO="${GITHUB_REPO:-}"
TOKEN="${GITHUB_TOKEN:-}"
TAG="v$VERSION"
NOTES="${M3VPN_RELEASE_NOTES:-m3vpn $VERSION}"

if [[ -z "$REPO" ]]; then
  echo "GITHUB_REPO is required, e.g. owner/m3vpn" >&2
  exit 2
fi
if [[ ! "$REPO" =~ ^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+$ ]]; then
  echo "GITHUB_REPO must look like owner/repo" >&2
  exit 2
fi
if [[ -z "$TOKEN" ]]; then
  echo "GITHUB_TOKEN is required (repo scope for a public release)" >&2
  exit 2
fi

if [[ "${M3VPN_SKIP_BUILD:-0}" != "1" ]]; then
  bash scripts/package-arch.sh
fi

ARCH_PKG="$ROOT/dist/m3vpn-$VERSION-1-x86_64.pkg.tar.zst"
PORTABLE="$ROOT/dist/m3vpn-$VERSION-linux-x86_64.tar.gz"
[[ -f "$ARCH_PKG" ]] || { echo "missing $ARCH_PKG" >&2; exit 1; }
[[ -f "$PORTABLE" ]] || { echo "missing $PORTABLE" >&2; exit 1; }

API="https://api.github.com/repos/$REPO/releases"
AUTH_HEADER="$(mktemp)"
trap 'rm -f "$AUTH_HEADER"' EXIT
chmod 600 "$AUTH_HEADER"
printf 'Authorization: Bearer %s\n' "$TOKEN" >"$AUTH_HEADER"
AUTH=(-H @"$AUTH_HEADER")
JSON=(-H "Accept: application/vnd.github+json" -H "X-GitHub-Api-Version: 2022-11-28")

payload="$(
  python3 - "$TAG" "$NOTES" <<'PY'
import json
import sys
tag, notes = sys.argv[1:]
print(json.dumps({
    "tag_name": tag,
    "name": tag,
    "body": notes,
    "draft": False,
    "prerelease": False,
}))
PY
)"

echo "==> create release $TAG in $REPO"
response="$(curl -sS -w '\n%{http_code}' -X POST \
  "${AUTH[@]}" "${JSON[@]}" \
  -H "Content-Type: application/json" \
  --data "$payload" "$API")"
status="$(tail -n1 <<<"$response")"
body="$(sed '$d' <<<"$response")"
if [[ "$status" != "201" ]]; then
  echo "GitHub release creation failed (HTTP $status):" >&2
  echo "$body" >&2
  exit 1
fi

release_id="$(python3 -c 'import json,sys; print(json.load(sys.stdin)["id"])' <<<"$body")"
echo "release id: $release_id"

upload() {
  local file="$1"
  local name
  name="$(basename "$file")"
  echo "==> upload $name"
  local response status
  response="$(curl -sS -w '\n%{http_code}' -X POST \
    "${AUTH[@]}" "${JSON[@]}" \
    -H "Content-Type: application/octet-stream" \
    --data-binary "@$file" \
    "https://uploads.github.com/repos/$REPO/releases/$release_id/assets?name=$name")"
  status="$(tail -n1 <<<"$response")"
  if [[ "$status" != "201" ]]; then
    echo "upload of $name failed (HTTP $status):" >&2
    sed '$d' <<<"$response" >&2
    exit 1
  fi
}

upload "$ARCH_PKG"
upload "$PORTABLE"

echo
echo "release: https://github.com/$REPO/releases/tag/$TAG"
echo "app settings: update.github_repo = $REPO"
