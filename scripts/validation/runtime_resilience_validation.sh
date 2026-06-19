#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
ROUNDTRIP_HELPER="${ROOT}/scripts/validation/runtime_resilience_roundtrip.sh"
EVIDENCE_HELPER="${ROOT}/scripts/validation/runtime_resilience_log_evidence.sh"
API_ADDR="${1:-127.0.0.1:9910}"
LOG_SOURCE="${2:-}"
OUT_DIR="${3:-${TMPDIR:-/tmp}/gewyvern-resilience-validation}"

usage() {
  cat <<EOF
usage:
  bash ${ROOT}/scripts/validation/runtime_resilience_validation.sh [api-addr] <log-file-or-dir> [output-dir]

This wrapper prepares one resilience validation bundle containing:

- fault-injection helper scripts
- a config snippet
- a runbook
- extracted resilience log evidence

arguments:
  api-addr         optional, default: 127.0.0.1:9910
  log-file-or-dir  required runtime log input for evidence extraction
  output-dir       optional bundle output directory
EOF
}

main() {
  if [[ "${1:-}" == "--help" || "${1:-}" == "-h" ]]; then
    usage
    exit 0
  fi

  if [[ -z "${LOG_SOURCE}" ]]; then
    usage
    exit 1
  fi

  mkdir -p "${OUT_DIR}"

  echo "[1/3] preparing roundtrip helpers and runbook"
  bash "${ROUNDTRIP_HELPER}" "${API_ADDR}" "${OUT_DIR}/roundtrip" >"${OUT_DIR}/roundtrip-output.txt"

  echo "[2/3] extracting resilience log evidence"
  bash "${EVIDENCE_HELPER}" "${LOG_SOURCE}" "${OUT_DIR}/evidence" >"${OUT_DIR}/evidence-output.txt"

  echo "[3/3] writing bundle index"
  cat >"${OUT_DIR}/README.txt" <<EOF
gewyvern runtime resilience validation bundle
============================================

api address:
- ${API_ADDR}

log source:
- ${LOG_SOURCE}

bundle contents:
- roundtrip/            prepared helper scripts, config snippet, and runbook
- evidence/             extracted resilience-events.log and resilience-summary.txt
- roundtrip-output.txt  console output from the roundtrip preparation helper
- evidence-output.txt   console output from the evidence extraction helper

recommended review order:
1. roundtrip/runbook.txt
2. evidence/resilience-summary.txt
3. evidence/resilience-events.log
EOF

  echo "prepared resilience validation bundle:"
  echo "- ${OUT_DIR}/README.txt"
  echo "- ${OUT_DIR}/roundtrip/runbook.txt"
  echo "- ${OUT_DIR}/evidence/resilience-summary.txt"
  echo "- ${OUT_DIR}/evidence/resilience-events.log"
}

main "$@"
