#!/usr/bin/env bash

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PACKAGES_DIR="${ROOT}/target/packages"
DEB_IMAGE="${GEWY_DEB_SMOKE_IMAGE:-ubuntu:24.04}"
RPM_IMAGE="${GEWY_RPM_SMOKE_IMAGE:-fedora:41}"

usage() {
  cat <<'EOF'
Usage: scripts/package_install_smoke.sh [--deb] [--rpm]

Install the most recent local .deb and/or .rpm artifact inside clean Linux
containers, then run a very-light post-install smoke:

- gewyvern --help
- gewyc --help
- gewyvern_socket_send --help
- packaged DSL/protocol asset directories exist

By default, both DEB and RPM smoke paths run.
EOF
}

RUN_DEB=1
RUN_RPM=1

while [[ $# -gt 0 ]]; do
  case "$1" in
    --deb)
      RUN_DEB=1
      RUN_RPM=0
      shift
      ;;
    --rpm)
      RUN_DEB=0
      RUN_RPM=1
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "unknown argument: $1" >&2
      usage >&2
      exit 1
      ;;
  esac
done

if ! command -v docker >/dev/null 2>&1; then
  echo "docker is required for package install smoke" >&2
  exit 1
fi

if ! docker info >/dev/null 2>&1; then
  echo "docker daemon is not reachable; start Docker Desktop or another local daemon and retry" >&2
  exit 1
fi

find_latest_deb() {
  find "${PACKAGES_DIR}" -maxdepth 1 -type f -name '*.deb' | sort | tail -n 1
}

find_latest_rpm() {
  find "${PACKAGES_DIR}/rpm" -maxdepth 1 -type f -name '*.rpm' | sort | tail -n 1
}

run_deb_smoke() {
  local deb_path
  deb_path="$(find_latest_deb)"
  if [[ -z "${deb_path}" ]]; then
    echo "no .deb artifact found under ${PACKAGES_DIR}" >&2
    exit 1
  fi

  docker run --rm \
    -v "${PACKAGES_DIR}:/packages:ro" \
    "${DEB_IMAGE}" \
    bash -lc "
      set -euo pipefail
      apt-get update >/dev/null
      apt-get install -y /packages/$(basename "${deb_path}") >/dev/null
      command -v gewyvern >/dev/null
      command -v gewyc >/dev/null
      command -v gewyvern_socket_send >/dev/null
      gewyvern --list-protocols >/dev/null
      gewyc /usr/share/gewyvern/dsl/http_request_path.gewy --json >/dev/null
      test -d /usr/share/gewyvern/dsl
      test -d /usr/share/gewyvern/protocols
    "

  echo "deb install smoke: ok (${deb_path})"
}

run_rpm_smoke() {
  local rpm_path
  rpm_path="$(find_latest_rpm)"
  if [[ -z "${rpm_path}" ]]; then
    echo "no .rpm artifact found under ${PACKAGES_DIR}/rpm" >&2
    exit 1
  fi

  docker run --rm \
    -v "${PACKAGES_DIR}/rpm:/packages:ro" \
    "${RPM_IMAGE}" \
    bash -lc "
      set -euo pipefail
      dnf install -y /packages/$(basename "${rpm_path}") >/dev/null
      command -v gewyvern >/dev/null
      command -v gewyc >/dev/null
      command -v gewyvern_socket_send >/dev/null
      gewyvern --list-protocols >/dev/null
      gewyc /usr/share/gewyvern/dsl/http_request_path.gewy --json >/dev/null
      test -d /usr/share/gewyvern/dsl
      test -d /usr/share/gewyvern/protocols
    "

  echo "rpm install smoke: ok (${rpm_path})"
}

if [[ "${RUN_DEB}" -eq 1 ]]; then
  run_deb_smoke
fi

if [[ "${RUN_RPM}" -eq 1 ]]; then
  run_rpm_smoke
fi

echo "package install smoke: ok"
