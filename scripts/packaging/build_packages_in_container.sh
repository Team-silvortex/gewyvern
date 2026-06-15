#!/usr/bin/env bash

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
IMAGE_TAG="${GEWY_DOCKER_IMAGE_TAG:-gewyvern-linux-dev:packaging}"

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

docker build -t "${IMAGE_TAG}" -f "${ROOT}/docker/linux-dev/Dockerfile" "${ROOT}"
docker run --rm \
  -v "${ROOT}:/workspace" \
  -w /workspace \
  "${IMAGE_TAG}" \
  bash scripts/packaging/build_packages.sh "${FORWARDED_ARGS[@]}"
