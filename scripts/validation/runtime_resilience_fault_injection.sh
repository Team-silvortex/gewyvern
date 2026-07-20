#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

usage() {
  cat <<'EOF'
usage:
  bash scripts/validation/runtime_resilience_fault_injection.sh emit-external-engine <timeout|fail|healthy> <output-path>
  bash scripts/validation/runtime_resilience_fault_injection.sh drive-socket-bad-json <host> <port> [count]
EOF
}

cmd="${1:-}"
case "${cmd}" in
  emit-external-engine)
    mode="${2:-}"
    output_path="${3:-}"
    if [[ -z "${mode}" || -z "${output_path}" ]]; then
      usage
      exit 1
    fi
    cd "${ROOT}"
    exec "${ROOT}/scripts/run_native_validation_bin.sh" gewyvern_validate -- \
      resilience-emit-helper \
      --mode "${mode}" \
      --output "${output_path}"
    ;;
  drive-socket-bad-json)
    host="${2:-}"
    port="${3:-}"
    count="${4:-5}"
    if [[ -z "${host}" || -z "${port}" ]]; then
      usage
      exit 1
    fi
    cd "${ROOT}"
    exec "${ROOT}/scripts/run_native_validation_bin.sh" gewyvern_validate -- \
      resilience-drive-bad-json \
      --host "${host}" \
      --port "${port}" \
      --count "${count}"
    ;;
  -h|--help|help|"")
    usage
    ;;
  *)
    echo "unsupported command: ${cmd}" >&2
    usage
    exit 1
    ;;
esac
