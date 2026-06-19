#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
usage:
  bash scripts/validation/runtime_resilience_fault_injection.sh emit-external-engine <timeout|fail|healthy> <output-path>
  bash scripts/validation/runtime_resilience_fault_injection.sh drive-socket-bad-json <host> <port> [count]

commands:
  emit-external-engine
    write a tiny helper script that can simulate:
      timeout : sleeps longer than the default gewyvern external-engine timeout
      fail    : exits non-zero with stderr output
      healthy : emits one minimal valid augmentation payload

  drive-socket-bad-json
    open repeated tcp connections and send intentionally invalid fact lines.
    requires 'nc' on the current host.
EOF
}

require_nc() {
  if ! command -v nc >/dev/null 2>&1; then
    echo "missing required command: nc" >&2
    exit 1
  fi
}

emit_external_engine() {
  local mode="${1:-}"
  local output_path="${2:-}"
  if [[ -z "${mode}" || -z "${output_path}" ]]; then
    usage
    exit 1
  fi

  mkdir -p "$(dirname "${output_path}")"
  case "${mode}" in
    timeout)
      cat >"${output_path}" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
sleep 6
printf 'late\n'
EOF
      ;;
    fail)
      cat >"${output_path}" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
printf 'simulated external engine failure\n' >&2
exit 1
EOF
      ;;
    healthy)
      cat >"${output_path}" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
cat >/dev/null
printf '%s\n' '[{"kind":"external-engine","name":"healthy_probe","summary":"simulated healthy external engine response","confidence":"advisory","producer_stage":"external","producer_pass":"fault-injection-helper","data_json":"{\"mode\":\"healthy\"}"}]'
EOF
      ;;
    *)
      echo "unsupported external engine mode: ${mode}" >&2
      usage
      exit 1
      ;;
  esac

  chmod +x "${output_path}"
  echo "wrote ${mode} helper to ${output_path}"
}

drive_socket_bad_json() {
  local host="${1:-}"
  local port="${2:-}"
  local count="${3:-5}"
  if [[ -z "${host}" || -z "${port}" ]]; then
    usage
    exit 1
  fi
  require_nc

  local i
  for i in $(seq 1 "${count}"); do
    printf '{"bad":"json"\n' | nc "${host}" "${port}" >/dev/null 2>&1 || true
  done
  echo "sent ${count} invalid socket payload(s) to ${host}:${port}"
}

main() {
  local cmd="${1:-}"
  case "${cmd}" in
    emit-external-engine)
      shift
      emit_external_engine "$@"
      ;;
    drive-socket-bad-json)
      shift
      drive_socket_bad_json "$@"
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
}

main "$@"
