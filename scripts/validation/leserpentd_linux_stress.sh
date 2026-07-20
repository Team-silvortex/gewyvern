#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
source "${ROOT}/scripts/remote/container_execution.sh"
gewy_container_maybe_run_remote "$@"

OUT_DIR="${1:-${ROOT}/target/validation/leserpentd-linux-stress}"
if [[ -n "${GEWY_NATIVE_BIN_DIR:-}" ]]; then
  NATIVE_BIN_DIR="${GEWY_NATIVE_BIN_DIR}"
elif [[ -n "${CARGO_TARGET_DIR:-}" ]]; then
  NATIVE_BIN_DIR="${CARGO_TARGET_DIR}"
else
  NATIVE_BIN_DIR="${ROOT}/target"
fi
NATIVE_BIN="${NATIVE_BIN_DIR}/debug/examples/linux_stress"
if [[ ! -x "${NATIVE_BIN}" ]]; then
  (cd "${ROOT}" && CARGO_TARGET_DIR="${NATIVE_BIN_DIR}" cargo build --quiet --example linux_stress -p leserpentd)
fi

if [[ ! -x "${NATIVE_BIN}" ]]; then
  echo "missing native binary: ${NATIVE_BIN}" >&2
  exit 1
fi

case "${OUT_DIR}" in
  "${ROOT}"/target/validation/*) ;;
  *)
    echo "output directory must stay under target/validation" >&2
    exit 2
    ;;
esac

mkdir -p "${OUT_DIR}"
STAMP="$(date -u +%Y%m%dT%H%M%SZ)"
TEMP="${OUT_DIR}/.${STAMP}.$$.tmp"
RUN="${OUT_DIR}/${STAMP}.json"
trap 'rm -f "${TEMP}"' EXIT

"${NATIVE_BIN}" >"${TEMP}"
mv "${TEMP}" "${RUN}"
cp "${RUN}" "${OUT_DIR}/latest.json"
printf 'leserpentd Linux stress evidence: %s\n' "${RUN}"
