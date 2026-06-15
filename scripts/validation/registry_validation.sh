#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
TMP_DIR="$(mktemp -d "${TMPDIR:-/tmp}/gewyvern-registry-validation.XXXXXX")"
trap 'rm -rf "${TMP_DIR}"' EXIT

LIMIT="${GEWY_REGISTRY_LIMIT:-0}"
STRICT_JSON="${GEWY_REGISTRY_STRICT_JSON:-1}"

TOTAL=0
COMMAND_FAIL=0
JSON_FAIL=0
PARSE_FAIL=0
VALIDATION_FAIL=0
DIAGNOSTICS_FAIL=0

FAILED_REPORT="${TMP_DIR}/failed.txt"
touch "${FAILED_REPORT}"

record_failure() {
  local rel="$1"
  local reason="$2"
  printf '%s :: %s\n' "${rel}" "${reason}" | tee -a "${FAILED_REPORT}" >/dev/null
}

echo "registry validation root: ${ROOT}"
echo "strict json: ${STRICT_JSON}"

while IFS= read -r pkg; do
  if [ "${LIMIT}" -gt 0 ] && [ "${TOTAL}" -ge "${LIMIT}" ]; then
    break
  fi

  TOTAL=$((TOTAL + 1))
  DIR="$(dirname "${pkg}")"
  MAIN="${DIR}/main.gewy"
  REL="${DIR#${ROOT}/}"
  OUT="${TMP_DIR}/case-${TOTAL}.json"

  printf '[%s] %s\n' "${TOTAL}" "${REL}"

  if ! (
    cd "${ROOT}"
    cargo run --quiet -p gewyc -- envelope "${MAIN}" --json > "${OUT}"
  ); then
    COMMAND_FAIL=$((COMMAND_FAIL + 1))
    printf '  command_failed\n'
    record_failure "${REL}" "command_failed"
  else
    if [ "${STRICT_JSON}" = "1" ]; then
      if ! python3 - "${OUT}" <<'PY'
import json,sys
with open(sys.argv[1], 'r', encoding='utf-8') as fh:
    json.load(fh)
PY
      then
        JSON_FAIL=$((JSON_FAIL + 1))
        printf '  invalid_json\n'
        record_failure "${REL}" "invalid_json"
        continue
      fi
    fi

    if ! grep -q '"parse":{"ok":true' "${OUT}"; then
      PARSE_FAIL=$((PARSE_FAIL + 1))
      printf '  parse_failed\n'
      record_failure "${REL}" "parse_failed"
      continue
    fi

    if ! grep -q '"validation":{"ok":true' "${OUT}"; then
      VALIDATION_FAIL=$((VALIDATION_FAIL + 1))
      printf '  validation_failed\n'
      record_failure "${REL}" "validation_failed"
      continue
    fi

    if ! grep -q '"diagnostics":{"ok":true' "${OUT}"; then
      DIAGNOSTICS_FAIL=$((DIAGNOSTICS_FAIL + 1))
      printf '  diagnostics_failed\n'
      record_failure "${REL}" "diagnostics_failed"
      continue
    fi

    printf '  ok\n'
  fi
done < <(find "${ROOT}/protocols" -name gewy.pkg | sort)

echo
echo "registry validation summary"
echo "  total=${TOTAL}"
echo "  command_fail=${COMMAND_FAIL}"
echo "  json_fail=${JSON_FAIL}"
echo "  parse_fail=${PARSE_FAIL}"
echo "  validation_fail=${VALIDATION_FAIL}"
echo "  diagnostics_fail=${DIAGNOSTICS_FAIL}"

if [ -s "${FAILED_REPORT}" ]; then
  echo
  echo "failed cases"
  cat "${FAILED_REPORT}"
  exit 1
fi

echo
echo "registry validation: ok"
