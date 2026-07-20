#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
API_ADDR="${1:-127.0.0.1:9910}"
OUT_DIR="${2:-${GEWY_TRAINING_DEMO_OUT_DIR:-${TMPDIR:-/tmp}/gewyvern-training-dataset-demo}}"
TARGET_SEGMENT="${3:-}"
LIMIT="${4:-0}"
ARGS=(
  training-roundtrip
  "${API_ADDR}"
  "${OUT_DIR}"
)
if [[ -n "${TARGET_SEGMENT}" ]]; then
  ARGS+=("${TARGET_SEGMENT}")
fi
ARGS+=("${LIMIT}")

cd "${ROOT}"
exec "${ROOT}/scripts/run_native_validation_bin.sh" gewyvern_native_call "${ARGS[@]}"
