#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
source "${ROOT}/scripts/remote/container_execution.sh"
gewy_container_maybe_run_remote "$@"

cd "${ROOT}"
exec cargo run --quiet --bin gewyvern_validate -- ftp-denied-container-validation "$@"
