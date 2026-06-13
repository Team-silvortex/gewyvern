#!/usr/bin/env bash

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
RUN_DEB=1
RUN_RPM=1

usage() {
  cat <<'EOF'
Usage: scripts/release_container_check.sh [--deb] [--rpm]

Run the current release-oriented packaged Linux validation suite:

- package_install_smoke.sh
- container_runtime_validation.sh
- container_validation_summary.sh

By default, both the DEB and RPM paths run.
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --deb)
      RUN_DEB=1
      RUN_RPM=0
      shift
      ;;
    --rpm)
      RUN_DEB=0
      RUN_RPM=1
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "unknown argument: $1" >&2
      usage >&2
      exit 1
      ;;
  esac
done

mode_args=()
mode_label="deb+rpm"
if [[ "${RUN_DEB}" -eq 1 && "${RUN_RPM}" -eq 0 ]]; then
  mode_args=(--deb)
  mode_label="deb"
elif [[ "${RUN_DEB}" -eq 0 && "${RUN_RPM}" -eq 1 ]]; then
  mode_args=(--rpm)
  mode_label="rpm"
fi

run_mode_script() {
  local script_path="$1"
  if [[ "${#mode_args[@]}" -eq 0 ]]; then
    bash "${script_path}"
  else
    bash "${script_path}" "${mode_args[@]}"
  fi
}

echo "[release-check] starting packaged release validation (${mode_label})"

echo "[release-check] ----------------------------------------"
echo "[release-check] running package install smoke"
run_mode_script "${ROOT}/scripts/package_install_smoke.sh"

echo "[release-check] ----------------------------------------"
echo "[release-check] running packaged runtime validation"
run_mode_script "${ROOT}/scripts/container_runtime_validation.sh"

echo "[release-check] ----------------------------------------"
echo "[release-check] running packaged protocol/operator summary"
run_mode_script "${ROOT}/scripts/container_validation_summary.sh"

echo "[release-check] ----------------------------------------"
echo "[release-check] packaged release validation: ok (${mode_label})"
