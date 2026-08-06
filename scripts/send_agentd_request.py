#!/usr/bin/env python3
from __future__ import annotations
import argparse, json, socket, sys

parser = argparse.ArgumentParser(description="Send one Ghost daemon JSONL request")
parser.add_argument("request", help="JSON object or @path/to/file.json")
parser.add_argument("--address", default="127.0.0.1:47644")
args = parser.parse_args()
text = open(args.request[1:]).read() if args.request.startswith("@") else args.request
payload = json.loads(text)
host, port = args.address.rsplit(":", 1)
with socket.create_connection((host, int(port)), timeout=30) as connection:
    connection.sendall(json.dumps(payload).encode() + b"\n")
    response = b""
    while not response.endswith(b"\n"):
        chunk = connection.recv(65536)
        if not chunk: break
        response += chunk
print(json.dumps(json.loads(response), indent=2))
