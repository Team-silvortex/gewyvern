#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TMP_DIR="$(mktemp -d "${TMPDIR:-/tmp}/gewyvern-runtime-validation.XXXXXX")"
trap 'rm -rf "${TMP_DIR}"' EXIT
GEWYVERN_BIN="${ROOT}/target/debug/gewyvern"
SOCKET_SEND_BIN="${ROOT}/target/debug/gewyvern_socket_send"

expect_contains() {
  local file="$1"
  local needle="$2"
  if ! grep -q "$needle" "$file"; then
    echo "expected to find '${needle}' in ${file}" >&2
    exit 1
  fi
}

wait_for_http_body() {
  local url="$1"
  local out="$2"
  for _ in $(seq 1 120); do
    if curl -fsS "$url" >"$out" 2>/dev/null; then
      return 0
    fi
    sleep 0.1
  done
  echo "timed out waiting for ${url}" >&2
  exit 1
}

wait_for_snapshot_json() {
  local url="$1"
  local out="$2"
  local fragment="${3:-}"
  for _ in $(seq 1 120); do
    if curl -fsS "$url" >"$out" 2>/dev/null; then
      if [ -z "${fragment}" ] || grep -q "${fragment}" "$out"; then
        return 0
      fi
    fi
    sleep 0.1
  done
  echo "timed out waiting for latest snapshot at ${url}" >&2
  exit 1
}

wait_for_http_fragment() {
  local url="$1"
  local out="$2"
  local fragment="$3"
  for _ in $(seq 1 120); do
    if curl -fsS "$url" >"$out" 2>/dev/null; then
      if grep -q "${fragment}" "$out"; then
        return 0
      fi
    fi
    sleep 0.1
  done
  echo "timed out waiting for ${fragment} at ${url}" >&2
  exit 1
}

start_server() {
  local template="$1"
  local socket_addr="$2"
  local api_addr="$3"
  local log_path="$4"

  (
    cd "${ROOT}"
    exec "${GEWYVERN_BIN}" --tcp-socket "${socket_addr}" --template "${template}" --serve --api-socket "${api_addr}" --json --summary-only >"${log_path}" 2>&1
  ) &
  echo $!
}

send_template() {
  local socket_addr="$1"
  local template="$2"
  "${SOCKET_SEND_BIN}" --tcp-socket "${socket_addr}" --template "${template}"
}

send_invalid_session() {
  local socket_addr="$1"
  "${SOCKET_SEND_BIN}" --tcp-socket "${socket_addr}" --raw-line '{"broken":true'
}

stop_server() {
  local pid="$1"
  kill "${pid}" >/dev/null 2>&1 || true
  wait "${pid}" >/dev/null 2>&1 || true
}

echo "[prep] building local runtime binaries"
(
  cd "${ROOT}"
  cargo build --quiet --bin gewyvern --bin gewyvern_socket_send
)

echo "[1/2] tcp serve accepts repeated sessions and serves latest snapshot"
TCP_SOCKET="127.0.0.1:19090"
TCP_API="127.0.0.1:19190"
TCP_LOG="${TMP_DIR}/tcp-serve.log"
TCP_SUMMARY="${TMP_DIR}/tcp-summary.json"
TCP_EXPORT="${TMP_DIR}/tcp-export.json"

TCP_PID="$(start_server tcp "${TCP_SOCKET}" "${TCP_API}" "${TCP_LOG}")"
trap 'kill "${TCP_PID}" >/dev/null 2>&1 || true; rm -rf "${TMP_DIR}"' EXIT

wait_for_http_body "http://${TCP_API}/health" "${TMP_DIR}/tcp-health.txt"
send_template "${TCP_SOCKET}" tcp
wait_for_snapshot_json "http://${TCP_API}/v1/latest/summary.json" "${TCP_SUMMARY}" '"primary_module_kind":"connection_establishment"'
wait_for_http_fragment "http://${TCP_API}/v1/latest/export.json" "${TCP_EXPORT}" '"template_id":"handshake_debug"'
expect_contains "${TCP_SUMMARY}" '"primary_module_kind":"connection_establishment"'
expect_contains "${TCP_SUMMARY}" '"operator_guidance_action":"avoid_pid_strong_actions"'
expect_contains "${TCP_EXPORT}" '"template_id":"handshake_debug"'

send_template "${TCP_SOCKET}" tcp
wait_for_snapshot_json "http://${TCP_API}/v1/latest/summary.json" "${TCP_SUMMARY}" '"accepted_facts":3'
curl -fsS "http://${TCP_API}/v1/latest/export.json" >"${TCP_EXPORT}"
expect_contains "${TCP_SUMMARY}" '"accepted_facts":3'

send_invalid_session "${TCP_SOCKET}"
wait_for_http_body "http://${TCP_API}/health" "${TMP_DIR}/tcp-health-after-bad.txt"
expect_contains "${TMP_DIR}/tcp-health-after-bad.txt" '"ok":true'
expect_contains "${TCP_SUMMARY}" '"name":"socket_session"'

send_template "${TCP_SOCKET}" tcp
wait_for_snapshot_json "http://${TCP_API}/v1/latest/summary.json" "${TCP_SUMMARY}" '"template_id":"handshake_debug"'
expect_contains "${TCP_SUMMARY}" '"template_id":"handshake_debug"'
stop_server "${TCP_PID}"

echo "[2/2] udp serve publishes datagram-oriented latest snapshot through API"
UDP_SOCKET="127.0.0.1:19091"
UDP_API="127.0.0.1:19191"
UDP_LOG="${TMP_DIR}/udp-serve.log"
UDP_SUMMARY="${TMP_DIR}/udp-summary.json"
UDP_ANALYSIS="${TMP_DIR}/udp-analysis.json"

UDP_PID="$(start_server udp "${UDP_SOCKET}" "${UDP_API}" "${UDP_LOG}")"
trap 'kill "${UDP_PID}" >/dev/null 2>&1 || true; rm -rf "${TMP_DIR}"' EXIT

wait_for_http_body "http://${UDP_API}/health" "${TMP_DIR}/udp-health.txt"
send_template "${UDP_SOCKET}" udp
wait_for_snapshot_json "http://${UDP_API}/v1/latest/summary.json" "${UDP_SUMMARY}" '"primary_module_kind":"datagram_exchange"'
wait_for_http_fragment "http://${UDP_API}/v1/latest/analysis.json" "${UDP_ANALYSIS}" '"primary_failure_mode":"none"'
expect_contains "${UDP_SUMMARY}" '"primary_module_kind":"datagram_exchange"'
expect_contains "${UDP_SUMMARY}" '"operator_guidance_action":"avoid_pid_strong_actions"'
expect_contains "${UDP_ANALYSIS}" '"protocol_flows"'
expect_contains "${UDP_ANALYSIS}" '"primary_failure_mode":"none"'

stop_server "${UDP_PID}"

echo "runtime operator validation: ok"
