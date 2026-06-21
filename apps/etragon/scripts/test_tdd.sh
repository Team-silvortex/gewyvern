#!/usr/bin/env bash
set -euo pipefail

MODE="${1:-all}"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"

case "$MODE" in
  unit)
    cargo test --manifest-path "$ROOT/Cargo.toml" --lib
    ;;
  cli)
    cargo test --manifest-path "$ROOT/Cargo.toml" --bin etragon
    ;;
  integration)
    cargo test --manifest-path "$ROOT/Cargo.toml" --test contract_fixtures --test pipeline_integration --test cli_end_to_end
    ;;
  all)
    cargo test --manifest-path "$ROOT/Cargo.toml"
    ;;
  *)
    echo "usage: scripts/test_tdd.sh [unit|cli|integration|all]" >&2
    exit 2
    ;;
esac
