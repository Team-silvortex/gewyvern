#!/usr/bin/env bash
set -euo pipefail

GEWY_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
INGEST_ADDR="${1:-127.0.0.1:9900}"
API_ADDR="${2:-127.0.0.1:9910}"
TEMPLATE="${3:-udp}"
ANALYSIS_JSON="${4:-${GEWY_EXTERNAL_ANALYSIS_JSON:-}}"
ENGINE_JSON="${5:-${GEWY_EXTERNAL_ENGINE_JSON:-}}"
TARGET_SEGMENT="${6:-}"

ARGS=(external-engine-roundtrip "${INGEST_ADDR}" "${API_ADDR}" "${TEMPLATE}")
if [[ -n "${ANALYSIS_JSON}" ]]; then
  ARGS+=("${ANALYSIS_JSON}")
fi
if [[ -n "${ENGINE_JSON}" ]]; then
  ARGS+=("${ENGINE_JSON}")
fi
if [[ -n "${TARGET_SEGMENT}" ]]; then
  ARGS+=("${TARGET_SEGMENT}")
fi

cd "${GEWY_ROOT}"
exec "${GEWY_ROOT}/scripts/run_native_validation_bin.sh" gewyvern_native_call "${ARGS[@]}"
