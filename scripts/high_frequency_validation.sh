#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TMP_DIR="$(mktemp -d "${TMPDIR:-/tmp}/gewyvern-high-frequency-validation.XXXXXX")"
trap 'rm -rf "${TMP_DIR}"' EXIT

expect_contains() {
  local file="$1"
  local needle="$2"
  if ! grep -q "$needle" "$file"; then
    echo "expected to find '${needle}' in ${file}" >&2
    exit 1
  fi
}

echo "[1/3] standalone high-frequency protocol summaries"

HTTP_JSON="${TMP_DIR}/http-request.json"
TLS_JSON="${TMP_DIR}/tls-client.json"
SSH_JSON="${TMP_DIR}/ssh-session.json"
SOCKS_JSON="${TMP_DIR}/socks5-auth.json"
PG_JSON="${TMP_DIR}/postgres-query.json"

(
  cd "${ROOT}"
  cargo run --quiet -- --dsl "${ROOT}/dsl/http_request_path.gewy" --json --summary-only >"${HTTP_JSON}"
  cargo run --quiet -- --dsl "${ROOT}/dsl/tls_client_path.gewy" --json --summary-only >"${TLS_JSON}"
  cargo run --quiet -- --dsl "${ROOT}/dsl/ssh_session_path.gewy" --json --summary-only >"${SSH_JSON}"
  cargo run --quiet -- --dsl "${ROOT}/dsl/socks5_auth_path.gewy" --json --summary-only >"${SOCKS_JSON}"
  cargo run --quiet -- --protocol postgres --entry query --json --summary-only >"${PG_JSON}"
)

expect_contains "${HTTP_JSON}" '"primary_module_kind":"http_request_response"'
expect_contains "${TLS_JSON}" '"primary_module_kind":"tls_handshake"'
expect_contains "${SSH_JSON}" '"primary_module_kind":"remote_access_session"'
expect_contains "${SOCKS_JSON}" '"primary_module_kind":"proxy_authentication"'
expect_contains "${PG_JSON}" '"primary_module_kind":"database_query"'

echo "[2/3] mixed-flow conservatism tests"
(
  cd "${ROOT}"
  cargo test --quiet --bin gewyvern mixed_dns_tls_http_profile_stays_ambiguous_and_low_confidence
  cargo test --quiet --bin gewyvern mixed_proxy_tunnel_and_upstream_request_exposes_competing_hypotheses
  cargo test --quiet --bin gewyvern mixed_quic_http3_hy2_profile_stays_conservative
)

echo "[3/3] targeted operator-guidance expectations"
expect_contains "${HTTP_JSON}" '"operator_guidance_action":"manual_review"'
expect_contains "${TLS_JSON}" '"operator_guidance_action":"manual_review"'
expect_contains "${SSH_JSON}" '"operator_guidance_action":"collect_more_runtime_evidence"'
expect_contains "${SOCKS_JSON}" '"operator_guidance_action":"collect_more_runtime_evidence"'
expect_contains "${PG_JSON}" '"operator_guidance_action":"collect_more_runtime_evidence"'

echo "high-frequency validation: ok"
