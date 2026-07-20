#!/usr/bin/env bash

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

exec "${ROOT}/scripts/run_native_validation_bin.sh" gewyvern_validate -- package-install-smoke "$@"
