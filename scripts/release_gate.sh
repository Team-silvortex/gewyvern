#!/usr/bin/env bash

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
RUN_BUILD=1
RUN_RELEASE_CHECK=1
RUN_STACK=1
RELEASE_ARGS=()

usage() {
  cat <<'EOF'
Usage: scripts/release_gate.sh [--skip-build] [--skip-release-check] [--skip-stack] [--deb|--rpm]

Run the current release gate as one deliberate sequence:

1. rebuild fresh native packages in Docker
2. run the packaged release validation wrapper
3. run the three-module stack smoke

Flags:
  --skip-build          Reuse current package artifacts instead of rebuilding
  --skip-release-check  Skip packaged DEB/RPM validation
  --skip-stack          Skip three-module stack smoke
  --deb                 Run the packaged release check in DEB-only mode
  --rpm                 Run the packaged release check in RPM-only mode
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --skip-build)
      RUN_BUILD=0
      shift
      ;;
    --skip-release-check)
      RUN_RELEASE_CHECK=0
      shift
      ;;
    --skip-stack)
      RUN_STACK=0
      shift
      ;;
    --deb|--rpm)
      RELEASE_ARGS=("$1")
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

run_step() {
  local label="$1"
  shift
  echo "[release-gate] ----------------------------------------"
  echo "[release-gate] ${label}"
  "$@"
}

if [[ "${RUN_BUILD}" -eq 1 ]]; then
  run_step \
    "building fresh native artifacts" \
    bash "${ROOT}/scripts/build_packages_in_container.sh" --format all
else
  echo "[release-gate] skipping package rebuild"
fi

if [[ "${RUN_RELEASE_CHECK}" -eq 1 ]]; then
  if [[ "${#RELEASE_ARGS[@]}" -eq 0 ]]; then
    run_step \
      "running packaged release validation" \
      bash "${ROOT}/scripts/release_container_check.sh"
  else
    run_step \
      "running packaged release validation (${RELEASE_ARGS[0]#--})" \
      bash "${ROOT}/scripts/release_container_check.sh" "${RELEASE_ARGS[@]}"
  fi
else
  echo "[release-gate] skipping packaged release validation"
fi

if [[ "${RUN_STACK}" -eq 1 ]]; then
  run_step \
    "running three-module stack smoke" \
    bash "${ROOT}/scripts/three_module_stack_smoke.sh"
else
  echo "[release-gate] skipping three-module stack smoke"
fi

echo "[release-gate] ----------------------------------------"
echo "[release-gate] release gate: ok"
