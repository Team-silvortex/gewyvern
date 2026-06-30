#!/usr/bin/env python3
import argparse
import socket
import time


def connect(host: str, port: int) -> socket.socket:
    return socket.create_connection((host, port), timeout=2)


def truncated_json(host: str, port: int, count: int) -> None:
    payload = b'{"facts":[{"kind":"truncated"'
    for _ in range(count):
        with connect(host, port) as sock:
            sock.sendall(payload)


def empty_disconnect(host: str, port: int, count: int) -> None:
    for _ in range(count):
        with connect(host, port):
            pass


def oversize_line(host: str, port: int, count: int) -> None:
    payload = b'{"facts":[{"kind":"oversize","value":"' + (b"x" * 262144)
    for _ in range(count):
        with connect(host, port) as sock:
            sock.sendall(payload)


def slow_drip(host: str, port: int, count: int) -> None:
    chunks = [b'{"facts"', b":[", b'{"kind"', b':"slow-drip"']
    for _ in range(count):
        with connect(host, port) as sock:
            for chunk in chunks:
                sock.sendall(chunk)
                time.sleep(0.15)


SCENARIOS = {
    "truncated-json": truncated_json,
    "empty-disconnect": empty_disconnect,
    "oversize-line": oversize_line,
    "slow-drip": slow_drip,
}


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--scenario", choices=sorted(SCENARIOS), required=True)
    parser.add_argument("--host", required=True)
    parser.add_argument("--port", type=int, required=True)
    parser.add_argument("--count", type=int, default=3)
    args = parser.parse_args()

    SCENARIOS[args.scenario](args.host, args.port, args.count)
    print(
        f"pathology scenario complete: scenario={args.scenario} "
        f"host={args.host} port={args.port} count={args.count}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
