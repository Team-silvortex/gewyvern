#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
OUT_DIR="${ROOT}/target/validation/runtime-lifecycle"

if [ "${1:-}" = "--out-dir" ]; then
  OUT_DIR="${2:?missing value for --out-dir}"
fi

cd "${ROOT}"
cargo run --quiet --bin gewyvern_validate -- runtime-lifecycle --out-dir "${OUT_DIR}"
