#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
OUT_DIR="${1:-${ROOT}/target/validation/debugger-cross-validation}"

cd "${ROOT}"
cargo run --quiet --bin gewyvern_validate -- debugger-cross --out-dir "${OUT_DIR}"
