#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SOCKET_TARGET="${1:-/tmp/gewyvern-demo.sock}"
TEMPLATE="${2:-udp}"
OUT_PATH="${3:-/tmp/gewyvern-demo-output.json}"
SOCKET_KIND="${4:-unix}"

cd "${ROOT}"
cargo run --quiet --bin gewyvern_validate -- \
  socket-roundtrip \
  --socket-target "${SOCKET_TARGET}" \
  --template "${TEMPLATE}" \
  --output "${OUT_PATH}" \
  --socket-kind "${SOCKET_KIND}"

cat "${OUT_PATH}"
