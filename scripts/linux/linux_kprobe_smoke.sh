#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SYMBOL_NAME="${1:-ip_route_output_flow}"

cd "${ROOT}"
exec "${ROOT}/scripts/run_native_validation_bin.sh" gewyvern_native_call linux-kprobe-smoke "${SYMBOL_NAME}"
