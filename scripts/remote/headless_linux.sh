#!/usr/bin/env bash

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
ACTION="${1:-status}"

case "${ACTION}" in
  build)
    exec "${ROOT}/scripts/remote/run_on_linux_host.sh" --no-sync-back -- \
      docker compose -f docker-compose.headless-linux.yml build
    ;;
  up)
    exec "${ROOT}/scripts/remote/run_on_linux_host.sh" --no-sync-back -- \
      docker compose -f docker-compose.headless-linux.yml up -d
    ;;
  down)
    exec "${ROOT}/scripts/remote/run_on_linux_host.sh" --no-sync-back -- \
      docker compose -f docker-compose.headless-linux.yml down
    ;;
  status)
    exec "${ROOT}/scripts/remote/run_on_linux_host.sh" --no-sync-back -- \
      docker compose -f docker-compose.headless-linux.yml ps
    ;;
  shell)
    exec "${ROOT}/scripts/remote/run_on_linux_host.sh" --no-sync-back --tty -- \
      docker compose -f docker-compose.headless-linux.yml exec ebpf-dev bash
    ;;
  *)
    echo "Usage: scripts/remote/headless_linux.sh build|up|down|status|shell" >&2
    exit 2
    ;;
esac
