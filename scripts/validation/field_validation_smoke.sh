#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
OUT_DIR="${ROOT}/target/validation/field-validation-smoke"
ARGS=(field-smoke --out-dir "${OUT_DIR}")

if [ "${GEWY_FIELD_VALIDATE_SOCKET:-0}" = "1" ]; then
  ARGS+=(--socket)
fi

if [ "${GEWY_FIELD_VALIDATE_SCAN_ALL:-0}" = "1" ]; then
  ARGS+=(--scan-all)
fi

cd "${ROOT}"
cargo run --quiet --bin gewyvern_validate -- "${ARGS[@]}"
