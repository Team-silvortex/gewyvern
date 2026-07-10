#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SYMBOL_NAME="${1:-ip_route_output_flow}"

cd "${ROOT}"
cargo run --quiet --bin gewyvern_validate -- linux-kprobe-smoke --symbol "${SYMBOL_NAME}"
