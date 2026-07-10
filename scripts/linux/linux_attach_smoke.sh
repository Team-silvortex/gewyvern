#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
HOOKPOINT_NAME="${1:-syscalls/sys_enter_nanosleep}"

cd "${ROOT}"
cargo run --quiet --bin gewyvern_validate -- linux-attach-smoke --hookpoint "${HOOKPOINT_NAME}"
