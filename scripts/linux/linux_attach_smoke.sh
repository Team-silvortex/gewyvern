#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
HOOKPOINT_NAME="${1:-syscalls/sys_enter_nanosleep}"

cd "${ROOT}"
exec "${ROOT}/scripts/run_native_validation_bin.sh" gewyvern_native_call linux-attach-smoke "${HOOKPOINT_NAME}"
