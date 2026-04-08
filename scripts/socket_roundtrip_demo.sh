#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SOCKET_PATH="${1:-/tmp/gewyvern-demo.sock}"
TEMPLATE="${2:-udp}"
OUT_PATH="${3:-/tmp/gewyvern-demo-output.json}"

rm -f "${SOCKET_PATH}" "${OUT_PATH}"

(
  cd "${ROOT}"
  cargo run -- --unix-socket "${SOCKET_PATH}" --template "${TEMPLATE}" --json --out "${OUT_PATH}"
) &
SERVER_PID=$!

cleanup() {
  kill "${SERVER_PID}" >/dev/null 2>&1 || true
  rm -f "${SOCKET_PATH}"
}
trap cleanup EXIT

for _ in $(seq 1 80); do
  if [ -S "${SOCKET_PATH}" ]; then
    break
  fi
  sleep 0.05
done

if [ ! -S "${SOCKET_PATH}" ]; then
  echo "socket did not appear: ${SOCKET_PATH}" >&2
  exit 1
fi

(
  cd "${ROOT}"
  cargo run --bin gewyvern_socket_send -- --socket "${SOCKET_PATH}" --template "${TEMPLATE}"
)

wait "${SERVER_PID}"
cat "${OUT_PATH}"
