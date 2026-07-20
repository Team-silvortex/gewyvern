#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

usage() {
  cat <<'EOF'
usage:
  bash scripts/validation/runtime_resilience_log_evidence.sh <log-file-or-dir> [output-dir]
EOF
}

INPUT_PATH="${1:-}"
OUT_DIR="${2:-${GEWY_RESILIENCE_LOG_EVIDENCE_OUT_DIR:-${TMPDIR:-/tmp}/gewyvern-resilience-log-evidence}}"

if [[ -z "${INPUT_PATH}" ]]; then
  usage
  exit 1
fi
if [[ "${INPUT_PATH}" == "--help" || "${INPUT_PATH}" == "-h" ]]; then
  usage
  exit 0
fi

cd "${ROOT}"
"${ROOT}/scripts/run_native_validation_bin.sh" gewyvern_validate -- \
  resilience-log-evidence \
  --log-source "${INPUT_PATH}" \
  --out-dir "${OUT_DIR}"

echo
cat "${OUT_DIR}/resilience-summary.txt"
