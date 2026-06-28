#!/usr/bin/env bash

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
IMAGE_TAG="${GEWY_DOCKER_IMAGE_TAG:-gewyvern-linux-dev:packaging}"
DOCKER_BASE_IMAGE="${DOCKER_BASE_IMAGE:-ubuntu:24.04}"
DOCKER_APT_MIRROR="${DOCKER_APT_MIRROR:-}"
DOCKER_RUSTUP_INIT_URL="${DOCKER_RUSTUP_INIT_URL:-https://sh.rustup.rs}"
DOCKER_RUSTUP_INIT_FALLBACK_URL="${DOCKER_RUSTUP_INIT_FALLBACK_URL:-https://sh.rustup.rs}"
DOCKER_RUSTUP_DIST_SERVER="${DOCKER_RUSTUP_DIST_SERVER:-https://static.rust-lang.org}"
DOCKER_RUSTUP_UPDATE_ROOT="${DOCKER_RUSTUP_UPDATE_ROOT:-https://static.rust-lang.org/rustup}"
DOCKER_RUSTUP_INSTALL_TIMEOUT_SECONDS="${DOCKER_RUSTUP_INSTALL_TIMEOUT_SECONDS:-600}"
CARGO_CACHE_DIR="${CARGO_CACHE_DIR:-${CARGO_HOME:-${HOME}/.cargo}}"
CARGO_NET_OFFLINE="${CARGO_NET_OFFLINE:-false}"

usage() {
  cat <<'EOF'
Usage: scripts/packaging/build_packages_in_container.sh [--format deb|rpm|all] [--layout-only]

Build DEB/RPM artifacts inside the bundled Linux development container.
All arguments after `--` are forwarded to `scripts/packaging/build_packages.sh`.
EOF
}

FORWARDED_ARGS=()
if [[ $# -gt 0 ]]; then
  case "$1" in
    -h|--help)
      usage
      exit 0
      ;;
  esac
  FORWARDED_ARGS=("$@")
fi

if ! command -v docker >/dev/null 2>&1; then
  echo "docker is required for containerized package builds" >&2
  exit 1
fi

if ! docker info >/dev/null 2>&1; then
  echo "docker daemon is not reachable; start Docker Desktop or another local daemon and retry" >&2
  exit 1
fi

mkdir -p "${CARGO_CACHE_DIR}"

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
  "${ROOT}"
docker run --rm \
  -v "${ROOT}:/workspace" \
  -v "${CARGO_CACHE_DIR}:/cargo-cache" \
  -e CARGO_HOME=/cargo-cache \
  -e "CARGO_NET_OFFLINE=${CARGO_NET_OFFLINE}" \
  -w /workspace \
  "${IMAGE_TAG}" \
  bash scripts/packaging/build_packages.sh "${FORWARDED_ARGS[@]}"
