#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
OUT_DIR="${ROOT}/target/validation/runtime-operator"
ARGS=(runtime-operator --out-dir "${OUT_DIR}")

while [ $# -gt 0 ]; do
  case "$1" in
    --json-out)
      if [ $# -lt 2 ]; then
        echo "missing value for --json-out" >&2
        exit 1
      fi
      ARGS+=(--json-out "$2")
      shift 2
      ;;
    *)
      echo "unknown argument: $1" >&2
      echo "usage: $(basename "$0") [--json-out target/validation/runtime-operator-summary.json]" >&2
      exit 1
      ;;
  esac
done

cd "${ROOT}"
exec "${ROOT}/scripts/run_native_validation_bin.sh" gewyvern_validate -- "${ARGS[@]}"
