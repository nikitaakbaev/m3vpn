#!/usr/bin/env python3
"""Inspect an INCY-compatible subscription body and print a sanitized summary."""
import base64
import json
import sys
from pathlib import Path

RAW = Path(__file__).resolve().parents[2] / ".secrets" / "subscription.raw"


def maybe_b64(text: str) -> str | None:
    compact = "".join(text.split())
    if len(compact) < 16:
        return None
    compact = compact.replace("-", "+").replace("_", "/")
    compact += "=" * (-len(compact) % 4)
    try:
        decoded = base64.b64decode(compact, validate=True).decode("utf-8")
    except Exception:
        return None
    return decoded if decoded.count("\n") or "://" in decoded or decoded.lstrip().startswith(("{", "[")) else None


def load():
    body = RAW.read_text(errors="replace")
    stripped = body.lstrip()
    if stripped.startswith(("{", "[")):
        return json.loads(body), "json"
    decoded = maybe_b64(body)
    if decoded:
        return json.loads(decoded), "base64-json"
    return [ln for ln in body.splitlines() if ln.strip()], "lines"


def mask_host(host: str) -> str:
    if not host:
        return ""
    parts = host.split(".")
    if len(parts) >= 3:
        return f"{parts[0][:3]}***.{parts[-2]}.{parts[-1]}"
    return host[:4] + "***"


def main():
    data, kind = load()
    print(f"format: {kind}")
    if isinstance(data, list) and data and isinstance(data[0], dict):
        print(f"configs: {len(data)}")
        for i, cfg in enumerate(data):
            outs = cfg.get("outbounds", [])
            protocols = sorted({o.get("protocol", "?") for o in outs})
            tags = [o.get("tag") for o in outs]
            balancers = cfg.get("routing", {}).get("balancers", [])
            obs = "burstObservatory" if "burstObservatory" in cfg else ("observatory" if "observatory" in cfg else "-")
            full = "inbounds" in cfg and "outbounds" in cfg
            print(
                f"[{i:02d}] {cfg.get('remarks','?')[:48]!r} full={full} proto={protocols} "
                f"balancers={len(balancers)} obs={obs} out_tags={tags}"
            )
            for ob in balancers:
                print(f"      balancer {ob.get('tag')} strategy={ob.get('strategy')} selector={ob.get('selector')}")
    elif isinstance(data, list):
        from collections import Counter
        schemes = Counter(line.split("://", 1)[0] for line in data if "://" in line)
        print(f"links: {len(data)} schemes={dict(schemes)}")
    else:
        print("unexpected data", type(data))


if __name__ == "__main__":
    sys.exit(main())
