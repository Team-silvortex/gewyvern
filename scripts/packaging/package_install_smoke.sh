#!/usr/bin/env bash

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
source "${ROOT}/scripts/packaging/container_validation_common.sh"
PACKAGES_DIR="${ROOT}/target/packages"
DEB_IMAGE="${GEWY_DEB_SMOKE_IMAGE:-ubuntu:24.04}"
RPM_IMAGE="${GEWY_RPM_SMOKE_IMAGE:-fedora:41}"
RELEASE_LINE="${GEWY_RELEASE_LINE:-v0.18.x}"
DEB_APT_MIRROR="${GEWY_DEB_APT_MIRROR:-}"
RPM_DNF_MIRROR="${GEWY_RPM_DNF_MIRROR:-}"

usage() {
  cat <<'EOF'
Usage: scripts/packaging/package_install_smoke.sh [--deb] [--rpm]

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

container_validation_require_docker "package install smoke"

find_latest_deb() {
  find "${PACKAGES_DIR}" -maxdepth 1 -type f -name '*.deb' | sort | tail -n 1
}

find_latest_rpm() {
  find "${PACKAGES_DIR}/rpm" -maxdepth 1 -type f -name '*.rpm' | sort | tail -n 1
}

deb_preamble() {
  cat <<EOF
if [ -n "${DEB_APT_MIRROR}" ]; then
  sed -i "s|http://archive.ubuntu.com/ubuntu|${DEB_APT_MIRROR}|g; s|http://security.ubuntu.com/ubuntu|${DEB_APT_MIRROR}|g" /etc/apt/sources.list /etc/apt/sources.list.d/*.sources 2>/dev/null || true
fi
EOF
}

rpm_preamble() {
  cat <<EOF
if [ -n "${RPM_DNF_MIRROR}" ]; then
  sed -i "s|^metalink=|#metalink=|g; s|^mirrorlist=|#mirrorlist=|g; s|^#baseurl=http://download.example/pub/fedora/linux|baseurl=${RPM_DNF_MIRROR}|g; s|^#baseurl=https://download.example/pub/fedora/linux|baseurl=${RPM_DNF_MIRROR}|g" /etc/yum.repos.d/*.repo 2>/dev/null || true
fi
EOF
}

run_deb_smoke() {
  local deb_path
  deb_path="$(find_latest_deb)"
  if [[ -z "${deb_path}" ]]; then
    echo "no .deb artifact found under ${PACKAGES_DIR}" >&2
    exit 1
  fi

  container_validation_docker_run \
    -v "${PACKAGES_DIR}:/packages:ro" \
    "${DEB_IMAGE}" \
    bash -lc "
      set -euo pipefail
      $(deb_preamble)
      dpkg-deb -c /packages/$(basename "${deb_path}") >/tmp/gewyvern-package-contents.txt
      grep -q './usr/share/doc/gewyvern/LICENSE' /tmp/gewyvern-package-contents.txt
      apt-get update >/dev/null
      apt-get install -y /packages/$(basename "${deb_path}") >/dev/null
      command -v gewyvern >/dev/null
      command -v gewyc >/dev/null
      command -v gewyvern_socket_send >/dev/null
      gewyvern --list-protocols >/dev/null
      gewyc /usr/share/gewyvern/dsl/http_request_path.gewy --json >/dev/null
      test -d /usr/share/gewyvern/dsl
      test -d /usr/share/gewyvern/protocols
      test -f /usr/share/gewyvern/package-compat.toml
      grep -q '^schema_version = 1$' /usr/share/gewyvern/package-compat.toml
      grep -q '^release_line = \"${RELEASE_LINE}\"$' /usr/share/gewyvern/package-compat.toml
      test -f /usr/share/gewyvern/examples/gewyvern.toml.example
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

  container_validation_docker_run \
    -v "${PACKAGES_DIR}/rpm:/packages:ro" \
    "${RPM_IMAGE}" \
    bash -lc "
      set -euo pipefail
      $(rpm_preamble)
      rpm -qpl /packages/$(basename "${rpm_path}") >/tmp/gewyvern-package-contents.txt
      grep -q '/usr/share/doc/gewyvern/LICENSE' /tmp/gewyvern-package-contents.txt
      rpm -Uvh /packages/$(basename "${rpm_path}") >/dev/null || dnf install -y /packages/$(basename "${rpm_path}") >/dev/null
      command -v gewyvern >/dev/null
      command -v gewyc >/dev/null
      command -v gewyvern_socket_send >/dev/null
      gewyvern --list-protocols >/dev/null
      gewyc /usr/share/gewyvern/dsl/http_request_path.gewy --json >/dev/null
      test -d /usr/share/gewyvern/dsl
      test -d /usr/share/gewyvern/protocols
      test -f /usr/share/gewyvern/package-compat.toml
      grep -q '^schema_version = 1$' /usr/share/gewyvern/package-compat.toml
      grep -q '^release_line = \"${RELEASE_LINE}\"$' /usr/share/gewyvern/package-compat.toml
      test -f /usr/share/gewyvern/examples/gewyvern.toml.example
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
