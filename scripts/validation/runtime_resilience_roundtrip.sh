#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
HELPER="${ROOT}/scripts/validation/runtime_resilience_fault_injection.sh"
TMP_DIR="${TMPDIR:-/tmp}/gewyvern-resilience-roundtrip"
API_ADDR="${1:-127.0.0.1:9910}"
OUT_DIR="${2:-${TMP_DIR}}"

usage() {
  cat <<EOF
usage:
  bash ${ROOT}/scripts/validation/runtime_resilience_roundtrip.sh [api-addr] [out-dir]

This helper does not start gewyvern for you.
It prepares external-engine fault-injection helpers and shows the exact
commands to run for one failure -> circuit-open -> recovery drill.
EOF
}

if [[ "${1:-}" == "--help" || "${1:-}" == "-h" ]]; then
  usage
  exit 0
fi

mkdir -p "${OUT_DIR}"

TIMEOUT_HELPER="${OUT_DIR}/external-timeout.sh"
FAIL_HELPER="${OUT_DIR}/external-fail.sh"
HEALTHY_HELPER="${OUT_DIR}/external-healthy.sh"
CONFIG_SNIPPET="${OUT_DIR}/resilience-snippet.toml"
RUNBOOK="${OUT_DIR}/runbook.txt"

bash "${HELPER}" emit-external-engine timeout "${TIMEOUT_HELPER}" >/dev/null
bash "${HELPER}" emit-external-engine fail "${FAIL_HELPER}" >/dev/null
bash "${HELPER}" emit-external-engine healthy "${HEALTHY_HELPER}" >/dev/null

cat >"${CONFIG_SNIPPET}" <<EOF
[external_engine]
bin = "${FAIL_HELPER}"

[resilience]
external_failure_circuit_threshold = 2
external_failure_circuit_cooldown_seconds = 10
socket_failure_backoff_base_ms = 100
socket_failure_backoff_cap_ms = 800
EOF

cat >"${RUNBOOK}" <<EOF
runtime resilience roundtrip
============================

prepared helpers:
- fail:    ${FAIL_HELPER}
- timeout: ${TIMEOUT_HELPER}
- healthy: ${HEALTHY_HELPER}

config snippet:
- ${CONFIG_SNIPPET}

recommended drill:

1. point your runtime config at the fail helper first:
   [external_engine].bin = "${FAIL_HELPER}"

2. run a diagnostics path enough times to cross the threshold:
   cargo run -- --diagnostics --summary
   cargo run -- --diagnostics --summary

3. check logs for:
   event=external_analysis_failed
   event=external_analysis_circuit_open

4. switch the helper to healthy:
   [external_engine].bin = "${HEALTHY_HELPER}"

5. wait for cooldown or set a short one in config, then run again:
   cargo run -- --diagnostics --summary

6. check logs for:
   event=external_analysis_recovered

7. if you already have a serve loop on tcp socket 127.0.0.1:9909, drive
   repeated invalid payloads with:
   bash "${HELPER}" drive-socket-bad-json 127.0.0.1 9909 6

8. query the API at:
   http://${API_ADDR}/health

expected socket-side signals:
- event=socket_session_collect_failed or event=socket_session_run_failed
- backoff_ms=...
- event=socket_service_recovered
EOF

echo "prepared resilience roundtrip artifacts:"
echo "- ${TIMEOUT_HELPER}"
echo "- ${FAIL_HELPER}"
echo "- ${HEALTHY_HELPER}"
echo "- ${CONFIG_SNIPPET}"
echo "- ${RUNBOOK}"
echo
cat "${RUNBOOK}"
