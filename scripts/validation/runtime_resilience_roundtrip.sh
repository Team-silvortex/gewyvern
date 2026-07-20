#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
API_ADDR="${1:-127.0.0.1:9910}"
OUT_DIR="${2:-${GEWY_RESILIENCE_ROUNDTRIP_OUT_DIR:-${TMPDIR:-/tmp}/gewyvern-resilience-roundtrip}}"

usage() {
  cat <<EOF
usage:
  bash ${ROOT}/scripts/validation/runtime_resilience_roundtrip.sh [api-addr] [out-dir]
EOF
}

if [[ "${1:-}" == "--help" || "${1:-}" == "-h" ]]; then
  usage
  exit 0
fi

cd "${ROOT}"
"${ROOT}/scripts/run_native_validation_bin.sh" gewyvern_validate -- \
  resilience-roundtrip \
  --api-addr "${API_ADDR}" \
  --out-dir "${OUT_DIR}"

echo
cat "${OUT_DIR}/runbook.txt"
