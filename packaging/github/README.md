# GitHub Releases

m3vpn checks the latest public GitHub Release and opens release assets in the
browser. It never installs packages by itself.

## Repository

Use a **public** repository. The app reads:

```text
GET https://api.github.com/repos/<owner>/<repo>/releases/latest
```

No token is stored in the client. Public releases use the anonymous GitHub API
rate limit (60 requests per hour per IP); m3vpn checks at startup and every
6 hours.

## Prepare a release

```bash
export GITHUB_REPO="nikitaakbaev/m3vpn"
export GITHUB_TOKEN="github_pat_..."
export M3VPN_RELEASE_NOTES="TUN fixes, Fast HTTP ping, update checker"
bash scripts/publish-github.sh
```

The script builds the Arch package and portable archive, creates tag
`v<version>`, and uploads:

```text
m3vpn-0.2.0-1-x86_64.pkg.tar.zst
m3vpn-0.2.0-linux-x86_64.tar.gz
```

The tag must not already exist. Bump `apps/desktop/src-tauri/tauri.conf.json`
before publishing a new version.

GitHub computes SHA-256 digests for release assets. The app displays them in
the artifact tooltip and requires a valid digest for every asset.

## App settings

Set:

```text
Настройки → Обновления → GitHub-репозиторий: nikitaakbaev/m3vpn
```

The latest release must be public and non-draft. The app strips a leading `v`
from the tag and compares numeric versions.

## Install on Arch

```bash
curl -LO https://github.com/nikitaakbaev/m3vpn/releases/latest/download/m3vpn-0.2.0-1-x86_64.pkg.tar.zst
sudo pacman -U m3vpn-0.2.0-1-x86_64.pkg.tar.zst
```

For a private repository, use a self-hosted update endpoint instead; the
client intentionally does not store GitHub tokens.
