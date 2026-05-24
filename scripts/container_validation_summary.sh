#!/usr/bin/env bash

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
RUN_DEB=1
RUN_RPM=1

usage() {
  cat <<'EOF'
Usage: scripts/container_validation_summary.sh [--deb] [--rpm]

Run the packaged Linux container validation suite as one summarized entrypoint:

- container_protocol_validation.sh
- container_operator_path_validation.sh

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

echo "[summary] starting packaged container validation (${mode_label})"

echo "[summary] ----------------------------------------"
echo "[summary] running packaged protocol validation"
bash "${ROOT}/scripts/container_protocol_validation.sh" "${mode_args[@]}"

echo "[summary] ----------------------------------------"
echo "[summary] running packaged operator-path validation"
bash "${ROOT}/scripts/container_operator_path_validation.sh" "${mode_args[@]}"

echo "[summary] ----------------------------------------"
echo "[summary] packaged container validation: ok (${mode_label})"
