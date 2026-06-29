#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
GEWY_ROOT="${ROOT}"
ETRAGON_ROOT="${ROOT}/apps/etragon"
LESERPENT_ROOT="${ROOT}/apps/leserpent"

if [ -x "${HOME}/.dotnet/dotnet" ]; then
  export DOTNET_ROOT="${DOTNET_ROOT:-${HOME}/.dotnet}"
  export PATH="${HOME}/.dotnet:${PATH}"
fi

IMAGE_TAG="${IMAGE_TAG:-gewyvern-stack-dev}"
SKIP_DOCKER_BUILD="${SKIP_DOCKER_BUILD:-false}"
DOCKER_BASE_IMAGE="${DOCKER_BASE_IMAGE:-ubuntu:24.04}"
DOCKER_APT_MIRROR="${DOCKER_APT_MIRROR:-}"
DOCKER_RUSTUP_INIT_URL="${DOCKER_RUSTUP_INIT_URL:-https://sh.rustup.rs}"
DOCKER_RUSTUP_INIT_FALLBACK_URL="${DOCKER_RUSTUP_INIT_FALLBACK_URL:-https://sh.rustup.rs}"
DOCKER_RUSTUP_DIST_SERVER="${DOCKER_RUSTUP_DIST_SERVER:-https://static.rust-lang.org}"
DOCKER_RUSTUP_UPDATE_ROOT="${DOCKER_RUSTUP_UPDATE_ROOT:-https://static.rust-lang.org/rustup}"
DOCKER_RUSTUP_INSTALL_TIMEOUT_SECONDS="${DOCKER_RUSTUP_INSTALL_TIMEOUT_SECONDS:-600}"
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
ET_A_ADMIN_TOKEN="${ET_A_ADMIN_TOKEN:-stack-smoke-admin-token}"
LESERPENT_DOTNET_RESTORE_FIRST="${LESERPENT_DOTNET_RESTORE_FIRST:-false}"
LESERPENT_DOTNET_IGNORE_FAILED_SOURCES="${LESERPENT_DOTNET_IGNORE_FAILED_SOURCES:-false}"
LESERPENT_DOTNET_NO_RESTORE="${LESERPENT_DOTNET_NO_RESTORE:-false}"

WORK_DIR="$(mktemp -d "${TMPDIR:-/tmp}/three-module-stack.XXXXXX")"
TARGET_CACHE_DIR="${WORK_DIR}/target-cache"
CARGO_CACHE_DIR="${CARGO_CACHE_DIR:-${CARGO_HOME:-${HOME}/.cargo}}"
CARGO_NET_OFFLINE="${CARGO_NET_OFFLINE:-false}"
STATE_PATH="${WORK_DIR}/leserpent-state.json"
LESP_LOG="${WORK_DIR}/leserpent.log"
RESILIENCE_SUMMARY_PATH="${RESILIENCE_SUMMARY_PATH:-${WORK_DIR}/resilience-summary.txt}"
mkdir -p "${TARGET_CACHE_DIR}"
mkdir -p "${CARGO_CACHE_DIR}"

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
require_cmd dotnet

if ! docker info >/dev/null 2>&1; then
  echo "docker daemon is not reachable; start Docker Desktop or another local daemon and retry" >&2
  exit 1
fi

if [ ! -d "${ETRAGON_ROOT}" ]; then
  echo "missing etragon app at ${ETRAGON_ROOT}" >&2
  exit 1
fi

if [ ! -d "${LESERPENT_ROOT}" ]; then
  echo "missing leserpent app at ${LESERPENT_ROOT}" >&2
  exit 1
fi

if [ "${SKIP_DOCKER_BUILD}" != "true" ]; then
  docker build \
    --build-arg "BASE_IMAGE=${DOCKER_BASE_IMAGE}" \
    --build-arg "APT_MIRROR=${DOCKER_APT_MIRROR}" \
    --build-arg "RUSTUP_INIT_URL=${DOCKER_RUSTUP_INIT_URL}" \
    --build-arg "RUSTUP_INIT_FALLBACK_URL=${DOCKER_RUSTUP_INIT_FALLBACK_URL}" \
    --build-arg "RUSTUP_DIST_SERVER=${DOCKER_RUSTUP_DIST_SERVER}" \
    --build-arg "RUSTUP_UPDATE_ROOT=${DOCKER_RUSTUP_UPDATE_ROOT}" \
    --build-arg "RUSTUP_INSTALL_TIMEOUT_SECONDS=${DOCKER_RUSTUP_INSTALL_TIMEOUT_SECONDS}" \
    -t "${IMAGE_TAG}" \
    -f "${GEWY_ROOT}/docker/linux-dev/Dockerfile" \
    "${GEWY_ROOT}" >/dev/null
elif ! docker image inspect "${IMAGE_TAG}" >/dev/null 2>&1; then
  echo "SKIP_DOCKER_BUILD=true but image is missing: ${IMAGE_TAG}" >&2
  exit 1
fi

docker network rm "${NETWORK_NAME}" >/dev/null 2>&1 || true
docker network create "${NETWORK_NAME}" >/dev/null

docker run --rm \
  -v "${ROOT}:/workspace/dev/gewyvern" \
  -v "${TARGET_CACHE_DIR}:/stack-target" \
  -v "${CARGO_CACHE_DIR}:/cargo-cache" \
  -e CARGO_HOME=/cargo-cache \
  -e "CARGO_NET_OFFLINE=${CARGO_NET_OFFLINE}" \
  "${IMAGE_TAG}" \
  bash -lc '
    set -euo pipefail
    export CARGO_TARGET_DIR=/stack-target/etragon
    cd /workspace/dev/gewyvern/apps/etragon
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
    -p "127.0.0.1:${socket_port}:9000" \
    -p "127.0.0.1:${api_port}:9100" \
    -v "${ROOT}:/workspace/dev/gewyvern" \
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
    -p "127.0.0.1:${ET_A_API_PORT}:4321" \
    -e "ETRAGON_ADMIN_TOKEN=${ET_A_ADMIN_TOKEN}" \
    -v "${ROOT}:/workspace/dev/gewyvern" \
    -v "${TARGET_CACHE_DIR}:/stack-target" \
    "${IMAGE_TAG}" \
    bash -lc "
      set -euo pipefail
      export CARGO_TARGET_DIR=/stack-target/etragon
      cd /workspace/dev/gewyvern/apps/etragon
      /stack-target/etragon/debug/etragon \
        serve-python-url \
        http://${GW_A_NAME}:9100/v1/latest/analysis.json \
        --bind 0.0.0.0:4321 \
        --interval-ms 500 \
        --python-worker /workspace/dev/gewyvern/apps/etragon/scripts/python_baseline_worker.py \
        --python-state /tmp/etragon-online-state.json \
        --daemon-state /tmp/etragon-daemon-state.json
    " >/dev/null
}

etragon_curl() {
  curl -fsS -H "X-Etragon-Admin-Token: ${ET_A_ADMIN_TOKEN}" "$@"
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

stack_probe() {
  local profile="$1"
  local url="$2"
  local output="$3"
  shift 3
  (
    cd "${GEWY_ROOT}"
    cargo run --quiet --bin gewyvern_validate -- \
      stack-probe \
      --profile "${profile}" \
      --url "${url}" \
      --output "${output}" \
      "$@"
  ) >/dev/null
  cat "${output}"
}
ingest_template() {
  local container_name="$1"
  local template="$2"
  docker exec "${container_name}" /stack-target/gewyvern/debug/gewyvern_socket_send \
    --tcp-socket 127.0.0.1:9000 \
    --template "${template}" >/dev/null
}
inject_socket_bad_json() {
  local container_name="$1"
  local count="${2:-4}"
  docker exec "${container_name}" bash -lc "
    set -euo pipefail
    for _ in \$(seq 1 ${count}); do
      exec 3<>/dev/tcp/127.0.0.1/9000
      printf '{\"bad\":\"json\"\\n' >&3 || true
      exec 3>&-
      exec 3<&-
    done
  " >/dev/null
}
register_runtime() {
  local name="$1"
  local endpoint="$2"
  local environment="$3"
  local cluster="$4"
  local role="$5"
  local sidecar_endpoint="${6:-}"
  local sidecar_admin_token="${7:-}"
  local args=(
    stack-register-runtime-json
    --name "${name}"
    --endpoint "${endpoint}"
    --environment "${environment}"
    --cluster "${cluster}"
    --role "${role}"
  )
  if [ -n "${sidecar_endpoint}" ]; then
    args+=(--sidecar-endpoint "${sidecar_endpoint}")
  fi
  if [ -n "${sidecar_admin_token}" ]; then
    args+=(--sidecar-admin-token "${sidecar_admin_token}")
  fi
  (
    cd "${GEWY_ROOT}"
    cargo run --quiet --bin gewyvern_validate -- "${args[@]}"
  )
}
start_gewyvern "${GW_A_NAME}" "${GW_A_SOCKET_PORT}" "${GW_A_API_PORT}"
start_gewyvern "${GW_B_NAME}" "${GW_B_SOCKET_PORT}" "${GW_B_API_PORT}"
stack_probe "http-ready" "http://127.0.0.1:${GW_A_API_PORT}/health" "${WORK_DIR}/gw-a-health.json" >/dev/null || {
  echo "gw-a did not become healthy" >&2
  docker ps -a --filter "name=${GW_A_NAME}" >&2 || true
  docker logs "${GW_A_NAME}" >&2 || true
  exit 1
}
stack_probe "http-ready" "http://127.0.0.1:${GW_B_API_PORT}/health" "${WORK_DIR}/gw-b-health.json" >/dev/null || {
  echo "gw-b did not become healthy" >&2
  docker ps -a --filter "name=${GW_B_NAME}" >&2 || true
  docker logs "${GW_B_NAME}" >&2 || true
  exit 1
}
ingest_template "${GW_A_NAME}" udp
ingest_template "${GW_B_NAME}" udp
GW_A_RESILIENCE_JSON="$(stack_probe "resilience-healthy" "http://127.0.0.1:${GW_A_API_PORT}/v1/runtime/resilience.json" "${WORK_DIR}/gw-a-resilience.json")" || {
  echo "gw-a never published a healthy resilience surface" >&2
  curl -fsS "http://127.0.0.1:${GW_A_API_PORT}/v1/runtime/resilience.json" >&2 || true
  docker logs "${GW_A_NAME}" >&2 || true
  exit 1
}
GW_B_RESILIENCE_JSON="$(stack_probe "resilience-healthy" "http://127.0.0.1:${GW_B_API_PORT}/v1/runtime/resilience.json" "${WORK_DIR}/gw-b-resilience.json")" || {
  echo "gw-b never published a healthy resilience surface" >&2
  curl -fsS "http://127.0.0.1:${GW_B_API_PORT}/v1/runtime/resilience.json" >&2 || true
  docker logs "${GW_B_NAME}" >&2 || true
  exit 1
}
inject_socket_bad_json "${GW_B_NAME}" 5
GW_B_DEGRADED_HEALTH_JSON="$(stack_probe "health-degraded" "http://127.0.0.1:${GW_B_API_PORT}/health" "${WORK_DIR}/gw-b-health-degraded.json")" || {
  echo "gw-b never exposed resilience_degraded=true after repeated socket failures" >&2
  curl -fsS "http://127.0.0.1:${GW_B_API_PORT}/health" >&2 || true
  docker logs "${GW_B_NAME}" >&2 || true
  exit 1
}
GW_B_DEGRADED_RESILIENCE_JSON="$(stack_probe "resilience-degraded" "http://127.0.0.1:${GW_B_API_PORT}/v1/runtime/resilience.json" "${WORK_DIR}/gw-b-resilience-degraded.json")" || {
  echo "gw-b never published a degraded resilience surface after repeated socket failures" >&2
  curl -fsS "http://127.0.0.1:${GW_B_API_PORT}/v1/runtime/resilience.json" >&2 || true
  docker logs "${GW_B_NAME}" >&2 || true
  exit 1
}
stack_probe "meta-has-analysis" "http://127.0.0.1:${GW_A_API_PORT}/v1/latest/meta" "${WORK_DIR}/gw-a-meta.json" >/dev/null || {
  echo "gw-a never published analysis_json" >&2
  curl -fsS "http://127.0.0.1:${GW_A_API_PORT}/v1/latest/meta" >&2 || true
  docker logs "${GW_A_NAME}" >&2 || true
  exit 1
}
stack_probe "meta-has-analysis" "http://127.0.0.1:${GW_B_API_PORT}/v1/latest/meta" "${WORK_DIR}/gw-b-meta.json" >/dev/null || {
  echo "gw-b never published analysis_json" >&2
  curl -fsS "http://127.0.0.1:${GW_B_API_PORT}/v1/latest/meta" >&2 || true
  docker logs "${GW_B_NAME}" >&2 || true
  exit 1
}
start_etragon
stack_probe "http-ready" "http://127.0.0.1:${ET_A_API_PORT}/health" "${WORK_DIR}/etragon-health.json" --admin-token "${ET_A_ADMIN_TOKEN}" >/dev/null || {
  echo "etragon sidecar did not become healthy" >&2
  docker ps -a --filter "name=${ET_A_NAME}" >&2 || true
  docker logs "${ET_A_NAME}" >&2 || true
  exit 1
}
ETRAGON_STATUS_JSON="$(stack_probe "etragon-status" "http://127.0.0.1:${ET_A_API_PORT}/v1/latest/status" "${WORK_DIR}/etragon-status.json" --admin-token "${ET_A_ADMIN_TOKEN}")" || {
  echo "etragon sidecar never reached ready/degraded daemon status" >&2
  etragon_curl "http://127.0.0.1:${ET_A_API_PORT}/v1/latest/status" >&2 || true
  docker logs "${ET_A_NAME}" >&2 || true
  exit 1
}
echo "etragon-status-ok"
ETRAGON_OUTPUT_JSON="$(stack_probe "etragon-output" "http://127.0.0.1:${ET_A_API_PORT}/v1/latest/output.json" "${WORK_DIR}/etragon-output.json" --admin-token "${ET_A_ADMIN_TOKEN}")" || {
  echo "etragon sidecar never published output_json with augmentations" >&2
  etragon_curl "http://127.0.0.1:${ET_A_API_PORT}/v1/latest/output.json" >&2 || true
  docker logs "${ET_A_NAME}" >&2 || true
  exit 1
}
echo "etragon-output-ok"
LESERPENT_DOTNET_RESTORE_ARGS=()
if [ "${LESERPENT_DOTNET_IGNORE_FAILED_SOURCES}" = "true" ]; then
  LESERPENT_DOTNET_RESTORE_ARGS+=(--ignore-failed-sources)
fi
if [ "${LESERPENT_DOTNET_RESTORE_FIRST}" = "true" ]; then
  DOTNET_CLI_TELEMETRY_OPTOUT=1 \
    dotnet restore "${LESERPENT_ROOT}/src/Leserpent/Leserpent.csproj" "${LESERPENT_DOTNET_RESTORE_ARGS[@]}"
fi
LESERPENT_DOTNET_RUN_ARGS=()
if [ "${LESERPENT_DOTNET_NO_RESTORE}" = "true" ]; then
  LESERPENT_DOTNET_RUN_ARGS+=(--no-restore)
fi
LESERPENT_STATE_PATH="${STATE_PATH}" \
  DOTNET_CLI_TELEMETRY_OPTOUT=1 \
  dotnet run --project "${LESERPENT_ROOT}/src/Leserpent/Leserpent.csproj" "${LESERPENT_DOTNET_RUN_ARGS[@]}" --no-launch-profile --urls "http://127.0.0.1:${LESP_PORT}" \
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
  --data "$(register_runtime "gw-stack-a" "http://127.0.0.1:${GW_A_API_PORT}" "stack" "local" "with-sidecar" "http://127.0.0.1:${ET_A_API_PORT}" "${ET_A_ADMIN_TOKEN}")" >/dev/null
curl -fsS \
  -X POST "http://127.0.0.1:${LESP_PORT}/v1/runtimes/register" \
  -H 'content-type: application/json' \
  -H 'X-Leserpent-Intent: mutate' \
  --data "$(register_runtime "gw-stack-b" "http://127.0.0.1:${GW_B_API_PORT}" "stack" "local" "plain")" >/dev/null
curl -fsS \
  -X POST "http://127.0.0.1:${LESP_PORT}/v1/fleet/refresh-all?environment=stack" \
  -H 'X-Leserpent-Intent: mutate' >/dev/null
RUNTIMES_JSON="$(curl -fsS "http://127.0.0.1:${LESP_PORT}/v1/runtimes?environment=stack")"
printf '%s' "${RUNTIMES_JSON}" >"${WORK_DIR}/leserpent-runtimes.json"
(
  cd "${GEWY_ROOT}"
  cargo run --quiet --bin gewyvern_validate -- \
    stack-check-json \
    --profile leserpent-runtimes-sidecar \
    --input "${WORK_DIR}/leserpent-runtimes.json"
) >/dev/null || {
  echo "leserpent never observed a healthy sidecar for gw-stack-a" >&2
  curl -fsS "http://127.0.0.1:${LESP_PORT}/v1/runtimes?environment=stack" >&2 || true
  docker logs "${ET_A_NAME}" >&2 || true
  cat "${LESP_LOG}" >&2 || true
  exit 1
}
SUMMARY_JSON="$(curl -fsS "http://127.0.0.1:${LESP_PORT}/v1/fleet/summary?environment=stack")"
printf '%s' "${SUMMARY_JSON}" >"${WORK_DIR}/leserpent-summary.json"
(
  cd "${GEWY_ROOT}"
  cargo run --quiet --bin gewyvern_validate -- \
    stack-check-json \
    --profile leserpent-summary \
    --input "${WORK_DIR}/leserpent-summary.json"
) >/dev/null || {
  echo "leserpent never published the expected fleet summary" >&2
  curl -fsS "http://127.0.0.1:${LESP_PORT}/v1/fleet/summary?environment=stack" >&2 || true
  cat "${LESP_LOG}" >&2 || true
  exit 1
}
echo "summary-ok"
(
  cd "${GEWY_ROOT}"
  cargo run --quiet --bin gewyvern_validate -- \
    stack-check-json \
    --profile leserpent-runtime-detail \
    --input "${WORK_DIR}/leserpent-runtimes.json"
) >/dev/null
echo "runtimes-ok"
echo "gw-a-resilience-ok"
echo "gw-b-resilience-ok"
echo "gw-b-health-degraded-ok"
echo "gw-b-resilience-degraded-ok"
(
  cd "${GEWY_ROOT}"
  cargo run --quiet --bin gewyvern_validate -- \
    stack-resilience-summary \
    --healthy-a "${WORK_DIR}/gw-a-resilience.json" \
    --healthy-b "${WORK_DIR}/gw-b-resilience.json" \
    --degraded-b "${WORK_DIR}/gw-b-resilience-degraded.json" \
    --output "${RESILIENCE_SUMMARY_PATH}"
) >/dev/null
echo "three-module stack smoke: ok"
echo "leserpent=http://127.0.0.1:${LESP_PORT}"
echo "gewyvern_a=http://127.0.0.1:${GW_A_API_PORT}"
echo "gewyvern_b=http://127.0.0.1:${GW_B_API_PORT}"
echo "etragon_a=http://127.0.0.1:${ET_A_API_PORT}"
echo "resilience_summary=${RESILIENCE_SUMMARY_PATH}"
