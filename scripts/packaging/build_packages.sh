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
TIMINGS_FILE=""
MANIFEST_FILE=""
CACHE_KEY_FILE=""
PENDING_MANIFEST_FILE=""
MAINTAINER="${GEWY_PACKAGE_MAINTAINER:-OpenAI Codex <codex@example.invalid>}"
PACKAGE_NAME="${GEWY_PACKAGE_NAME:-gewyvern}"
PACKAGE_RELEASE="${GEWY_PACKAGE_RELEASE:-1}"
RELEASE_LINE="${GEWY_RELEASE_LINE:-v1.15.0}"
LAYOUT_VERSION="${GEWY_LAYOUT_VERSION:-1}"
CONFIG_SCHEMA_VERSION="${GEWY_CONFIG_SCHEMA_VERSION:-1}"
RPM_DIST="${GEWY_RPM_DIST:-}"
SOURCE_DATE_EPOCH_VALUE=""

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

now_seconds() {
  python3 - <<'PY'
import time
print(f"{time.monotonic():.6f}")
PY
}

duration_seconds() {
  local start="$1"
  local end="$2"
  python3 - "$start" "$end" <<'PY'
import sys
start = float(sys.argv[1])
end = float(sys.argv[2])
print(f"{end - start:.3f}")
PY
}

record_timing() {
  local key="$1"
  local value="$2"
  printf '%s=%s\n' "$key" "$value" >>"${TIMINGS_FILE}"
}

record_manifest() {
  local key="$1"
  local value="$2"
  printf '%s=%s\n' "$key" "$value" >>"${MANIFEST_FILE}"
}

write_cache_key() {
  local value="$1"
  local pending
  pending="$(mktemp "${OUT_DIR}/.build-cache-key.XXXXXX")"
  if ! printf '%s\n' "$value" >"${pending}"; then
    rm -f "${pending}"
    return 1
  fi
  if ! chmod 0644 "${pending}"; then
    rm -f "${pending}"
    return 1
  fi
  if ! mv -f "${pending}" "${CACHE_KEY_FILE}"; then
    rm -f "${pending}"
    return 1
  fi
}

resolve_source_date_epoch() {
  if [[ -n "${SOURCE_DATE_EPOCH:-}" ]]; then
    echo "${SOURCE_DATE_EPOCH}"
    return
  fi

  if command -v git >/dev/null 2>&1 && git -C "${ROOT}" rev-parse --is-inside-work-tree >/dev/null 2>&1; then
    git -C "${ROOT}" log -1 --format=%ct
    return
  fi

  python3 - "${ROOT}/Cargo.toml" <<'PY'
from pathlib import Path
import sys

print(int(Path(sys.argv[1]).stat().st_mtime))
PY
}

configure_rust_build_acceleration() {
  if command -v ld.lld >/dev/null 2>&1; then
    export RUSTFLAGS="${RUSTFLAGS:-} -C link-arg=-fuse-ld=lld"
  fi
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
    "${stage_root}/usr/libexec" \
    "${stage_root}/usr/sbin" \
    "${stage_root}/usr/share/gewyvern/examples" \
    "${stage_root}/usr/share/gewyvern" \
    "${stage_root}/usr/share/doc/${PACKAGE_NAME}"

  install -m 0755 "${RELEASE_BIN_DIR}/gewyvern" \
    "${stage_root}/usr/bin/gewyvern"
  install -m 0755 "${RELEASE_BIN_DIR}/gewyvern_socket_send" \
    "${stage_root}/usr/bin/gewyvern_socket_send"
  install -m 0755 "${RELEASE_BIN_DIR}/gewyc" \
    "${stage_root}/usr/bin/gewyc"
  install -m 0755 "${RELEASE_BIN_DIR}/gewyvern_ebpf_helper" \
    "${stage_root}/usr/libexec/gewyvern-ebpf-helper"
  install -m 0755 "${RELEASE_BIN_DIR}/gewyvern_ebpf_provision" \
    "${stage_root}/usr/sbin/gewyvern-ebpf-provision"

  cp -a "${ROOT}/dsl" "${stage_root}/usr/share/gewyvern/dsl"
  cp -a "${ROOT}/protocols" "${stage_root}/usr/share/gewyvern/protocols"
  install -m 0644 "${ROOT}/docs/fixtures/gewyvern.toml.example" \
    "${stage_root}/usr/share/gewyvern/examples/gewyvern.toml.example"
  install -m 0644 "${ROOT}/packaging/ebpf-helper.conf.example" \
    "${stage_root}/usr/share/gewyvern/examples/ebpf-helper.conf.example"
  install -m 0644 "${ROOT}/packaging/gewyvern-ebpf-validation.sudoers.example" \
    "${stage_root}/usr/share/gewyvern/examples/gewyvern-ebpf-validation.sudoers.example"
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

normalize_stage_timestamps() {
  local stage_root="$1"
  local epoch="$2"

  python3 - "${stage_root}" "${epoch}" <<'PY'
from pathlib import Path
import os
import sys

root = Path(sys.argv[1])
epoch = int(sys.argv[2])

for path in sorted(root.rglob("*")):
    os.utime(path, (epoch, epoch), follow_symlinks=False)
os.utime(root, (epoch, epoch), follow_symlinks=False)
PY
}

compute_package_cache_key() {
  python3 - \
    "${ROOT}" \
    "${RELEASE_BIN_DIR}" \
    "${PACKAGE_NAME}" \
    "${PACKAGE_RELEASE}" \
    "${RELEASE_LINE}" \
    "${LAYOUT_VERSION}" \
    "${CONFIG_SCHEMA_VERSION}" \
    "${MAINTAINER}" \
    "${RPM_DIST}" <<'PY'
from pathlib import Path
import hashlib
import os
import sys

root = Path(sys.argv[1])
release_bin_dir = Path(sys.argv[2])
config_values = sys.argv[3:]

hash_obj = hashlib.sha256()

for value in config_values:
    hash_obj.update(value.encode("utf-8"))
    hash_obj.update(b"\0")

files = [
    release_bin_dir / "gewyvern",
    release_bin_dir / "gewyvern_socket_send",
    release_bin_dir / "gewyc",
    root / "Cargo.toml",
    root / "README.md",
    root / "LICENSE",
    root / "docs/fixtures/gewyvern.toml.example",
    root / "packaging/deb/control.in",
    root / "packaging/rpm/gewyvern.spec.in",
]

directories = [
    root / "dsl",
    root / "protocols",
    root / "docs",
]

def update_file(path: Path) -> None:
    relative = path.relative_to(root) if path.is_relative_to(root) else path
    stat = path.stat()
    hash_obj.update(str(relative).encode("utf-8"))
    hash_obj.update(b"\0")
    hash_obj.update(oct(stat.st_mode).encode("utf-8"))
    hash_obj.update(b"\0")
    with path.open("rb") as handle:
        while True:
            chunk = handle.read(1024 * 1024)
            if not chunk:
                break
            hash_obj.update(chunk)

for file_path in files:
    update_file(file_path)

for directory in directories:
    for file_path in sorted(path for path in directory.rglob("*") if path.is_file()):
        update_file(file_path)

print(hash_obj.hexdigest())
PY
}

can_reuse_cached_packages() {
  local cache_key="$1"
  local manifest_deb="$2"
  local manifest_rpm="$3"

  [[ -f "${CACHE_KEY_FILE}" && ! -L "${CACHE_KEY_FILE}" ]] || return 1
  [[ -f "${MANIFEST_FILE}" && ! -L "${MANIFEST_FILE}" ]] || return 1

  local current_key
  current_key="$(cat "${CACHE_KEY_FILE}")"
  [[ "${current_key}" == "${cache_key}" ]] || return 1

  case "${FORMAT}" in
    deb)
      package_cache_artifact_valid "${manifest_deb}" deb || return 1
      ;;
    rpm)
      package_cache_artifact_valid "${manifest_rpm}" rpm || return 1
      ;;
    all)
      package_cache_artifact_valid "${manifest_deb}" deb || return 1
      package_cache_artifact_valid "${manifest_rpm}" rpm || return 1
      ;;
  esac
}

package_cache_artifact_valid() {
  local artifact="$1"
  local extension="$2"
  local candidate package_root resolved

  [[ -n "${artifact}" && "${artifact}" != /* ]] || return 1
  candidate="${OUT_DIR}/${artifact}"
  [[ -f "${candidate}" && ! -L "${candidate}" ]] || return 1
  package_root="$(realpath "${OUT_DIR}")" || return 1
  resolved="$(realpath "${candidate}")" || return 1
  case "${resolved}" in
    "${package_root}"/*) ;;
    *) return 1 ;;
  esac
  [[ "${resolved}" == *."${extension}" ]] || return 1
}

read_manifest_value() {
  local key="$1"
  [[ -f "${MANIFEST_FILE}" && ! -L "${MANIFEST_FILE}" ]] || return 1
  awk -F= -v wanted="${key}" '
    index($0, "=") == 0 || $1 == "" || substr($0, length($1) + 2) == "" || ($1 != "deb" && $1 != "rpm") {
      malformed = 1
      next
    }
    $1 == wanted {
      count++
      value = substr($0, length($1) + 2)
    }
    END {
      if (malformed || count != 1) exit 1
      print value
    }
  ' "${MANIFEST_FILE}"
}

build_release_binaries() {
  cargo build --release \
    -p gewyvern \
    -p gewyc \
    --bin gewyvern \
    --bin gewyvern_socket_send \
    --bin gewyvern_validate \
    --bin gewyvern_ebpf_helper \
    --bin gewyvern_ebpf_provision \
    --bin gewyc
}

build_deb() {
  local version="$1"
  local deb_arch="$2"
  local stage_root="$3"
  local deb_root="${WORK_DIR}/deb"
  local control_dir="${deb_root}/DEBIAN"
  local deb_path="${OUT_DIR}/${PACKAGE_NAME}_${version}-${PACKAGE_RELEASE}_${deb_arch}.deb"
  local built_deb="${WORK_DIR}/${PACKAGE_NAME}_${version}-${PACKAGE_RELEASE}_${deb_arch}.deb"

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

  dpkg-deb --root-owner-group --build "${deb_root}" "${built_deb}"
  local pending_deb
  pending_deb="$(mktemp "${OUT_DIR}/.package-deb.XXXXXX")"
  if ! install -m 0644 "${built_deb}" "${pending_deb}"; then
    rm -f "${pending_deb}"
    return 1
  fi
  mv -f "${pending_deb}" "${deb_path}"
  record_manifest "deb" "${deb_path#"${OUT_DIR}/"}"
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

  rpmbuild \
    --define "_topdir ${rpm_topdir}" \
    --define "use_source_date_epoch_as_buildtime 1" \
    --define "clamp_mtime_to_source_date_epoch 1" \
    -bb "${spec_path}"
  local rpm_candidates=()
  mapfile -t rpm_candidates < <(find "${rpm_topdir}/RPMS" -type f -name '*.rpm' -print | sort)
  if [[ "${#rpm_candidates[@]}" -ne 1 ]]; then
    echo "expected exactly one RPM from this build, found ${#rpm_candidates[@]}" >&2
    return 1
  fi
  local rpm_path="${rpm_out_dir}/$(basename "${rpm_candidates[0]}")"
  local pending_rpm
  pending_rpm="$(mktemp "${rpm_out_dir}/.package-rpm.XXXXXX")"
  if ! install -m 0644 "${rpm_candidates[0]}" "${pending_rpm}"; then
    rm -f "${pending_rpm}"
    return 1
  fi
  mv -f "${pending_rpm}" "${rpm_path}"
  record_manifest "rpm" "${rpm_path#"${OUT_DIR}/"}"
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
SOURCE_DATE_EPOCH_VALUE="$(resolve_source_date_epoch)"
export SOURCE_DATE_EPOCH="${SOURCE_DATE_EPOCH_VALUE}"
mkdir -p "${OUT_DIR}"
TIMINGS_FILE="${OUT_DIR}/build-timings.txt"
MANIFEST_FILE="${OUT_DIR}/build-manifest.txt"
CACHE_KEY_FILE="${OUT_DIR}/build-cache-key.txt"
PACKAGE_LOCK_FILE="${OUT_DIR}/.build-packages.lock"
PACKAGE_LOCK_TIMEOUT_SECONDS="${GEWY_PACKAGE_LOCK_TIMEOUT_SECONDS:-120}"
if [[ ! "${PACKAGE_LOCK_TIMEOUT_SECONDS}" =~ ^[0-9]+$ ]] ||
  ((PACKAGE_LOCK_TIMEOUT_SECONDS < 1 || PACKAGE_LOCK_TIMEOUT_SECONDS > 3600)); then
  echo "GEWY_PACKAGE_LOCK_TIMEOUT_SECONDS must be an integer from 1 through 3600" >&2
  exit 2
fi
if [[ -L "${PACKAGE_LOCK_FILE}" ]]; then
  echo "package build lock must not be a symlink: ${PACKAGE_LOCK_FILE}" >&2
  exit 2
fi
exec 9>"${PACKAGE_LOCK_FILE}"
if ! flock -w "${PACKAGE_LOCK_TIMEOUT_SECONDS}" 9; then
  echo "timed out waiting for package build lock: ${PACKAGE_LOCK_FILE}" >&2
  exit 3
fi
rm -f "${TIMINGS_FILE}"
TOTAL_STARTED="$(now_seconds)"

if [[ "${LAYOUT_ONLY}" -eq 1 ]]; then
  KEEP_WORK_DIR=1
  WORK_DIR="$(mktemp -d "${OUT_DIR}/layout.XXXXXX")"
else
  WORK_DIR="$(mktemp -d "${TMPDIR:-/tmp}/gewyvern-packaging.XXXXXX")"
fi

STAGE_ROOT="${WORK_DIR}/stage"

if [[ "${KEEP_WORK_DIR}" -eq 0 ]]; then
  trap 'rm -rf "${WORK_DIR}"; if [[ -n "${PENDING_MANIFEST_FILE}" ]]; then rm -f "${PENDING_MANIFEST_FILE}"; fi' EXIT
fi

echo "building release binaries for packaging..."
configure_rust_build_acceleration
RELEASE_BUILD_STARTED="$(now_seconds)"
build_release_binaries
RELEASE_BUILD_FINISHED="$(now_seconds)"
record_timing "release_build" "$(duration_seconds "${RELEASE_BUILD_STARTED}" "${RELEASE_BUILD_FINISHED}")"

PACKAGE_CACHE_KEY="$(compute_package_cache_key)"
if [[ "${LAYOUT_ONLY}" -eq 0 ]]; then
  MANIFEST_DEB="$(read_manifest_value deb || true)"
  MANIFEST_RPM="$(read_manifest_value rpm || true)"
  if can_reuse_cached_packages "${PACKAGE_CACHE_KEY}" "${MANIFEST_DEB}" "${MANIFEST_RPM}"; then
    echo "reusing cached package artifacts..."
    chmod 0644 "${CACHE_KEY_FILE}" "${MANIFEST_FILE}"
    record_timing "stage_layout" "0.000"
    case "${FORMAT}" in
      deb)
        record_timing "package_deb" "0.000"
        ;;
      rpm)
        record_timing "package_rpm" "0.000"
        ;;
      all)
        record_timing "package_all" "0.000"
        ;;
    esac
    TOTAL_FINISHED="$(now_seconds)"
    record_timing "total" "$(duration_seconds "${TOTAL_STARTED}" "${TOTAL_FINISHED}")"
    exit 0
  fi
  PENDING_MANIFEST_FILE="$(mktemp "${OUT_DIR}/.build-manifest.XXXXXX")"
  MANIFEST_FILE="${PENDING_MANIFEST_FILE}"
fi

echo "staging install tree..."
STAGE_LAYOUT_STARTED="$(now_seconds)"
stage_layout "${STAGE_ROOT}"
normalize_stage_timestamps "${STAGE_ROOT}" "${SOURCE_DATE_EPOCH_VALUE}"
chown -R 0:0 "${STAGE_ROOT}" 2>/dev/null || true
STAGE_LAYOUT_FINISHED="$(now_seconds)"
record_timing "stage_layout" "$(duration_seconds "${STAGE_LAYOUT_STARTED}" "${STAGE_LAYOUT_FINISHED}")"

case "${FORMAT}" in
  deb)
    DEB_BUILD_STARTED="$(now_seconds)"
    build_deb "${VERSION}" "${DEB_ARCH}" "${STAGE_ROOT}"
    DEB_BUILD_FINISHED="$(now_seconds)"
    record_timing "package_deb" "$(duration_seconds "${DEB_BUILD_STARTED}" "${DEB_BUILD_FINISHED}")"
    ;;
  rpm)
    RPM_BUILD_STARTED="$(now_seconds)"
    build_rpm "${VERSION}" "${RPM_ARCH}" "${STAGE_ROOT}"
    RPM_BUILD_FINISHED="$(now_seconds)"
    record_timing "package_rpm" "$(duration_seconds "${RPM_BUILD_STARTED}" "${RPM_BUILD_FINISHED}")"
    ;;
  all)
    PACKAGE_ALL_STARTED="$(now_seconds)"
    build_all_formats "${VERSION}" "${DEB_ARCH}" "${RPM_ARCH}" "${STAGE_ROOT}"
    PACKAGE_ALL_FINISHED="$(now_seconds)"
    record_timing "package_all" "$(duration_seconds "${PACKAGE_ALL_STARTED}" "${PACKAGE_ALL_FINISHED}")"
    ;;
esac

TOTAL_FINISHED="$(now_seconds)"
record_timing "total" "$(duration_seconds "${TOTAL_STARTED}" "${TOTAL_FINISHED}")"

if [[ "${LAYOUT_ONLY}" -eq 1 ]]; then
  echo "staged layout available at ${STAGE_ROOT}"
else
  case "${FORMAT}" in
    deb)
      PENDING_DEB="$(read_manifest_value deb)"
      package_cache_artifact_valid "${PENDING_DEB}" deb
      ;;
    rpm)
      PENDING_RPM="$(read_manifest_value rpm)"
      package_cache_artifact_valid "${PENDING_RPM}" rpm
      ;;
    all)
      PENDING_DEB="$(read_manifest_value deb)"
      PENDING_RPM="$(read_manifest_value rpm)"
      package_cache_artifact_valid "${PENDING_DEB}" deb
      package_cache_artifact_valid "${PENDING_RPM}" rpm
      ;;
  esac
  chmod 0644 "${PENDING_MANIFEST_FILE}"
  mv -f "${PENDING_MANIFEST_FILE}" "${OUT_DIR}/build-manifest.txt"
  PENDING_MANIFEST_FILE=""
  MANIFEST_FILE="${OUT_DIR}/build-manifest.txt"
  write_cache_key "${PACKAGE_CACHE_KEY}"
fi
