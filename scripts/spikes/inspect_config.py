#!/usr/bin/env python3
"""Dump a sanitized structural view of one full Xray config from the subscription."""
import json
import os
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
RAW = ROOT / ".secrets" / "subscription.raw"
OUT = ROOT / ".secrets" / "xray-test.json"

SENSITIVE = {"id", "password", "publicKey", "privateKey", "shortId", "uuid", "token", "key"}


def redact(obj):
    if isinstance(obj, dict):
        return {k: ("<redacted>" if k in SENSITIVE else redact(v)) for k, v in obj.items()}
    if isinstance(obj, list):
        return [redact(v) for v in obj]
    return obj


SOCKS_PORT = int(os.environ.get("M3_SOCKS_PORT", "20808"))
HTTP_PORT = int(os.environ.get("M3_HTTP_PORT", "20809"))


def patch(cfg):
    """Make config safe to run locally: localhost inbounds only."""
    cfg.setdefault("log", {})["loglevel"] = "warning"
    inbounds = cfg.get("inbounds", [])
    has_local = False
    for inbound in inbounds:
        proto = inbound.get("protocol")
        if proto in {"socks", "mixed"}:
            inbound["listen"] = "127.0.0.1"
            inbound["port"] = SOCKS_PORT
            has_local = True
        elif proto == "http":
            inbound["listen"] = "127.0.0.1"
            inbound["port"] = HTTP_PORT
            has_local = True
    if not has_local:
        inbounds.append(
            {
                "tag": "local-socks",
                "protocol": "socks",
                "listen": "127.0.0.1",
                "port": SOCKS_PORT,
                "settings": {"auth": "noauth", "udp": True},
                "sniffing": {"enabled": True, "destOverride": ["http", "tls"]},
            }
        )
    cfg["inbounds"] = inbounds
    if ("burstObservatory" in cfg or "observatory" in cfg) and "stats" not in cfg:
        cfg["stats"] = {}
    return cfg


def main():
    index = int(sys.argv[1]) if len(sys.argv) > 1 else 4
    data = json.loads(RAW.read_text())
    cfg = patch(data[index])
    OUT.write_text(json.dumps(cfg, ensure_ascii=False, indent=2))
    OUT.chmod(0o600)

    print(f"config: {index} {cfg.get('remarks')!r}")
    print("inbounds:")
    for ib in cfg.get("inbounds", []):
        print(f"  - {ib.get('protocol'):8} {ib.get('listen')}:{ib.get('port')} tag={ib.get('tag')}")
    print("outbounds:")
    for ob in cfg.get("outbounds", []):
        stream = ob.get("streamSettings", {}) or {}
        print(
            f"  - {ob.get('protocol'):10} tag={ob.get('tag')!r} "
            f"network={stream.get('network','tcp')} security={stream.get('security','none')}"
        )
    print("dns servers:", len(cfg.get("dns", {}).get("servers", [])))
    print("routing rules:", len(cfg.get("routing", {}).get("rules", [])))
    print("balancers:", len(cfg.get("routing", {}).get("balancers", [])))
    print("observatory:", "burstObservatory" in cfg or "observatory" in cfg)
    print("stats:", "stats" in cfg)
    print("wrote:", OUT)


if __name__ == "__main__":
    main()
