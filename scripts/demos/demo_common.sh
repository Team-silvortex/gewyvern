#!/usr/bin/env bash
set -euo pipefail

demo_require_cmd() {
  local cmd="$1"
  if ! command -v "${cmd}" >/dev/null 2>&1; then
    echo "missing required command: ${cmd}" >&2
    exit 1
  fi
}

demo_wait_for_http_ready() {
  local url="$1"
  local attempts="${2:-120}"
  local delay="${3:-0.05}"
  for _ in $(seq 1 "${attempts}"); do
    if curl -fsS "${url}" >/dev/null 2>&1; then
      return 0
    fi
    sleep "${delay}"
  done
  return 1
}

demo_wait_for_http_body() {
  local url="$1"
  local out="$2"
  local attempts="${3:-120}"
  local delay="${4:-0.1}"
  for _ in $(seq 1 "${attempts}"); do
    if curl -fsS "${url}" >"${out}" 2>/dev/null; then
      return 0
    fi
    sleep "${delay}"
  done
  return 1
}

demo_wait_for_http_fragment() {
  local url="$1"
  local fragment="$2"
  local attempts="${3:-240}"
  local delay="${4:-0.05}"
  for _ in $(seq 1 "${attempts}"); do
    local body
    if body="$(curl -fsS "${url}" 2>/dev/null)"; then
      if [[ "${body}" == *"${fragment}"* ]]; then
        printf '%s' "${body}"
        return 0
      fi
    fi
    sleep "${delay}"
  done
  return 1
}
