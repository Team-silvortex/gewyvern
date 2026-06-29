#!/usr/bin/env bash
set -euo pipefail

GEWY_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

INGEST_ADDR="${1:-127.0.0.1:9900}"
API_ADDR="${2:-127.0.0.1:9910}"
TEMPLATE="${3:-udp}"
ANALYSIS_OUT="${4:-/tmp/gewyvern-analysis.json}"
ENGINE_OUT="${5:-/tmp/external-engine-augmentations.json}"
TARGET_PATH_SEGMENT="${6:-}"

ARGS=(
  external-engine-roundtrip
  --ingest-addr "${INGEST_ADDR}"
  --api-addr "${API_ADDR}"
  --template "${TEMPLATE}"
  --analysis-out "${ANALYSIS_OUT}"
  --engine-out "${ENGINE_OUT}"
)

if [ -n "${TARGET_PATH_SEGMENT}" ]; then
  ARGS+=(--target-path-segment "${TARGET_PATH_SEGMENT}")
fi

(
  cd "${GEWY_ROOT}"
  cargo run --quiet --bin gewyvern_validate -- "${ARGS[@]}"
)

echo "analysis_json=${ANALYSIS_OUT}"
echo "external_engine_output=${ENGINE_OUT}"
if [ -s "${ENGINE_OUT}" ]; then
  cat "${ENGINE_OUT}"
fi
