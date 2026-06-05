#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DEV_ROOT="$(cd "${ROOT}/.." && pwd)"
GEWY_ROOT="${ROOT}"
ETRAGON_ROOT="${DEV_ROOT}/etragon"
LESERPENT_ROOT="${DEV_ROOT}/leserpent"

IMAGE_TAG="${IMAGE_TAG:-gewyvern-stack-dev}"
NETWORK_NAME="${NETWORK_NAME:-gewyvern-stack-net}"
GW_A_NAME="${GW_A_NAME:-gewyvern-stack-a}"
GW_B_NAME="${GW_B_NAME:-gewyvern-stack-b}"
ET_A_NAME="${ET_A_NAME:-etragon-stack-a}"
LESP_PORT="${LESP_PORT:-5118}"
GW_A_SOCKET_PORT="${GW_A_SOCKET_PORT:-19001}"
GW_A_API_PORT="${GW_A_API_PORT:-19101}"
GW_B_SOCKET_PORT="${GW_B_SOCKET_PORT:-19002}"
GW_B_API_PORT="${GW_B_API_PORT:-19102}"
ET_A_API_PORT="${ET_A_API_PORT:-19431}"

WORK_DIR="$(mktemp -d /private/tmp/three-module-stack.XXXXXX)"
TARGET_CACHE_DIR="${WORK_DIR}/target-cache"
STATE_PATH="${WORK_DIR}/leserpent-state.json"
LESP_LOG="${WORK_DIR}/leserpent.log"
mkdir -p "${TARGET_CACHE_DIR}"

LESP_PID=""

cleanup() {
  if [ -n "${LESP_PID}" ]; then
    kill "${LESP_PID}" >/dev/null 2>&1 || true
    wait "${LESP_PID}" >/dev/null 2>&1 || true
  fi
  docker rm -f "${GW_A_NAME}" >/dev/null 2>&1 || true
  docker rm -f "${GW_B_NAME}" >/dev/null 2>&1 || true
  docker rm -f "${ET_A_NAME}" >/dev/null 2>&1 || true
  docker network rm "${NETWORK_NAME}" >/dev/null 2>&1 || true
}
trap cleanup EXIT

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "required command not found: $1" >&2
    exit 1
  fi
}

require_cmd docker
require_cmd curl
require_cmd python3
require_cmd dotnet

if ! docker info >/dev/null 2>&1; then
  echo "docker daemon is not reachable; start Docker Desktop or another local daemon and retry" >&2
  exit 1
fi

if [ ! -d "${ETRAGON_ROOT}" ]; then
  echo "missing sibling etragon repo at ${ETRAGON_ROOT}" >&2
  exit 1
fi

if [ ! -d "${LESERPENT_ROOT}" ]; then
  echo "missing sibling leserpent repo at ${LESERPENT_ROOT}" >&2
  exit 1
fi

docker build -t "${IMAGE_TAG}" -f "${GEWY_ROOT}/docker/linux-dev/Dockerfile" "${GEWY_ROOT}" >/dev/null

docker network rm "${NETWORK_NAME}" >/dev/null 2>&1 || true
docker network create "${NETWORK_NAME}" >/dev/null

docker run --rm \
  -v "${DEV_ROOT}:/workspace/dev" \
  -v "${TARGET_CACHE_DIR}:/stack-target" \
  "${IMAGE_TAG}" \
  bash -lc '
    set -euo pipefail
    export CARGO_TARGET_DIR=/stack-target/etragon
    cd /workspace/dev/etragon
    cargo build --quiet
    export CARGO_TARGET_DIR=/stack-target/gewyvern
    cd /workspace/dev/gewyvern
    cargo build --quiet --bin gewyvern --bin gewyvern_socket_send
  '

start_gewyvern() {
  local name="$1"
  local socket_port="$2"
  local api_port="$3"
  docker rm -f "${name}" >/dev/null 2>&1 || true
  docker run -d \
    --name "${name}" \
    --network "${NETWORK_NAME}" \
    -p "${socket_port}:9000" \
    -p "${api_port}:9100" \
    -v "${DEV_ROOT}:/workspace/dev" \
    -v "${TARGET_CACHE_DIR}:/stack-target" \
    "${IMAGE_TAG}" \
    bash -lc "
      set -euo pipefail
      export CARGO_TARGET_DIR=/stack-target/gewyvern
      cd /workspace/dev/gewyvern
      /stack-target/gewyvern/debug/gewyvern \
        --tcp-socket 0.0.0.0:9000 \
        --template udp \
        --ingest-mode remote-advisory \
        --serve \
        --allow-remote-api \
        --api-socket 0.0.0.0:9100 \
        --json \
        --summary-only
    " >/dev/null
}

start_etragon() {
  docker rm -f "${ET_A_NAME}" >/dev/null 2>&1 || true
  docker run -d \
    --name "${ET_A_NAME}" \
    --network "${NETWORK_NAME}" \
    -p "${ET_A_API_PORT}:4321" \
    -v "${DEV_ROOT}:/workspace/dev" \
    -v "${TARGET_CACHE_DIR}:/stack-target" \
    "${IMAGE_TAG}" \
    bash -lc "
      set -euo pipefail
      export CARGO_TARGET_DIR=/stack-target/etragon
      cd /workspace/dev/etragon
      /stack-target/etragon/debug/etragon \
        serve-python-url \
        http://${GW_A_NAME}:9100/v1/latest/analysis.json \
        --bind 0.0.0.0:4321 \
        --interval-ms 500 \
        --python-worker /workspace/dev/etragon/scripts/python_baseline_worker.py \
        --python-state /tmp/etragon-online-state.json \
        --daemon-state /tmp/etragon-daemon-state.json
    " >/dev/null
}

wait_http() {
  local url="$1"
  for _ in $(seq 1 240); do
    if curl -fsS "${url}" >/dev/null 2>&1; then
      return 0
    fi
    sleep 0.25
  done
  return 1
}

wait_for_meta_field() {
  local url="$1"
  local field="$2"
  local expected="$3"
  for _ in $(seq 1 240); do
    local body
    if body="$(curl -fsS "${url}" 2>/dev/null)"; then
      if [ "$(printf '%s' "${body}" | python3 -c 'import json, sys; payload = json.load(sys.stdin); print("true" if bool(payload.get(sys.argv[1])) else "false")' "${field}")" = "${expected}" ]; then
        return 0
      fi
    fi
    sleep 0.25
  done
  return 1
}

wait_for_json_fragment() {
  local url="$1"
  local fragment="$2"
  for _ in $(seq 1 240); do
    local body
    if body="$(curl -fsS "${url}" 2>/dev/null)"; then
      if [[ "${body}" == *"${fragment}"* ]]; then
        printf '%s' "${body}"
        return 0
      fi
    fi
    sleep 0.25
  done
  return 1
}

ingest_template() {
  local container_name="$1"
  local template="$2"
  docker exec "${container_name}" /stack-target/gewyvern/debug/gewyvern_socket_send \
    --tcp-socket 127.0.0.1:9000 \
    --template "${template}" >/dev/null
}

register_runtime() {
  local name="$1"
  local endpoint="$2"
  local environment="$3"
  local cluster="$4"
  local role="$5"
  local sidecar_endpoint="${6:-}"
  python3 - "$name" "$endpoint" "$environment" "$cluster" "$role" "$sidecar_endpoint" <<'PY'
import json, sys
name, endpoint, environment, cluster, role, sidecar_endpoint = sys.argv[1:7]
print(json.dumps({
    "name": name,
    "endpoint": endpoint,
    "sidecarEndpoint": sidecar_endpoint or None,
    "pairingToken": "stack-smoke",
    "capabilities": [],
    "tags": {
        "environment": environment,
        "cluster": cluster,
        "role": role,
    },
    "fetchCapabilities": True,
}))
PY
}

start_gewyvern "${GW_A_NAME}" "${GW_A_SOCKET_PORT}" "${GW_A_API_PORT}"
start_gewyvern "${GW_B_NAME}" "${GW_B_SOCKET_PORT}" "${GW_B_API_PORT}"

wait_http "http://127.0.0.1:${GW_A_API_PORT}/health" || {
  echo "gw-a did not become healthy" >&2
  docker ps -a --filter "name=${GW_A_NAME}" >&2 || true
  docker logs "${GW_A_NAME}" >&2 || true
  exit 1
}
wait_http "http://127.0.0.1:${GW_B_API_PORT}/health" || {
  echo "gw-b did not become healthy" >&2
  docker ps -a --filter "name=${GW_B_NAME}" >&2 || true
  docker logs "${GW_B_NAME}" >&2 || true
  exit 1
}

ingest_template "${GW_A_NAME}" udp
ingest_template "${GW_B_NAME}" udp

wait_for_meta_field "http://127.0.0.1:${GW_A_API_PORT}/v1/latest/meta" "has_analysis_json" "true" || {
  echo "gw-a never published analysis_json" >&2
  curl -fsS "http://127.0.0.1:${GW_A_API_PORT}/v1/latest/meta" >&2 || true
  docker logs "${GW_A_NAME}" >&2 || true
  exit 1
}

wait_for_meta_field "http://127.0.0.1:${GW_B_API_PORT}/v1/latest/meta" "has_analysis_json" "true" || {
  echo "gw-b never published analysis_json" >&2
  curl -fsS "http://127.0.0.1:${GW_B_API_PORT}/v1/latest/meta" >&2 || true
  docker logs "${GW_B_NAME}" >&2 || true
  exit 1
}

start_etragon

wait_http "http://127.0.0.1:${ET_A_API_PORT}/health" || {
  echo "etragon sidecar did not become healthy" >&2
  docker ps -a --filter "name=${ET_A_NAME}" >&2 || true
  docker logs "${ET_A_NAME}" >&2 || true
  exit 1
}

ETRAGON_OUTPUT_JSON="$(wait_for_json_fragment "http://127.0.0.1:${ET_A_API_PORT}/v1/latest/output.json" "\"augmentations\"")" || {
  echo "etragon sidecar never published output_json with augmentations" >&2
  curl -fsS "http://127.0.0.1:${ET_A_API_PORT}/v1/latest/output.json" >&2 || true
  docker logs "${ET_A_NAME}" >&2 || true
  exit 1
}
printf '%s' "${ETRAGON_OUTPUT_JSON}" | python3 -c 'import json, sys
payload = json.load(sys.stdin)
assert "output" in payload and isinstance(payload["output"], dict), payload
assert "augmentations" in payload["output"], payload
print("etragon-output-ok")' || {
  echo "etragon output validation failed" >&2
  echo "${ETRAGON_OUTPUT_JSON}" >&2
  docker logs "${ET_A_NAME}" >&2 || true
  exit 1
}

LESERPENT_STATE_PATH="${STATE_PATH}" \
  dotnet run --project "${LESERPENT_ROOT}/src/Leserpent/Leserpent.csproj" --no-launch-profile --urls "http://127.0.0.1:${LESP_PORT}" \
  >"${LESP_LOG}" 2>&1 &
LESP_PID=$!

wait_http "http://127.0.0.1:${LESP_PORT}/health" || {
  echo "leserpent did not become healthy" >&2
  cat "${LESP_LOG}" >&2 || true
  exit 1
}

curl -fsS \
  -X POST "http://127.0.0.1:${LESP_PORT}/v1/runtimes/register" \
  -H 'content-type: application/json' \
  -H 'X-Leserpent-Intent: mutate' \
  --data "$(register_runtime "gw-stack-a" "http://127.0.0.1:${GW_A_API_PORT}" "stack" "local" "with-sidecar" "http://127.0.0.1:${ET_A_API_PORT}")" >/dev/null

curl -fsS \
  -X POST "http://127.0.0.1:${LESP_PORT}/v1/runtimes/register" \
  -H 'content-type: application/json' \
  -H 'X-Leserpent-Intent: mutate' \
  --data "$(register_runtime "gw-stack-b" "http://127.0.0.1:${GW_B_API_PORT}" "stack" "local" "plain")" >/dev/null

curl -fsS \
  -X POST "http://127.0.0.1:${LESP_PORT}/v1/fleet/refresh-all?environment=stack" \
  -H 'X-Leserpent-Intent: mutate' >/dev/null

SUMMARY_JSON="$(curl -fsS "http://127.0.0.1:${LESP_PORT}/v1/fleet/summary?environment=stack")"
RUNTIMES_JSON="$(curl -fsS "http://127.0.0.1:${LESP_PORT}/v1/runtimes?environment=stack")"

printf '%s' "${SUMMARY_JSON}" | python3 -c 'import json, sys
payload = json.load(sys.stdin)
summary = payload["summary"]
assert summary["runtimeCount"] == 2, summary
assert summary["runtimesWithLatestSnapshot"] == 2, summary
assert summary["runtimesWithAnalysisJson"] == 2, summary
assert summary["runtimesWithPairedSidecar"] == 1, summary
assert summary["runtimesWithHealthySidecar"] == 1, summary
assert summary["runtimesWithObservedSidecarStatus"] == 1, summary
print("summary-ok")' || {
  echo "fleet summary validation failed" >&2
  echo "${SUMMARY_JSON}" >&2
  exit 1
}

printf '%s' "${RUNTIMES_JSON}" | python3 -c 'import json, sys
payload = json.load(sys.stdin)
runtimes = {item["name"]: item for item in payload["runtimes"]}
a = runtimes["gw-stack-a"]
b = runtimes["gw-stack-b"]
assert a["status"]["hasLatestSnapshot"] is True, a
assert a["status"]["hasAnalysisJson"] is True, a
assert a["sidecarEndpoint"], a
assert a["sidecarStatus"]["healthy"] is True, a
assert a["sidecarStatus"]["daemonStatus"] in {"ready", "degraded"}, a
assert b["status"]["hasLatestSnapshot"] is True, b
assert b["status"]["hasAnalysisJson"] is True, b
assert b["sidecarEndpoint"] is None, b
print("runtimes-ok")' || {
  echo "runtime detail validation failed" >&2
  echo "${RUNTIMES_JSON}" >&2
  exit 1
}

echo "three-module stack smoke: ok"
echo "leserpent=http://127.0.0.1:${LESP_PORT}"
echo "gewyvern_a=http://127.0.0.1:${GW_A_API_PORT}"
echo "gewyvern_b=http://127.0.0.1:${GW_B_API_PORT}"
echo "etragon_a=http://127.0.0.1:${ET_A_API_PORT}"
