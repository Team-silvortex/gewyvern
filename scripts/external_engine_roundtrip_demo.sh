#!/usr/bin/env bash
set -euo pipefail

GEWY_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ENGINE_ROOT_DEFAULT="${ENGINE_ROOT:-}"
ETRAGON_ROOT_DEFAULT="${ETRAGON_ROOT:-}"

INGEST_ADDR="${1:-127.0.0.1:9900}"
API_ADDR="${2:-127.0.0.1:9910}"
TEMPLATE="${3:-udp}"
ANALYSIS_OUT="${4:-/tmp/gewyvern-analysis.json}"
ENGINE_OUT="${5:-/tmp/external-engine-augmentations.json}"
TARGET_PATH_SEGMENT="${6:-}"

ANALYSIS_ROUTE="/v1/latest/analysis.json"
if [ -n "${TARGET_PATH_SEGMENT}" ]; then
  ANALYSIS_ROUTE="/v1/latest/targets/${TARGET_PATH_SEGMENT}/analysis.json"
fi

ENGINE_ROOT="${ENGINE_ROOT_DEFAULT}"
if [ -z "${ENGINE_ROOT}" ] && [ -n "${ETRAGON_ROOT_DEFAULT}" ]; then
  ENGINE_ROOT="${ETRAGON_ROOT_DEFAULT}"
fi
if [ -z "${ENGINE_ROOT}" ] && [ -d "${GEWY_ROOT}/../etragon" ]; then
  ENGINE_ROOT="$(cd "${GEWY_ROOT}/../etragon" && pwd)"
fi

ENGINE_CMD="${EXTERNAL_ENGINE_CMD:-}"
if [ -z "${ENGINE_CMD}" ]; then
  ENGINE_CMD="cargo run -- analyze-url"
fi

rm -f "${ANALYSIS_OUT}" "${ENGINE_OUT}"

(
  cd "${GEWY_ROOT}"
  cargo run -- \
    --tcp-socket "${INGEST_ADDR}" \
    --ingest-mode local-advisory \
    --serve \
    --api-socket "${API_ADDR}" \
    --json \
    --summary-only
) &
SERVER_PID=$!

cleanup() {
  kill "${SERVER_PID}" >/dev/null 2>&1 || true
}
trap cleanup EXIT

for _ in $(seq 1 120); do
  if curl -fsS "http://${API_ADDR}/health" >/dev/null 2>&1; then
    break
  fi
  sleep 0.05
done

if ! curl -fsS "http://${API_ADDR}/health" >/dev/null 2>&1; then
  echo "gewyvern API did not become ready at ${API_ADDR}" >&2
  exit 1
fi

(
  cd "${GEWY_ROOT}"
  cargo run --bin gewyvern_socket_send -- --tcp-socket "${INGEST_ADDR}" --template "${TEMPLATE}"
)

if [ -z "${ENGINE_ROOT}" ]; then
  echo "external engine root is not set and no sibling /../etragon repo was found" >&2
  echo "set ENGINE_ROOT=/path/to/external-engine or ETRAGON_ROOT=/path/to/etragon" >&2
  exit 1
fi

(
  cd "${ENGINE_ROOT}"
  sh -c "${ENGINE_CMD} \"http://${API_ADDR}${ANALYSIS_ROUTE}\""
) > "${ENGINE_OUT}"

curl -fsS "http://${API_ADDR}${ANALYSIS_ROUTE}" > "${ANALYSIS_OUT}"

echo "analysis_json=${ANALYSIS_OUT}"
echo "external_engine_output=${ENGINE_OUT}"
cat "${ENGINE_OUT}"
