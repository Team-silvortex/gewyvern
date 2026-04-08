#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SOCKET_TARGET="${1:-/tmp/gewyvern-demo.sock}"
TEMPLATE="${2:-udp}"
OUT_PATH="${3:-/tmp/gewyvern-demo-output.json}"
SOCKET_KIND="${4:-unix}"

if [ "${SOCKET_KIND}" = "unix" ]; then
  rm -f "${SOCKET_TARGET}"
fi
rm -f "${OUT_PATH}"

(
  cd "${ROOT}"
  if [ "${SOCKET_KIND}" = "tcp" ]; then
    cargo run -- --tcp-socket "${SOCKET_TARGET}" --template "${TEMPLATE}" --json --out "${OUT_PATH}"
  else
    cargo run -- --unix-socket "${SOCKET_TARGET}" --template "${TEMPLATE}" --json --out "${OUT_PATH}"
  fi
) &
SERVER_PID=$!

cleanup() {
  kill "${SERVER_PID}" >/dev/null 2>&1 || true
  if [ "${SOCKET_KIND}" = "unix" ]; then
    rm -f "${SOCKET_TARGET}"
  fi
}
trap cleanup EXIT

for _ in $(seq 1 80); do
  if [ "${SOCKET_KIND}" = "tcp" ]; then
    sleep 0.05
    break
  else
    if [ -S "${SOCKET_TARGET}" ]; then
      break
    fi
    sleep 0.05
  fi
done

if [ "${SOCKET_KIND}" = "unix" ] && [ ! -S "${SOCKET_TARGET}" ]; then
  echo "socket did not appear: ${SOCKET_TARGET}" >&2
  exit 1
fi

(
  cd "${ROOT}"
  if [ "${SOCKET_KIND}" = "tcp" ]; then
    cargo run --bin gewyvern_socket_send -- --tcp-socket "${SOCKET_TARGET}" --template "${TEMPLATE}"
  else
    cargo run --bin gewyvern_socket_send -- --socket "${SOCKET_TARGET}" --template "${TEMPLATE}"
  fi
)

wait "${SERVER_PID}"
cat "${OUT_PATH}"
