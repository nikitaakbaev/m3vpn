#!/usr/bin/env python3
"""M0 spike: take one Xray VLESS outbound from the subscription and build a sing-box config."""
import argparse
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


def convert(ob: dict, tag: str) -> dict:
    settings = ob["settings"]["vnext"][0]
    user = settings["users"][0]
    stream = ob.get("streamSettings", {})
    out = {
        "type": "vless",
        "tag": tag,
        "server": settings["address"],
        "server_port": settings["port"],
        "uuid": user["id"],
    }
    flow = user.get("flow") or ""
    if flow:
        out["flow"] = flow

    tls = {"enabled": True}
    security = stream.get("security", "none")
    if security == "tls":
        tls["server_name"] = stream.get("tlsSettings", {}).get("serverName", settings["address"])
    elif security == "reality":
        reality = stream.get("realitySettings", {})
        tls["server_name"] = reality.get("serverName", settings["address"])
        tls["reality"] = {
            "enabled": True,
            "public_key": reality["publicKey"],
            "short_id": reality.get("shortId", ""),
        }
    else:
        tls = {"enabled": False}

    fingerprint = (stream.get("realitySettings") or stream.get("tlsSettings") or {}).get("fingerprint")
    if fingerprint:
        tls["utls"] = {"enabled": True, "fingerprint": fingerprint}
    if tls.get("enabled"):
        out["tls"] = tls

    network = stream.get("network", "tcp")
    if network == "ws":
        ws = stream.get("wsSettings", {})
        out["transport"] = {"type": "ws", "path": ws.get("path", "/"), "headers": ws.get("headers", {})}
    elif network == "grpc":
        grpc = stream.get("grpcSettings", {})
        out["transport"] = {"type": "grpc", "service_name": grpc.get("serviceName", "")}
    elif network == "xhttp":
        xhttp = stream.get("xhttpSettings", {})
        out["transport"] = {"type": "xhttp", "path": xhttp.get("path", "/"), "host": xhttp.get("host", "")}

    return out


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--config-index", type=int, default=0)
    ap.add_argument("--outbound-index", type=int, default=0)
    ap.add_argument("-o", "--output", required=True)
    ap.add_argument("--socks-port", type=int, default=20808)
    ap.add_argument("--api-port", type=int, default=29090)
    args = ap.parse_args()

    data = json.loads((ROOT / ".secrets" / "subscription.raw").read_text())
    cfg = data[args.config_index]
    vlesses = [o for o in cfg["outbounds"] if o.get("protocol") == "vless"]
    ob = vlesses[args.outbound_index]
    outbound = convert(ob, "vless-test")

    config = {
        "log": {"level": "info", "timestamp": True},
        "inbounds": [
            {"type": "mixed", "tag": "mixed-in", "listen": "127.0.0.1", "listen_port": args.socks_port}
        ],
        "outbounds": [
            {
                "type": "selector",
                "tag": "proxy",
                "outbounds": [outbound["tag"]],
                "default": outbound["tag"],
                "interrupt_exist_connections": False,
            },
            outbound,
            {"type": "direct", "tag": "direct"},
        ],
        "route": {"rules": [{"action": "sniff"}], "final": "proxy"},
        "experimental": {
            "clash_api": {
                "external_controller": f"127.0.0.1:{args.api_port}",
                "secret": "m0-vless-secret",
            }
        },
    }
    Path(args.output).write_text(json.dumps(config, indent=2))
    print(f"wrote {args.output}: {outbound['server']}:{outbound['server_port']} reality={'reality' in outbound.get('tls', {})}")


if __name__ == "__main__":
    main()
