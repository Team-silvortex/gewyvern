#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
OUT_DIR="${1:-${ROOT}/target/validation/high-frequency-validation}"

cd "${ROOT}"
cargo run --quiet --bin gewyvern_validate -- high-frequency --out-dir "${OUT_DIR}"
