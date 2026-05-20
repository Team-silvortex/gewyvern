#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
export ENGINE_ROOT="${ETRAGON_ROOT:-}"
export EXTERNAL_ENGINE_CMD="${EXTERNAL_ENGINE_CMD:-cargo run -- analyze-url}"
exec "${SCRIPT_DIR}/external_engine_roundtrip_demo.sh" "$@"
