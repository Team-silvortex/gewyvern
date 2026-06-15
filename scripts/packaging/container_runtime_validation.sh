#!/usr/bin/env bash

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
source "${ROOT}/scripts/packaging/container_validation_common.sh"
PACKAGES_DIR="${ROOT}/target/packages"
DEB_IMAGE="${GEWY_DEB_RUNTIME_IMAGE:-ubuntu:24.04}"
RPM_IMAGE="${GEWY_RPM_RUNTIME_IMAGE:-fedora:41}"

usage() {
  cat <<'EOF'
Usage: scripts/packaging/container_runtime_validation.sh [--deb] [--rpm]

Install the latest local native package into a clean Linux container and run a
real standalone `--serve` validation path:

- start packaged `gewyvern` as a TCP-ingest + API service
- feed repeated valid sessions with packaged `gewyvern_socket_send`
- feed one malformed line and confirm the service survives
- fetch `/health`, `/v1/latest/summary.json`, and `/v1/latest/analysis.json`

By default, both the DEB and RPM paths run.
EOF
}

container_validation_parse_mode_args usage "$@"
container_validation_require_docker "container runtime validation"

runtime_validation_body() {
  cat <<'EOF'
set -euo pipefail

TCP_SOCKET="127.0.0.1:19090"
TCP_API="127.0.0.1:19190"
TCP_SUMMARY="/tmp/tcp-summary.json"
TCP_ANALYSIS="/tmp/tcp-analysis.json"
TCP_EXPORT="/tmp/tcp-export.json"

UDP_SOCKET="127.0.0.1:19091"
UDP_API="127.0.0.1:19191"
UDP_SUMMARY="/tmp/udp-summary.json"
UDP_ANALYSIS="/tmp/udp-analysis.json"

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
  local fragment="${3:-}"
  for _ in $(seq 1 120); do
    if curl -fsS "$url" >"$out" 2>/dev/null; then
      if [ -z "${fragment}" ] || grep -q "${fragment}" "$out"; then
        return 0
      fi
    fi
    sleep 0.1
  done
  echo "timed out waiting for ${url}" >&2
  exit 1
}

start_server() {
  local template="$1"
  local socket_addr="$2"
  local api_addr="$3"
  local log_path="$4"
  gewyvern --tcp-socket "${socket_addr}" --template "${template}" --serve --api-socket "${api_addr}" --json --summary-only >"${log_path}" 2>&1 &
  echo $!
}

stop_server() {
  local pid="$1"
  kill "${pid}" >/dev/null 2>&1 || true
  wait "${pid}" >/dev/null 2>&1 || true
}

send_template() {
  local socket_addr="$1"
  local template="$2"
  gewyvern_socket_send --tcp-socket "${socket_addr}" --template "${template}"
}

send_invalid_session() {
  local socket_addr="$1"
  gewyvern_socket_send --tcp-socket "${socket_addr}" --raw-line '{"broken":true'
}

TCP_PID="$(start_server tcp "${TCP_SOCKET}" "${TCP_API}" /tmp/tcp-serve.log)"
trap 'stop_server "${TCP_PID:-}"; stop_server "${UDP_PID:-}"' EXIT

wait_for_http_body "http://${TCP_API}/health" /tmp/tcp-health.txt
send_template "${TCP_SOCKET}" tcp
wait_for_http_body "http://${TCP_API}/v1/latest/summary.json" "${TCP_SUMMARY}" '"primary_module_kind":"connection_establishment"'
wait_for_http_body "http://${TCP_API}/v1/latest/export.json" "${TCP_EXPORT}" '"template_id":"handshake_debug"'
expect_contains "${TCP_SUMMARY}" '"primary_module_kind":"connection_establishment"'
expect_contains "${TCP_SUMMARY}" '"operator_guidance_action":"avoid_pid_strong_actions"'
expect_contains "${TCP_EXPORT}" '"template_id":"handshake_debug"'

send_template "${TCP_SOCKET}" tcp
wait_for_http_body "http://${TCP_API}/v1/latest/summary.json" "${TCP_SUMMARY}" '"accepted_facts":3'
expect_contains "${TCP_SUMMARY}" '"accepted_facts":3'

send_invalid_session "${TCP_SOCKET}"
wait_for_http_body "http://${TCP_API}/health" /tmp/tcp-health-after-bad.txt
expect_contains /tmp/tcp-health-after-bad.txt '"ok":true'

send_template "${TCP_SOCKET}" tcp
wait_for_http_body "http://${TCP_API}/v1/latest/analysis.json" "${TCP_ANALYSIS}" '"protocol_flows"'
expect_contains "${TCP_ANALYSIS}" '"protocol_flows"'
stop_server "${TCP_PID}"

UDP_PID="$(start_server udp "${UDP_SOCKET}" "${UDP_API}" /tmp/udp-serve.log)"
wait_for_http_body "http://${UDP_API}/health" /tmp/udp-health.txt
send_template "${UDP_SOCKET}" udp
wait_for_http_body "http://${UDP_API}/v1/latest/summary.json" "${UDP_SUMMARY}" '"primary_module_kind":"datagram_exchange"'
wait_for_http_body "http://${UDP_API}/v1/latest/analysis.json" "${UDP_ANALYSIS}" '"primary_failure_mode":"none"'
expect_contains "${UDP_SUMMARY}" '"primary_module_kind":"datagram_exchange"'
expect_contains "${UDP_SUMMARY}" '"operator_guidance_action":"avoid_pid_strong_actions"'
expect_contains "${UDP_ANALYSIS}" '"primary_failure_mode":"none"'
stop_server "${UDP_PID}"

echo "container runtime validation: ok"
EOF
}

run_deb_runtime_validation() {
  local deb_path
  deb_path="$(container_validation_find_latest_deb "${PACKAGES_DIR}")"
  if [[ -z "${deb_path}" ]]; then
    echo "no .deb artifact found under ${PACKAGES_DIR}" >&2
    exit 1
  fi

  container_validation_run_deb_with_curl \
    "${PACKAGES_DIR}" \
    "${DEB_IMAGE}" \
    "${deb_path}" \
    "$(runtime_validation_body)"

  echo "deb runtime validation: ok (${deb_path})"
}

run_rpm_runtime_validation() {
  local rpm_path
  rpm_path="$(container_validation_find_latest_rpm "${PACKAGES_DIR}")"
  if [[ -z "${rpm_path}" ]]; then
    echo "no .rpm artifact found under ${PACKAGES_DIR}/rpm" >&2
    exit 1
  fi

  container_validation_run_rpm_with_curl \
    "${PACKAGES_DIR}" \
    "${RPM_IMAGE}" \
    "${rpm_path}" \
    "$(runtime_validation_body)"

  echo "rpm runtime validation: ok (${rpm_path})"
}

if [[ "${RUN_DEB}" -eq 1 ]]; then
  run_deb_runtime_validation
fi

if [[ "${RUN_RPM}" -eq 1 ]]; then
  run_rpm_runtime_validation
fi

echo "container runtime validation: ok"
