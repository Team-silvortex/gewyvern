#!/usr/bin/env bash
set -euo pipefail

container_validation_require_docker() {
  local validation_name="$1"
  if ! command -v docker >/dev/null 2>&1; then
    echo "docker is required for ${validation_name}" >&2
    exit 1
  fi

  if ! docker info >/dev/null 2>&1; then
    echo "docker daemon is not reachable; start Docker Desktop or another local daemon and retry" >&2
    exit 1
  fi
}

container_validation_parse_mode_args() {
  local usage_fn="$1"
  shift

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
        "${usage_fn}"
        exit 0
        ;;
      *)
        echo "unknown argument: $1" >&2
        "${usage_fn}" >&2
        exit 1
        ;;
    esac
  done
}

container_validation_find_latest_deb() {
  local packages_dir="$1"
  find "${packages_dir}" -maxdepth 1 -type f -name '*.deb' | sort | tail -n 1
}

container_validation_find_latest_rpm() {
  local packages_dir="$1"
  find "${packages_dir}/rpm" -maxdepth 1 -type f -name '*.rpm' | sort | tail -n 1
}

container_validation_deb_preamble() {
  local mirror="${GEWY_DEB_APT_MIRROR:-}"
  cat <<EOF
if [ -n "${mirror}" ]; then
  sed -i "s|http://archive.ubuntu.com/ubuntu|${mirror}|g; s|http://security.ubuntu.com/ubuntu|${mirror}|g" /etc/apt/sources.list /etc/apt/sources.list.d/*.sources 2>/dev/null || true
fi
EOF
}

container_validation_rpm_preamble() {
  local mirror="${GEWY_RPM_DNF_MIRROR:-}"
  cat <<EOF
if [ -n "${mirror}" ]; then
  sed -i "s|^metalink=|#metalink=|g; s|^mirrorlist=|#mirrorlist=|g; s|^#baseurl=http://download.example/pub/fedora/linux|baseurl=${mirror}|g; s|^#baseurl=https://download.example/pub/fedora/linux|baseurl=${mirror}|g" /etc/yum.repos.d/*.repo 2>/dev/null || true
fi
EOF
}

container_validation_run_deb() {
  local packages_dir="$1"
  local image="$2"
  local package_path="$3"
  local body="$4"
  docker run --rm \
    -v "${packages_dir}:/packages:ro" \
    "${image}" \
    bash -lc "
      set -euo pipefail
      $(container_validation_deb_preamble)
      apt-get update >/dev/null
      apt-get install -y /packages/$(basename "${package_path}") >/dev/null
      ${body}
    "
}

container_validation_run_deb_with_curl() {
  local packages_dir="$1"
  local image="$2"
  local package_path="$3"
  local body="$4"
  docker run --rm \
    -v "${packages_dir}:/packages:ro" \
    "${image}" \
    bash -lc "
      set -euo pipefail
      $(container_validation_deb_preamble)
      apt-get update >/dev/null
      apt-get install -y curl /packages/$(basename "${package_path}") >/dev/null
      ${body}
    "
}

container_validation_run_rpm() {
  local packages_dir="$1"
  local image="$2"
  local package_path="$3"
  local body="$4"
  docker run --rm \
    -v "${packages_dir}/rpm:/packages:ro" \
    "${image}" \
    bash -lc "
      set -euo pipefail
      $(container_validation_rpm_preamble)
      dnf install -y /packages/$(basename "${package_path}") >/dev/null
      ${body}
    "
}

container_validation_run_rpm_with_curl() {
  local packages_dir="$1"
  local image="$2"
  local package_path="$3"
  local body="$4"
  docker run --rm \
    -v "${packages_dir}/rpm:/packages:ro" \
    "${image}" \
    bash -lc "
      set -euo pipefail
      $(container_validation_rpm_preamble)
      dnf install -y curl /packages/$(basename "${package_path}") >/dev/null
      ${body}
    "
}
