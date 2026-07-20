#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SOCKET_PATH="${1:-${GEWY_DEMO_SOCKET_PATH:-}}"
SOCKET_TEMPLATE="${2:-udp}"
SOCKET_OUTPUT="${3:-${GEWY_DEMO_OUTPUT:-}}"
SOCKET_KIND="${4:-unix}"

if [[ -z "${SOCKET_PATH}" ]]; then
  SOCKET_PATH="$(mktemp -u "${TMPDIR:-/tmp}/gewyvern-demo-XXXXXX.sock")"
fi
if [[ -z "${SOCKET_OUTPUT}" ]]; then
  SOCKET_OUTPUT="${TMPDIR:-/tmp}/gewyvern-demo-output.json"
fi

ARGS=(
  socket-roundtrip
  "${SOCKET_PATH}"
  "${SOCKET_TEMPLATE}"
  "${SOCKET_OUTPUT}"
  "${SOCKET_KIND}"
)

cd "${ROOT}"

exec "${ROOT}/scripts/run_native_validation_bin.sh" gewyvern_native_call "${ARGS[@]}"
