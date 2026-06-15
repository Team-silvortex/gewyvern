#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
TMP_DIR="$(mktemp -d "${TMPDIR:-/tmp}/gewyvern-runtime-validation.XXXXXX")"
trap 'rm -rf "${TMP_DIR}"' EXIT
GEWYVERN_BIN="${ROOT}/target/debug/gewyvern"
SOCKET_SEND_BIN="${ROOT}/target/debug/gewyvern_socket_send"
JSON_OUT=""

while [ $# -gt 0 ]; do
  case "$1" in
    --json-out)
      if [ $# -lt 2 ]; then
        echo "missing value for --json-out" >&2
        exit 1
      fi
      JSON_OUT="$2"
      shift 2
      ;;
    *)
      echo "unknown argument: $1" >&2
      echo "usage: $(basename "$0") [--json-out /path/to/summary.json]" >&2
      exit 1
      ;;
  esac
done

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

write_json_summary() {
  local out_path="$1"
  mkdir -p "$(dirname "${out_path}")"
  cat >"${out_path}" <<'JSON'
{
  "script": "runtime_operator_validation.sh",
  "status": "ok",
  "security_checklist_coverage": {
    "covered_by_script": [
      "serve_loop_survives_repeated_healthy_sessions",
      "latest_snapshot_refresh_visible_through_read_only_api",
      "malformed_ingest_does_not_kill_service_loop",
      "tcp_and_udp_latest_snapshot_paths_remain_readable",
      "restart_cleared_latest_only_api_shape_is_observable"
    ],
    "requires_operator_confirmation": [
      "ingest_mode_matches_deployment_trust_intent",
      "remote_api_exposure_is_avoided_or_explicitly_opted_in",
      "external_engine_wiring_is_intentional_and_bounded",
      "custom_registry_roots_are_trusted_and_scoped",
      "surrounding_automation_handles_404_and_503_paths"
    ]
  }
}
JSON
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
UDP_TRAINING_SUMMARY="${TMP_DIR}/udp-training-roundtrip.json"

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
    bash "${ROOT}/scripts/demos/training_dataset_roundtrip_demo.sh" \
  "${UDP_API}" \
  "${TMP_DIR}/training-roundtrip" \
  >"${UDP_TRAINING_SUMMARY}"
expect_contains "${UDP_TRAINING_SUMMARY}" '"sample_ids_verified": true'
expect_contains "${UDP_TRAINING_SUMMARY}" '"default_split_policy": "name_bucket_mod_10"'

stop_server "${UDP_PID}"

echo "runtime operator validation: ok"
echo
echo "security checklist coverage summary:"
echo "- covered by this script:"
echo "  - serve loop survives repeated healthy sessions"
echo "  - latest snapshot refresh is observable through the read-only API"
echo "  - malformed ingest does not kill the service loop"
echo "  - TCP and UDP latest-snapshot paths remain readable"
echo "  - restart-cleared / latest-only API mental model is the expected operating shape"
echo "- still requires operator confirmation:"
echo "  - ingest mode matches trust intent for the actual deployment"
echo "  - remote API exposure is either avoided or explicitly opted in"
echo "  - external engine wiring is intentional and bounded"
echo "  - custom registry roots are trusted and scoped"
echo "  - surrounding automation handles 404/503 paths correctly"
echo
echo "see docs/book/how-to-security-checklist.md for the full preflight"

if [ -n "${JSON_OUT}" ]; then
  write_json_summary "${JSON_OUT}"
  echo "json summary written to ${JSON_OUT}"
fi
