#!/usr/bin/env bash
set -Eeuo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
GEWYVERN_BIN="${ROOT}/target/debug/gewyvern"
SOCKET_SEND_BIN="${ROOT}/target/debug/gewyvern_socket_send"
OUT_DIR="${ROOT}/target/validation/runtime-lifecycle"
RUN_DIR=""
PIDS=()

usage() {
  cat <<'EOF'
Usage: scripts/validation/runtime_lifecycle_validation.sh [--out-dir path]

Validate the local gewyvern runtime lifecycle:

- bounded startup exits after its configured session budget
- long-running startup survives malformed input and records recovery
- runtime log evidence is written to a controlled output directory
- explicit stop removes the process and makes API/socket ports unreachable
- temporary run state is cleaned while evidence remains available
EOF
}

while [ $# -gt 0 ]; do
  case "$1" in
    --out-dir)
      if [ $# -lt 2 ]; then
        echo "missing value for --out-dir" >&2
        exit 1
      fi
      OUT_DIR="$2"
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "unknown argument: $1" >&2
      usage >&2
      exit 1
      ;;
  esac
done

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "required command not found: $1" >&2
    exit 1
  fi
}

cleanup() {
  for pid in "${PIDS[@]:-}"; do
    if [ -n "${pid}" ]; then
      kill "${pid}" >/dev/null 2>&1 || true
      wait "${pid}" >/dev/null 2>&1 || true
    fi
  done
  if [ -n "${RUN_DIR}" ] && [ -d "${RUN_DIR}" ]; then
    rm -rf "${RUN_DIR}"
  fi
}
trap cleanup EXIT

require_cmd cargo
require_cmd curl
require_cmd ps
require_cmd python3

rm -rf "${OUT_DIR}"
mkdir -p "${OUT_DIR}"
RUN_DIR="$(mktemp -d "${TMPDIR:-/tmp}/gewyvern-lifecycle.XXXXXX")"

choose_port() {
  python3 - <<'PY'
import socket
with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
    sock.bind(("127.0.0.1", 0))
    print(sock.getsockname()[1])
PY
}

expect_contains() {
  local file="$1"
  local needle="$2"
  if ! grep -q "${needle}" "${file}"; then
    echo "expected to find '${needle}' in ${file}" >&2
    echo "--- ${file} ---" >&2
    cat "${file}" >&2 || true
    exit 1
  fi
}

expect_http_unreachable() {
  local url="$1"
  if curl -fsS --max-time 1 "${url}" >/dev/null 2>&1; then
    echo "expected ${url} to be unreachable after shutdown" >&2
    exit 1
  fi
}

expect_socket_send_fails() {
  local socket_addr="$1"
  if "${SOCKET_SEND_BIN}" --tcp-socket "${socket_addr}" --template tcp >/dev/null 2>&1; then
    echo "expected socket ${socket_addr} to reject sessions after shutdown" >&2
    exit 1
  fi
}

wait_for_http_body() {
  local url="$1"
  local out="$2"
  local fragment="${3:-}"
  for _ in $(seq 1 160); do
    if curl -fsS "${url}" >"${out}" 2>/dev/null; then
      if [ -z "${fragment}" ] || grep -q "${fragment}" "${out}"; then
        return 0
      fi
    fi
    sleep 0.1
  done
  echo "timed out waiting for ${url}" >&2
  exit 1
}

wait_for_file_contains() {
  local file="$1"
  local fragment="$2"
  for _ in $(seq 1 120); do
    if [ -f "${file}" ] && grep -q "${fragment}" "${file}"; then
      return 0
    fi
    sleep 0.1
  done
  echo "timed out waiting for ${fragment} in ${file}" >&2
  exit 1
}

wait_for_pid_exit() {
  local pid="$1"
  for _ in $(seq 1 120); do
    if ! kill -0 "${pid}" >/dev/null 2>&1; then
      return 0
    fi
    local state
    state="$(ps -o stat= -p "${pid}" 2>/dev/null | tr -d ' ')"
    if [[ "${state}" == Z* ]]; then
      wait "${pid}" >/dev/null 2>&1 || true
      return 0
    fi
    sleep 0.1
  done
  echo "process ${pid} did not exit in time" >&2
  exit 1
}

start_gewyvern() {
  local socket_addr="$1"
  local api_addr="$2"
  local log_file="$3"
  local stdout_file="$4"
  shift 4
  (
    cd "${ROOT}"
    XDG_STATE_HOME="${RUN_DIR}/state" \
    exec "${GEWYVERN_BIN}" \
      --tcp-socket "${socket_addr}" \
      --template tcp \
      --serve \
      --api-socket "${api_addr}" \
      --log-level debug \
      --log-file "${log_file}" \
      --no-log-stderr \
      --json \
      --summary-only \
      "$@"
  ) >"${stdout_file}" 2>&1 &
  local pid=$!
  PIDS+=("${pid}")
  echo "${pid}"
}

send_template() {
  local socket_addr="$1"
  "${SOCKET_SEND_BIN}" --tcp-socket "${socket_addr}" --template tcp >/dev/null
}

send_invalid_session() {
  local socket_addr="$1"
  "${SOCKET_SEND_BIN}" --tcp-socket "${socket_addr}" --raw-line '{"broken":true' >/dev/null
}

write_summary() {
  local summary_path="${OUT_DIR}/summary.json"
  cat >"${summary_path}" <<JSON
{
  "script": "runtime_lifecycle_validation.sh",
  "status": "ok",
  "evidence_dir": "${OUT_DIR}",
  "covered": [
    "bounded_startup_exits_after_session_budget",
    "long_running_startup_survives_malformed_input",
    "runtime_log_records_start_failure_and_recovery",
    "explicit_stop_clears_pid_and_api_socket_reachability",
    "temporary_run_directory_removed_by_trap"
  ]
}
JSON
}

echo "[prep] building local runtime binaries"
(
  cd "${ROOT}"
  cargo build --quiet --bin gewyvern --bin gewyvern_socket_send
)

echo "[1/2] bounded serve exits after max session budget"
BOUNDED_SOCKET="127.0.0.1:$(choose_port)"
BOUNDED_API="127.0.0.1:$(choose_port)"
BOUNDED_LOG="${OUT_DIR}/bounded-runtime.log"
BOUNDED_STDOUT="${OUT_DIR}/bounded-stdout.log"
BOUNDED_HEALTH="${OUT_DIR}/bounded-health.json"

BOUNDED_PID="$(start_gewyvern "${BOUNDED_SOCKET}" "${BOUNDED_API}" "${BOUNDED_LOG}" "${BOUNDED_STDOUT}" --max-sessions 1)"
wait_for_http_body "http://${BOUNDED_API}/health" "${BOUNDED_HEALTH}" '"ok":true'
send_template "${BOUNDED_SOCKET}"
wait_for_file_contains "${BOUNDED_STDOUT}" '"template_id":"handshake_debug"'
wait_for_pid_exit "${BOUNDED_PID}"
expect_contains "${BOUNDED_LOG}" "event=tcp_service_start"
expect_http_unreachable "http://${BOUNDED_API}/health"
expect_socket_send_fails "${BOUNDED_SOCKET}"

echo "[2/2] long-running serve records failure, recovery, and controlled stop"
LONG_SOCKET="127.0.0.1:$(choose_port)"
LONG_API="127.0.0.1:$(choose_port)"
LONG_LOG="${OUT_DIR}/long-runtime.log"
LONG_STDOUT="${OUT_DIR}/long-stdout.log"
LONG_HEALTH="${OUT_DIR}/long-health.json"
LONG_DEGRADED="${OUT_DIR}/long-degraded.json"
LONG_RECOVERED="${OUT_DIR}/long-recovered.json"
LONG_SUMMARY="${OUT_DIR}/long-summary.json"

LONG_PID="$(start_gewyvern "${LONG_SOCKET}" "${LONG_API}" "${LONG_LOG}" "${LONG_STDOUT}")"
wait_for_http_body "http://${LONG_API}/health" "${LONG_HEALTH}" '"ok":true'
send_invalid_session "${LONG_SOCKET}" || true
send_invalid_session "${LONG_SOCKET}" || true
wait_for_http_body "http://${LONG_API}/v1/runtime/resilience.json" "${LONG_DEGRADED}" '"status":"degraded"'
send_template "${LONG_SOCKET}"
wait_for_http_body "http://${LONG_API}/v1/runtime/resilience.json" "${LONG_RECOVERED}" '"status":"healthy"'
wait_for_http_body "http://${LONG_API}/v1/latest/summary.json" "${LONG_SUMMARY}" '"template_id":"handshake_debug"'
expect_contains "${LONG_LOG}" "event=tcp_service_start"
expect_contains "${LONG_LOG}" "event=socket_session_run_failed"
expect_contains "${LONG_LOG}" "event=socket_service_recovered"

kill "${LONG_PID}" >/dev/null 2>&1 || true
wait_for_pid_exit "${LONG_PID}"
expect_http_unreachable "http://${LONG_API}/health"
expect_socket_send_fails "${LONG_SOCKET}"

write_summary

echo "runtime lifecycle validation: ok"
echo "evidence=${OUT_DIR}"
