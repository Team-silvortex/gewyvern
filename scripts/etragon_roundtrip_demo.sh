#!/usr/bin/env bash
set -euo pipefail

GEWY_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ETRAGON_ROOT="${ETRAGON_ROOT:-$(cd "${GEWY_ROOT}/../etragon" && pwd)}"

INGEST_ADDR="${1:-127.0.0.1:9900}"
API_ADDR="${2:-127.0.0.1:9910}"
TEMPLATE="${3:-udp}"
ANALYSIS_OUT="${4:-/tmp/gewyvern-analysis.json}"
ETRAGON_OUT="${5:-/tmp/etragon-augmentations.json}"
TARGET_PATH_SEGMENT="${6:-}"

ANALYSIS_ROUTE="/v1/latest/analysis.json"
if [ -n "${TARGET_PATH_SEGMENT}" ]; then
  ANALYSIS_ROUTE="/v1/latest/targets/${TARGET_PATH_SEGMENT}/analysis.json"
fi

rm -f "${ANALYSIS_OUT}" "${ETRAGON_OUT}"

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

(
  cd "${ETRAGON_ROOT}"
  cargo run -- analyze-url "http://${API_ADDR}${ANALYSIS_ROUTE}"
) > "${ETRAGON_OUT}"

curl -fsS "http://${API_ADDR}${ANALYSIS_ROUTE}" > "${ANALYSIS_OUT}"

echo "analysis_json=${ANALYSIS_OUT}"
echo "etragon_output=${ETRAGON_OUT}"
cat "${ETRAGON_OUT}"
