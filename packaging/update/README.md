# Update server

m3vpn checks a small static manifest and never installs anything by itself.
The package manager remains the trust and installation boundary.

## Server layout

```text
/var/www/updates/m3vpn/
  latest.json
  sha256sums.txt
  m3vpn-0.1.0-1-x86_64.pkg.tar.zst
  m3vpn-0.1.0-linux-x86_64.tar.gz
```

Example `latest.json`:

```json
{
  "version": "0.1.0",
  "publishedAt": "2026-11-24T23:00:00Z",
  "notes": "TUN fixes and INGY Ping",
  "artifacts": {
    "arch": {
      "url": "https://updates.example.com/m3vpn/m3vpn-0.1.0-1-x86_64.pkg.tar.zst",
      "sha256": "…",
      "size": 56623104
    },
    "portable": {
      "url": "https://updates.example.com/m3vpn/m3vpn-0.1.0-linux-x86_64.tar.gz",
      "sha256": "…",
      "size": 63963136
    }
  }
}
```

## Publish

```bash
M3VPN_UPDATE_URL="https://updates.example.com/m3vpn" \
M3VPN_UPDATE_HOST="user@updates.example.com" \
M3VPN_UPDATE_PATH="/var/www/updates/m3vpn" \
M3VPN_UPDATE_NOTES="TUN fixes and INGY Ping" \
bash scripts/publish-update.sh
```

`scripts/publish-update.sh` builds packages, writes `dist/update/latest.json`,
adds SHA-256 sums, and optionally uploads the directory with `rsync`.

Use `M3VPN_SKIP_BUILD=1` to publish already-built artifacts.

## Nginx

See [`nginx.conf.example`](nginx.conf.example). `latest.json` must not be
cached; artifacts can be cached for a day.

Caddy equivalent:

```caddyfile
updates.example.com {
    root * /var/www/updates
    file_server
    header /m3vpn/latest.json Cache-Control "no-store"
}
```

## Security

- HTTPS is required by the app. Plain HTTP is accepted only for
  `127.0.0.1`/`localhost` during local testing.
- Every artifact in the manifest must contain a 64-character SHA-256.
- The app opens artifact URLs in the system browser; it does not execute
  installers or `pacman`.
- Keep old artifact files until clients have upgraded. The publish script
  does not delete remote files.
