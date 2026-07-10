#!/usr/bin/env bash

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
OUT_DIR="${ROOT}/target/packages"
TARGET_ROOT="${CARGO_TARGET_DIR:-${ROOT}/target}"
RELEASE_BIN_DIR="${GEWY_PACKAGE_BINARIES_ROOT:-${TARGET_ROOT}/release}"
WORK_DIR=""
KEEP_WORK_DIR=0
FORMAT="all"
LAYOUT_ONLY=0
MAINTAINER="${GEWY_PACKAGE_MAINTAINER:-OpenAI Codex <codex@example.invalid>}"
PACKAGE_NAME="${GEWY_PACKAGE_NAME:-gewyvern}"
PACKAGE_RELEASE="${GEWY_PACKAGE_RELEASE:-1}"
RELEASE_LINE="${GEWY_RELEASE_LINE:-v0.20.x}"
LAYOUT_VERSION="${GEWY_LAYOUT_VERSION:-1}"
CONFIG_SCHEMA_VERSION="${GEWY_CONFIG_SCHEMA_VERSION:-1}"
RPM_DIST="${GEWY_RPM_DIST:-}"

usage() {
  cat <<'EOF'
Usage: scripts/packaging/build_packages.sh [--format deb|rpm|all] [--layout-only]

Build release binaries, stage a Linux installation tree, and optionally emit
native DEB/RPM packages if the host provides `dpkg-deb` and/or `rpmbuild`.

Options:
  --format <deb|rpm|all>  Select which package format to build. Default: all
  --layout-only           Only create the staged install tree and metadata
  --out-dir <path>        Override the package output directory
  -h, --help              Show this help text
EOF
}

read_version() {
  awk -F'"' '
    $0 == "[package]" { in_package = 1; next }
    /^\[/ { if (in_package) exit }
    in_package && $1 ~ /^version = / { print $2; exit }
  ' "${ROOT}/Cargo.toml"
}

map_deb_arch() {
  case "$1" in
    x86_64) echo "amd64" ;;
    aarch64|arm64) echo "arm64" ;;
    armv7l) echo "armhf" ;;
    *)
      echo "unsupported architecture for deb packaging: $1" >&2
      exit 1
      ;;
  esac
}

map_rpm_arch() {
  case "$1" in
    x86_64) echo "x86_64" ;;
    aarch64|arm64) echo "aarch64" ;;
    armv7l) echo "armv7hl" ;;
    *)
      echo "unsupported architecture for rpm packaging: $1" >&2
      exit 1
      ;;
  esac
}

render_template() {
  local template="$1"
  local output="$2"
  python3 - "$template" "$output" <<'PY'
from pathlib import Path
import os
import sys

template = Path(sys.argv[1]).read_text()
for key, value in os.environ.items():
    if key.startswith("GEWY_TEMPLATE_"):
        template = template.replace(f"@{key.removeprefix('GEWY_TEMPLATE_')}@", value)
Path(sys.argv[2]).write_text(template)
PY
}

stage_layout() {
  local stage_root="$1"
  local version
  version="$(read_version)"

  mkdir -p \
    "${stage_root}/usr/bin" \
    "${stage_root}/usr/share/gewyvern/examples" \
    "${stage_root}/usr/share/gewyvern" \
    "${stage_root}/usr/share/doc/${PACKAGE_NAME}"

  install -m 0755 "${RELEASE_BIN_DIR}/gewyvern" \
    "${stage_root}/usr/bin/gewyvern"
  install -m 0755 "${RELEASE_BIN_DIR}/gewyvern_socket_send" \
    "${stage_root}/usr/bin/gewyvern_socket_send"
  install -m 0755 "${RELEASE_BIN_DIR}/gewyc" \
    "${stage_root}/usr/bin/gewyc"

  cp -a "${ROOT}/dsl" "${stage_root}/usr/share/gewyvern/dsl"
  cp -a "${ROOT}/protocols" "${stage_root}/usr/share/gewyvern/protocols"
  install -m 0644 "${ROOT}/docs/fixtures/gewyvern.toml.example" \
    "${stage_root}/usr/share/gewyvern/examples/gewyvern.toml.example"
  cp -a "${ROOT}/docs" "${stage_root}/usr/share/doc/${PACKAGE_NAME}/docs"
  install -m 0644 "${ROOT}/README.md" \
    "${stage_root}/usr/share/doc/${PACKAGE_NAME}/README.md"
  install -m 0644 "${ROOT}/LICENSE" \
    "${stage_root}/usr/share/doc/${PACKAGE_NAME}/LICENSE"
  cat >"${stage_root}/usr/share/gewyvern/package-compat.toml" <<EOF
schema_version = 1
package_name = "${PACKAGE_NAME}"
package_version = "${version}"
package_release = "${PACKAGE_RELEASE}"
release_line = "${RELEASE_LINE}"
layout_version = ${LAYOUT_VERSION}
config_schema_version = ${CONFIG_SCHEMA_VERSION}
share_root = "/usr/share/gewyvern"
protocol_registry_root = "/usr/share/gewyvern/protocols"
dsl_root = "/usr/share/gewyvern/dsl"
config_example = "/usr/share/gewyvern/examples/gewyvern.toml.example"
legacy_compat_root = "~/.gewyvern"
upgrade_policy = "copy-forward-without-overwrite"
EOF
}

build_release_binaries() {
  cargo build --release -p gewyvern --bin gewyvern --bin gewyvern_socket_send
  cargo build --release -p gewyc --bin gewyc
}

build_deb() {
  local version="$1"
  local deb_arch="$2"
  local stage_root="$3"
  local deb_root="${WORK_DIR}/deb"
  local control_dir="${deb_root}/DEBIAN"
  local deb_path="${OUT_DIR}/${PACKAGE_NAME}_${version}-${PACKAGE_RELEASE}_${deb_arch}.deb"

  mkdir -p "${control_dir}"
  cp -a "${stage_root}/." "${deb_root}/"

  export GEWY_TEMPLATE_PACKAGE_NAME="${PACKAGE_NAME}"
  export GEWY_TEMPLATE_VERSION="${version}-${PACKAGE_RELEASE}"
  export GEWY_TEMPLATE_DEB_ARCH="${deb_arch}"
  export GEWY_TEMPLATE_MAINTAINER="${MAINTAINER}"
  render_template \
    "${ROOT}/packaging/deb/control.in" \
    "${control_dir}/control"

  if [[ "${LAYOUT_ONLY}" -eq 1 ]]; then
    echo "deb layout prepared at ${deb_root}"
    return
  fi

  if ! command -v dpkg-deb >/dev/null 2>&1; then
    echo "dpkg-deb is required to build .deb packages" >&2
    exit 1
  fi

  dpkg-deb --root-owner-group --build "${deb_root}" "${deb_path}"
  echo "built ${deb_path}"
}

build_rpm() {
  local version="$1"
  local rpm_arch="$2"
  local stage_root="$3"
  local spec_path="${WORK_DIR}/rpmbuild/SPECS/${PACKAGE_NAME}.spec"
  local rpm_topdir="${WORK_DIR}/rpmbuild"
  local rpm_out_dir="${OUT_DIR}/rpm"

  mkdir -p \
    "${rpm_topdir}/BUILD" \
    "${rpm_topdir}/BUILDROOT" \
    "${rpm_topdir}/RPMS" \
    "${rpm_topdir}/SOURCES" \
    "${rpm_topdir}/SPECS" \
    "${rpm_topdir}/SRPMS" \
    "${rpm_out_dir}"

  export GEWY_TEMPLATE_PACKAGE_NAME="${PACKAGE_NAME}"
  export GEWY_TEMPLATE_VERSION="${version}"
  export GEWY_TEMPLATE_RELEASE="${PACKAGE_RELEASE}"
  export GEWY_TEMPLATE_DIST="${RPM_DIST}"
  export GEWY_TEMPLATE_RPM_ARCH="${rpm_arch}"
  export GEWY_TEMPLATE_STAGE_ROOT="${stage_root}"
  export GEWY_TEMPLATE_RELEASE_LINE="${RELEASE_LINE}"
  export GEWY_TEMPLATE_LAYOUT_VERSION="${LAYOUT_VERSION}"
  export GEWY_TEMPLATE_CONFIG_SCHEMA_VERSION="${CONFIG_SCHEMA_VERSION}"
  render_template \
    "${ROOT}/packaging/rpm/gewyvern.spec.in" \
    "${spec_path}"

  if [[ "${LAYOUT_ONLY}" -eq 1 ]]; then
    echo "rpm spec prepared at ${spec_path}"
    return
  fi

  if ! command -v rpmbuild >/dev/null 2>&1; then
    echo "rpmbuild is required to build .rpm packages" >&2
    exit 1
  fi

  rpmbuild --define "_topdir ${rpm_topdir}" -bb "${spec_path}"
  find "${rpm_topdir}/RPMS" -type f -name '*.rpm' -exec cp '{}' "${rpm_out_dir}/" ';'
  echo "built rpm packages in ${rpm_out_dir}"
}

build_all_formats() {
  local version="$1"
  local deb_arch="$2"
  local rpm_arch="$3"
  local stage_root="$4"
  local deb_pid=""
  local rpm_pid=""
  local status=0

  build_deb "${version}" "${deb_arch}" "${stage_root}" &
  deb_pid=$!
  build_rpm "${version}" "${rpm_arch}" "${stage_root}" &
  rpm_pid=$!

  wait "${deb_pid}" || status=$?
  wait "${rpm_pid}" || status=$?
  return "${status}"
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --format)
      FORMAT="$2"
      shift 2
      ;;
    --layout-only)
      LAYOUT_ONLY=1
      shift
      ;;
    --out-dir)
      OUT_DIR="$2"
      shift 2
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

case "${FORMAT}" in
  deb|rpm|all) ;;
  *)
    echo "invalid format: ${FORMAT}" >&2
    usage >&2
    exit 1
    ;;
esac

VERSION="$(read_version)"
HOST_ARCH="$(uname -m)"
DEB_ARCH="$(map_deb_arch "${HOST_ARCH}")"
RPM_ARCH="$(map_rpm_arch "${HOST_ARCH}")"
mkdir -p "${OUT_DIR}"

if [[ "${LAYOUT_ONLY}" -eq 1 ]]; then
  KEEP_WORK_DIR=1
  WORK_DIR="$(mktemp -d "${OUT_DIR}/layout.XXXXXX")"
else
  WORK_DIR="$(mktemp -d "${TMPDIR:-/tmp}/gewyvern-packaging.XXXXXX")"
fi

STAGE_ROOT="${WORK_DIR}/stage"

if [[ "${KEEP_WORK_DIR}" -eq 0 ]]; then
  trap 'rm -rf "${WORK_DIR}"' EXIT
fi

echo "building release binaries for packaging..."
build_release_binaries

echo "staging install tree..."
stage_layout "${STAGE_ROOT}"
chown -R 0:0 "${STAGE_ROOT}" 2>/dev/null || true

case "${FORMAT}" in
  deb)
    build_deb "${VERSION}" "${DEB_ARCH}" "${STAGE_ROOT}"
    ;;
  rpm)
    build_rpm "${VERSION}" "${RPM_ARCH}" "${STAGE_ROOT}"
    ;;
  all)
    build_all_formats "${VERSION}" "${DEB_ARCH}" "${RPM_ARCH}" "${STAGE_ROOT}"
    ;;
esac

if [[ "${LAYOUT_ONLY}" -eq 1 ]]; then
  echo "staged layout available at ${STAGE_ROOT}"
fi
