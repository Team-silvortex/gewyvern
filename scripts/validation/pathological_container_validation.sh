#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

IMAGE_TAG="${IMAGE_TAG:-gewyvern-stack-dev}"
SKIP_DOCKER_BUILD="${SKIP_DOCKER_BUILD:-false}"
DOCKER_BASE_IMAGE="${DOCKER_BASE_IMAGE:-ubuntu:24.04}"
DOCKER_APT_MIRROR="${DOCKER_APT_MIRROR:-}"
DOCKER_RUSTUP_INIT_URL="${DOCKER_RUSTUP_INIT_URL:-https://sh.rustup.rs}"
DOCKER_RUSTUP_INIT_FALLBACK_URL="${DOCKER_RUSTUP_INIT_FALLBACK_URL:-https://sh.rustup.rs}"
DOCKER_RUSTUP_DIST_SERVER="${DOCKER_RUSTUP_DIST_SERVER:-https://static.rust-lang.org}"
DOCKER_RUSTUP_UPDATE_ROOT="${DOCKER_RUSTUP_UPDATE_ROOT:-https://static.rust-lang.org/rustup}"
DOCKER_RUSTUP_INSTALL_TIMEOUT_SECONDS="${DOCKER_RUSTUP_INSTALL_TIMEOUT_SECONDS:-600}"

NETWORK_NAME="${NETWORK_NAME:-gewyvern-pathology-net}"
GW_NAME="${GW_NAME:-gewyvern-pathology-runtime}"
PATHO_PREFIX="${PATHO_PREFIX:-gewyvern-pathology}"
SOCKET_PORT="${SOCKET_PORT:-19201}"
API_PORT="${API_PORT:-19301}"
OUT_DIR="${1:-${ROOT}/target/validation/pathological-container}"
PATHOLOGY_FIXTURE_DIR="${ROOT}/tests/pathological-containers"
WORK_DIR="$(mktemp -d "${TMPDIR:-/tmp}/gewyvern-pathology.XXXXXX")"
TARGET_CACHE_DIR="${WORK_DIR}/target-cache"
CARGO_CACHE_DIR="${CARGO_CACHE_DIR:-${CARGO_HOME:-${HOME}/.cargo}}"
CARGO_NET_OFFLINE="${CARGO_NET_OFFLINE:-false}"

mkdir -p "${OUT_DIR}" "${TARGET_CACHE_DIR}" "${CARGO_CACHE_DIR}"

cleanup() {
  docker rm -f "${GW_NAME}" >/dev/null 2>&1 || true
  docker rm -f "${PATHO_PREFIX}-truncated" >/dev/null 2>&1 || true
  docker rm -f "${PATHO_PREFIX}-disconnect" >/dev/null 2>&1 || true
  docker rm -f "${PATHO_PREFIX}-oversize" >/dev/null 2>&1 || true
  docker rm -f "${PATHO_PREFIX}-slow-drip" >/dev/null 2>&1 || true
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
require_cmd cargo

if ! docker info >/dev/null 2>&1; then
  echo "docker daemon is not reachable" >&2
  exit 1
fi
if [ ! -f "${PATHOLOGY_FIXTURE_DIR}/pathology_client.py" ]; then
  echo "missing pathological container fixture source: ${PATHOLOGY_FIXTURE_DIR}" >&2
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
    -f "${ROOT}/docker/linux-dev/Dockerfile" \
    "${ROOT}" >/dev/null
elif ! docker image inspect "${IMAGE_TAG}" >/dev/null 2>&1; then
  echo "SKIP_DOCKER_BUILD=true but image is missing: ${IMAGE_TAG}" >&2
  exit 1
fi

docker run --rm \
  -v "${ROOT}:/workspace/dev/gewyvern" \
  -v "${TARGET_CACHE_DIR}:/stack-target" \
  -v "${CARGO_CACHE_DIR}:/cargo-cache" \
  -e CARGO_HOME=/cargo-cache \
  -e "CARGO_NET_OFFLINE=${CARGO_NET_OFFLINE}" \
  "${IMAGE_TAG}" \
  bash -lc '
    set -euo pipefail
    export CARGO_TARGET_DIR=/stack-target/gewyvern
    cd /workspace/dev/gewyvern
    cargo build --quiet --bin gewyvern --bin gewyvern_socket_send
  '

docker network rm "${NETWORK_NAME}" >/dev/null 2>&1 || true
docker network create "${NETWORK_NAME}" >/dev/null

docker rm -f "${GW_NAME}" >/dev/null 2>&1 || true
docker run -d \
  --name "${GW_NAME}" \
  --network "${NETWORK_NAME}" \
  -p "127.0.0.1:${SOCKET_PORT}:9000" \
  -p "127.0.0.1:${API_PORT}:9100" \
  -v "${ROOT}:/workspace/dev/gewyvern" \
  -v "${TARGET_CACHE_DIR}:/stack-target" \
  "${IMAGE_TAG}" \
  bash -lc '
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
  ' >/dev/null

stack_probe() {
  local profile="$1"
  local url="$2"
  local output="$3"
  (
    cd "${ROOT}"
    cargo run --quiet --bin gewyvern_validate -- \
      stack-probe \
      --profile "${profile}" \
      --url "${url}" \
      --output "${output}"
  ) >/dev/null
}

run_pathology_container() {
  local suffix="$1"
  local scenario="$2"
  local count="$3"
  docker rm -f "${PATHO_PREFIX}-${suffix}" >/dev/null 2>&1 || true
  docker run --rm \
    --name "${PATHO_PREFIX}-${suffix}" \
    --network "${NETWORK_NAME}" \
    -v "${PATHOLOGY_FIXTURE_DIR}:/pathology:ro" \
    "${IMAGE_TAG}" \
    python3 /pathology/pathology_client.py \
      --scenario "${scenario}" \
      --host "${GW_NAME}" \
      --port 9000 \
      --count "${count}" \
      >"${OUT_DIR}/${suffix}.log"
}

stack_probe "http-ready" "http://127.0.0.1:${API_PORT}/health" "${OUT_DIR}/health-ready.json"

docker exec "${GW_NAME}" /stack-target/gewyvern/debug/gewyvern_socket_send \
  --tcp-socket 127.0.0.1:9000 \
  --template udp >/dev/null

stack_probe \
  "resilience-healthy" \
  "http://127.0.0.1:${API_PORT}/v1/runtime/resilience.json" \
  "${OUT_DIR}/resilience-healthy.json"

run_pathology_container "truncated" "truncated-json" 4
run_pathology_container "disconnect" "empty-disconnect" 4
run_pathology_container "slow-drip" "slow-drip" 3
run_pathology_container "oversize" "oversize-line" 3

stack_probe \
  "health-degraded" \
  "http://127.0.0.1:${API_PORT}/health" \
  "${OUT_DIR}/health-degraded.json"

stack_probe \
  "resilience-degraded" \
  "http://127.0.0.1:${API_PORT}/v1/runtime/resilience.json" \
  "${OUT_DIR}/resilience-degraded.json"

docker exec "${GW_NAME}" /stack-target/gewyvern/debug/gewyvern_socket_send \
  --tcp-socket 127.0.0.1:9000 \
  --template udp >/dev/null

stack_probe \
  "meta-has-analysis" \
  "http://127.0.0.1:${API_PORT}/v1/latest/meta" \
  "${OUT_DIR}/meta-after-pathology.json"

docker logs "${GW_NAME}" >"${OUT_DIR}/runtime.log" 2>&1 || true

if ! grep -Eq "socket_session_run_failed" "${OUT_DIR}/runtime.log"; then
  echo "runtime log did not preserve expected socket resilience evidence" >&2
  exit 1
fi
if ! grep -Eq "unexpected_token|fact_line_exceeded_65536_bytes" "${OUT_DIR}/runtime.log"; then
  echo "runtime log did not preserve expected pathological input class evidence" >&2
  exit 1
fi

cat >"${OUT_DIR}/summary.txt" <<EOF
pathological container validation: ok
host_api=http://127.0.0.1:${API_PORT}
host_socket=127.0.0.1:${SOCKET_PORT}
checked=healthy_baseline,truncated_json,empty_disconnect,slow_drip,oversize_line,degraded_health,degraded_resilience,post_fault_analysis,log_evidence
evidence=${OUT_DIR}
EOF

cat "${OUT_DIR}/summary.txt"
