#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TMP_DIR="$(mktemp -d "${TMPDIR:-/tmp}/gewyvern-field-validation.XXXXXX")"
trap 'rm -rf "${TMP_DIR}"' EXIT

expect_contains() {
  local file="$1"
  local needle="$2"
  if ! grep -q "$needle" "$file"; then
    echo "expected to find '${needle}' in ${file}" >&2
    exit 1
  fi
}

echo "[1/4] standalone demo summary"
SUMMARY_JSON="${TMP_DIR}/demo-summary.json"
(
  cd "${ROOT}"
  cargo run --quiet -- --demo udp --json --summary-only > "${SUMMARY_JSON}"
)
expect_contains "${SUMMARY_JSON}" '"primary_failure_mode"'
expect_contains "${SUMMARY_JSON}" '"operator_guidance_action"'

echo "[2/4] standalone DSL summary"
DSL_JSON="${TMP_DIR}/dsl-summary.json"
(
  cd "${ROOT}"
  cargo run --quiet -- --dsl "${ROOT}/dsl/http_request_path.gewy" --json --summary-only > "${DSL_JSON}"
)
expect_contains "${DSL_JSON}" '"primary_module_kind"'
expect_contains "${DSL_JSON}" '"process_network_profiles"'

echo "[3/4] gewyc explain surface"
EXPLAIN_JSON="${TMP_DIR}/explain.json"
(
  cd "${ROOT}"
  cargo run --quiet -p gewyc -- explain "${ROOT}/dsl/http_request_path.gewy" --json > "${EXPLAIN_JSON}"
)
expect_contains "${EXPLAIN_JSON}" '"summary"'
expect_contains "${EXPLAIN_JSON}" '"next_step"'

if [ "${GEWY_FIELD_VALIDATE_SOCKET:-0}" = "1" ]; then
  echo "[4/4] socket roundtrip"
  ROUNDTRIP_JSON="${TMP_DIR}/socket.json"
  (
    cd "${ROOT}"
    bash "${ROOT}/scripts/socket_roundtrip_demo.sh" "/private/tmp/gewyvern-field-validation.sock" udp "${ROUNDTRIP_JSON}" unix > /dev/null
  )
  expect_contains "${ROUNDTRIP_JSON}" '"template_id"'
  expect_contains "${ROUNDTRIP_JSON}" '"facts"'
else
  echo "[4/4] socket roundtrip skipped (set GEWY_FIELD_VALIDATE_SOCKET=1 to enable)"
fi

if [ "${GEWY_FIELD_VALIDATE_SCAN_ALL:-0}" = "1" ]; then
  echo "[extra] registry-wide scan validation"
  SCAN_JSON="${TMP_DIR}/scan.json"
  (
    cd "${ROOT}"
    cargo run --quiet -- --scan-all --json --summary-only > "${SCAN_JSON}"
  )
  expect_contains "${SCAN_JSON}" '"kind":"scan"'
  expect_contains "${SCAN_JSON}" '"target_count"'
fi

echo "field validation smoke: ok"
