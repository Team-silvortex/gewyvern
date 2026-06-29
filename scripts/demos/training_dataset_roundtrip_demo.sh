#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
API_ADDR="${1:-127.0.0.1:9910}"
OUT_DIR="${2:-/tmp/gewyvern-training-dataset-demo}"
TARGET_PATH_SEGMENT="${3:-}"
LIMIT="${4:-0}"

ARGS=(
  training-roundtrip
  --api-addr "${API_ADDR}"
  --out-dir "${OUT_DIR}"
  --limit "${LIMIT}"
)

if [ -n "${TARGET_PATH_SEGMENT}" ]; then
  ARGS+=(--target-path-segment "${TARGET_PATH_SEGMENT}")
fi

cd "${ROOT}"
cargo run --quiet --bin gewyvern_validate -- "${ARGS[@]}"

cat "${OUT_DIR}/roundtrip-summary.json"
