#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PARENT="$(dirname "$ROOT")"
NAME="ghost-agent-host"
cd "$PARENT"
rm -f "$PARENT/${NAME}.zip"
zip -qr "$PARENT/${NAME}.zip" "$NAME" -x "*/target/*" "*/.ghost/*" "*/__pycache__/*"
echo "$PARENT/${NAME}.zip"
