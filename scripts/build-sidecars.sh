#!/usr/bin/env bash
# Build the M0 sidecars reproducibly: hiddify-sing-box (with_awg) and Xray-core,
# plus the geo assets used by the Xray routing tests.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TOOLS="$ROOT/.tools"
BIN="$ROOT/sidecars/bin"
ASSETS="$ROOT/sidecars/assets"

GO_VERSION="${GO_VERSION:-1.27.4}"
SING_BOX_REPO="${SING_BOX_REPO:-https://github.com/hiddify/hiddify-sing-box.git}"
SING_BOX_BRANCH="${SING_BOX_BRANCH:-v5}"
SING_BOX_COMMIT="${SING_BOX_COMMIT:-1267fe6aac8d11f3a562e8feecad11d11f48f965}"
XRAY_VERSION="${XRAY_VERSION:-v26.11.9}"

mkdir -p "$TOOLS" "$BIN" "$ASSETS"

# --- Go --------------------------------------------------------------------
if [ ! -x "$TOOLS/go/bin/go" ]; then
  archive="$TOOLS/go${GO_VERSION}.linux-amd64.tar.gz"
  curl -fL --retry 3 -o "$archive" "https://go.dev/dl/go${GO_VERSION}.linux-amd64.tar.gz"
  tar -xzf "$archive" -C "$TOOLS"
  rm -f "$archive"
fi
export PATH="$TOOLS/go/bin:$PATH"
export GOTOOLCHAIN=local
echo "go: $($TOOLS/go/bin/go version)"

# --- source ----------------------------------------------------------------
SRC="$TOOLS/hiddify-sing-box"
if [ ! -d "$SRC/.git" ]; then
  git clone --depth 1 --branch "$SING_BOX_BRANCH" "$SING_BOX_REPO" "$SRC"
fi
git -C "$SRC" fetch --depth 1 origin "$SING_BOX_COMMIT" || true
git -C "$SRC" checkout -q "$SING_BOX_COMMIT" 2>/dev/null || git -C "$SRC" checkout -q "$SING_BOX_BRANCH"

# Submodules: the superproject's recorded commits are sometimes force-pushed away.
# Fall back to the branch tip, then pin what we actually used in versions.json.
git -C "$SRC" submodule update --init --recursive --depth 1 || true

pin_submodule() {
  local path="$1" branch="$2" commit="$3"
  local dir="$SRC/$path"
  [ -d "$dir" ] || return 0
  git -C "$dir" fetch --depth 50 origin "$branch" >/dev/null 2>&1 || true
  if git -C "$dir" cat-file -e "$commit^{commit}" 2>/dev/null; then
    git -C "$dir" checkout -q "$commit"
  else
    git -C "$dir" checkout -q FETCH_HEAD
    echo "warning: $path pinned commit unavailable, using $branch tip $(git -C "$dir" rev-parse --short HEAD)" >&2
  fi
}

pin_submodule replace/sing            v5      b40ea077780dbc1babef7d035d4b0fea8d9ad6f2
pin_submodule replace/sing-mux        v5      ea35b38fa88181b81914150e9d349e9014645e0d
pin_submodule replace/soar            hiddify e18587bc9fb62f326124d1f3eb0ceb31708bd077
pin_submodule replace/sing-tun        dev     fe80f0918269863a13628e53e4557b038882504f
pin_submodule replace/sing-vmess      dev     979150fe9f26c5f4df9c4a9c7e21f2dbec43120a
pin_submodule replace/gvisor          go      5618c875fd006f0f1d9b92bc2b16392fb6babc9a
pin_submodule replace/amneziawg-go    v5      a2ae165a1b3f062ce9dd3dc3353ebd1c591847a5
pin_submodule replace/tailscale       dev     4138b20dfb9f5852d043abd8bf9da6c29d5ee53b
pin_submodule replace/wireguard-go    v5      32f4b5369c2dff702384168612f83355a09ec177
pin_submodule replace/MasterDnsVPN    hiddify a6b693698ee8246ca415a994bb6da40d66dddba7
pin_submodule replace/psiphon-tls     master  3e697502354da72c121c7eb5266f3819bc8f91bd
pin_submodule replace/psiphon-quic-go master  c43fd98d13a8b954ef23fd51c68b238a8d8a780e

# The hiddify soar branch exposes Discovery.List() but not Conn.Abort().
if grep -q 'c.Conn.Abort()' "$SRC/protocol/hiddify/soar/outbound.go" 2>/dev/null; then
  sed -i 's/c\.Conn\.Abort()/c.Conn.Close()/' "$SRC/protocol/hiddify/soar/outbound.go"
  echo "patched: soar Conn.Abort -> Conn.Close"
fi

# --- build -----------------------------------------------------------------
(
  cd "$SRC"
  make build
)
cp "$SRC/sing-box" "$BIN/hiddify-sing-box"
chmod +x "$BIN/hiddify-sing-box"
"$BIN/hiddify-sing-box" version | head -1

# --- Xray ------------------------------------------------------------------
if [ ! -x "$BIN/xray" ]; then
  tmp="$(mktemp -d)"
  curl -fL --retry 3 -o "$tmp/xray.zip" \
    "https://github.com/XTLS/Xray-core/releases/download/${XRAY_VERSION}/Xray-linux-64.zip"
  unzip -q "$tmp/xray.zip" -d "$tmp"
  cp "$tmp/xray" "$BIN/xray"
  chmod +x "$BIN/xray"
  rm -rf "$tmp"
fi
"$BIN/xray" version | head -1

# --- geo assets ------------------------------------------------------------
for f in geoip.dat geosite.dat; do
  if [ ! -f "$ASSETS/$f" ]; then
    curl -fL --retry 3 -o "$ASSETS/$f" \
      "https://github.com/Loyalsoldier/v2ray-rules-dat/releases/latest/download/$f"
  fi
done

echo "--- sha256 ---"
sha256sum "$BIN/hiddify-sing-box" "$BIN/xray" "$ASSETS/geoip.dat" "$ASSETS/geosite.dat"
