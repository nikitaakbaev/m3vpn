#!/usr/bin/env python3
"""M0 spike: convert a WireGuard/AmneziaWG .conf into a sing-box config.

This is a prototype of the Rust parser; it intentionally supports only the
fields needed for the AmneziaWG handshake test.
"""
import argparse
import json
import re
from pathlib import Path

AWG_NUM = {
    "jc": "jc",
    "jmin": "jmin",
    "jmax": "jmax",
    "s1": "s1",
    "s2": "s2",
    "s3": "s3",
    "s4": "s4",
}
AWG_STR = {
    "h1": "h1",
    "h2": "h2",
    "h3": "h3",
    "h4": "h4",
    "i1": "i1",
    "i2": "i2",
    "i3": "i3",
    "i4": "i4",
    "i5": "i5",
    "header_protection_key": "header_protection_key",
    "content_padding_addition": "content_padding_addition",
    "rekey_after_time": "rekey_after_time",
    "rekey_timeout": "rekey_timeout",
    "reject_after_time": "reject_after_time",
    "keepalive_timeout": "keepalive_timeout",
    "max_handshake_attempts": "max_handshake_attempts",
}
AWG_BOOL = {"random_trailers": "random_trailers", "disable_cookies": "disable_cookies"}
BOOL_TRUE = {"1", "true", "yes", "on", "enabled"}


def parse_conf(text: str) -> dict:
    section = None
    data = {"interface": {}, "device": {}, "peers": []}
    peer = None
    for raw in text.splitlines():
        line = raw.strip()
        if not line or line.startswith("#") or line.startswith(";"):
            continue
        if line.startswith("[") and line.endswith("]"):
            section = line[1:-1].strip().lower()
            if section == "peer":
                peer = {}
                data["peers"].append(peer)
            continue
        if "=" not in line:
            continue
        key, value = (part.strip() for part in line.split("=", 1))
        low = key.lower()
        if section in {"interface", "device"}:
            data[section][low] = value
        elif section == "peer" and peer is not None:
            peer[low] = value
    return data


def split_endpoint(value: str) -> tuple[str, int]:
    if value.startswith("["):
        host, _, port = value[1:].partition("]:")
        return host, int(port)
    host, _, port = value.rpartition(":")
    return host, int(port)


def build(conf: dict, tag: str) -> dict:
    iface = {**conf.get("device", {}), **conf.get("interface", {})}
    if "privatekey" not in iface:
        raise SystemExit("missing PrivateKey")
    if not conf["peers"]:
        raise SystemExit("missing [Peer]")

    awg = {}
    for source, target in AWG_NUM.items():
        if source in iface:
            awg[target] = int(iface[source])
    for source, target in AWG_STR.items():
        if source in iface and iface[source] != "":
            awg[target] = iface[source]
    for source, target in AWG_BOOL.items():
        if source in iface:
            awg[target] = iface[source].lower() in BOOL_TRUE

    peers = []
    for peer in conf["peers"]:
        address, port = split_endpoint(peer["endpoint"])
        item = {
            "address": address,
            "port": port,
            "public_key": peer["publickey"],
            "allowed_ips": [x.strip() for x in peer.get("allowedips", "0.0.0.0/0, ::/0").split(",") if x.strip()],
        }
        if "presharedkey" in peer:
            item["preshared_key"] = peer["presharedkey"]
        if "persistentkeepalive" in peer:
            value = peer["persistentkeepalive"]
            item["persistent_keepalive_interval"] = value if "-" in value else int(value)
        peers.append(item)

    return {
        "type": "awg",
        "tag": tag,
        "private_key": iface["privatekey"],
        "address": [x.strip() for x in iface.get("address", "10.0.0.2/32").split(",") if x.strip()],
        "mtu": int(iface.get("mtu", 1408)),
        "awg": awg,
        "peers": peers,
    }


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("conf")
    ap.add_argument("-o", "--output", required=True)
    ap.add_argument("--socks-port", type=int, default=20808)
    ap.add_argument("--api-port", type=int, default=29090)
    ap.add_argument("--secret", default="m0-awg-secret")
    args = ap.parse_args()

    endpoint = build(parse_conf(Path(args.conf).read_text()), "awg-1")
    config = {
        "log": {"level": "info", "timestamp": True},
        "inbounds": [
            {
                "type": "mixed",
                "tag": "mixed-in",
                "listen": "127.0.0.1",
                "listen_port": args.socks_port,
            }
        ],
        "endpoints": [endpoint],
        "outbounds": [
            {
                "type": "selector",
                "tag": "proxy",
                "outbounds": [endpoint["tag"]],
                "default": endpoint["tag"],
                "interrupt_exist_connections": False,
            },
            {"type": "direct", "tag": "direct"},
        ],
        "route": {"rules": [{"action": "sniff"}], "final": "proxy"},
        "experimental": {
            "clash_api": {
                "external_controller": f"127.0.0.1:{args.api_port}",
                "secret": args.secret,
            }
        },
    }
    Path(args.output).write_text(json.dumps(config, indent=2))
    print(f"wrote {args.output} ({endpoint['type']}, {len(endpoint['peers'])} peer(s))")


if __name__ == "__main__":
    main()
