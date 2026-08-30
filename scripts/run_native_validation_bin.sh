#!/usr/bin/env bash
set -euo pipefail

if [ $# -lt 1 ]; then
  echo "usage: run_native_validation_bin.sh <binary-name> [--] [args...]" >&2
  exit 2
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BIN_NAME="$1"
shift
if [[ "${1:-}" == "--" ]]; then
  shift
fi

if [[ -n "${GEWYVERN_NATIVE_BIN_DIR:-}" ]]; then
  TARGET_DIR="${GEWYVERN_NATIVE_BIN_DIR}"
elif [[ -n "${CARGO_TARGET_DIR:-}" ]]; then
  if [[ "${CARGO_TARGET_DIR}" == /* ]]; then
    TARGET_DIR="${CARGO_TARGET_DIR}"
  else
    TARGET_DIR="${SCRIPT_DIR}/../${CARGO_TARGET_DIR}"
  fi
else
  TARGET_DIR="${SCRIPT_DIR}/../target"
fi

mkdir -p "${TARGET_DIR}/debug" "${TARGET_DIR}/release"

if [[ -z "${GEWYVERN_NATIVE_BIN_DIR:-}" ]]; then
  # Always refresh the managed debug binary so cached runners cannot execute stale source.
  (cd "${SCRIPT_DIR}/.." && cargo build --quiet --bin "${BIN_NAME}")
fi

NATIVE_BIN="${TARGET_DIR}/debug/${BIN_NAME}"
if [[ ! -x "${NATIVE_BIN}" ]]; then
  NATIVE_BIN="${TARGET_DIR}/release/${BIN_NAME}"
fi

if [[ ! -x "${NATIVE_BIN}" ]]; then
  echo "missing native binary: ${NATIVE_BIN}" >&2
  exit 1
fi

exec "${NATIVE_BIN}" "$@"
