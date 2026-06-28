#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
usage:
  bash scripts/validation/runtime_resilience_log_evidence.sh <log-file-or-dir> [output-dir]

Given one runtime log file or a directory containing runtime logs, extract the
current resilience evidence into:

- resilience-events.log
- resilience-summary.txt

The summary counts these event names when present:

- external_analysis_failed
- external_analysis_circuit_open
- external_analysis_recovered
- socket_session_collect_failed
- socket_session_run_failed
- socket_service_recovered
EOF
}

EVENTS=(
  "external_analysis_failed"
  "external_analysis_circuit_open"
  "external_analysis_recovered"
  "socket_session_collect_failed"
  "socket_session_run_failed"
  "socket_service_recovered"
)

resolve_input_files() {
  local input_path="$1"
  if [[ -f "${input_path}" ]]; then
    printf '%s\n' "${input_path}"
    return 0
  fi
  if [[ -d "${input_path}" ]]; then
    find "${input_path}" -maxdepth 1 -type f | sort
    return 0
  fi
  echo "input path does not exist: ${input_path}" >&2
  exit 1
}

extract_events() {
  local output_log="$1"
  shift
  : >"${output_log}"
  local file
  for file in "$@"; do
    rg --no-filename "event=($(printf '%s|' "${EVENTS[@]}" | sed 's/|$//'))|backoff_ms=" "${file}" >>"${output_log}" || true
  done
}

write_summary() {
  local event_log="$1"
  local output_summary="$2"
  {
    echo "runtime resilience log evidence summary"
    echo "====================================="
    echo
    echo "source events:"
    local event
    for event in "${EVENTS[@]}"; do
      local count
      count="$(awk -v event="event=${event}" 'index($0, event) { count++ } END { print count + 0 }' "${event_log}")"
      printf -- "- %s: %s\n" "${event}" "${count}"
    done
    local backoff_count
    backoff_count="$(awk 'index($0, "backoff_ms=") { count++ } END { print count + 0 }' "${event_log}")"
    printf -- "- backoff_ms fields: %s\n" "${backoff_count}"
  } >"${output_summary}"
}

main() {
  local input_path="${1:-}"
  local output_dir="${2:-}"
  if [[ -z "${input_path}" ]]; then
    usage
    exit 1
  fi
  if [[ "${input_path}" == "--help" || "${input_path}" == "-h" ]]; then
    usage
    exit 0
  fi

  if [[ -z "${output_dir}" ]]; then
    output_dir="${TMPDIR:-/tmp}/gewyvern-resilience-log-evidence"
  fi
  mkdir -p "${output_dir}"

  files=()
  while IFS= read -r file; do
    files+=("${file}")
  done < <(resolve_input_files "${input_path}")
  if [[ "${#files[@]}" -eq 0 ]]; then
    echo "no log files found under: ${input_path}" >&2
    exit 1
  fi

  local output_log="${output_dir}/resilience-events.log"
  local output_summary="${output_dir}/resilience-summary.txt"
  extract_events "${output_log}" "${files[@]}"
  write_summary "${output_log}" "${output_summary}"

  echo "wrote:"
  echo "- ${output_log}"
  echo "- ${output_summary}"
  echo
  cat "${output_summary}"
}

main "$@"
