#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
DEV_NAME="${1:-eth0}"

cd "${ROOT}"
cargo run --quiet --bin gewyvern_validate -- linux-tc-smoke --dev "${DEV_NAME}"
