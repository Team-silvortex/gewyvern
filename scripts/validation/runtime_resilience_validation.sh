#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
API_ADDR="${1:-127.0.0.1:9910}"
LOG_SOURCE="${2:-}"
OUT_DIR="${3:-${TMPDIR:-/tmp}/gewyvern-resilience-validation}"

usage() {
  cat <<EOF
usage:
  bash ${ROOT}/scripts/validation/runtime_resilience_validation.sh [api-addr] <log-file-or-dir> [output-dir]
EOF
}

if [[ "${1:-}" == "--help" || "${1:-}" == "-h" ]]; then
  usage
  exit 0
fi
if [[ -z "${LOG_SOURCE}" ]]; then
  usage
  exit 1
fi

cd "${ROOT}"
cargo run --quiet --bin gewyvern_validate -- \
  resilience-bundle \
  --api-addr "${API_ADDR}" \
  --log-source "${LOG_SOURCE}" \
  --out-dir "${OUT_DIR}"

echo "prepared resilience validation bundle:"
echo "- ${OUT_DIR}/README.txt"
echo "- ${OUT_DIR}/roundtrip/runbook.txt"
echo "- ${OUT_DIR}/evidence/resilience-summary.txt"
echo "- ${OUT_DIR}/evidence/resilience-events.log"
