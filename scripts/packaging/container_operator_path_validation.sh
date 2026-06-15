#!/usr/bin/env bash

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
source "${ROOT}/scripts/packaging/container_validation_common.sh"
PACKAGES_DIR="${ROOT}/target/packages"
DEB_IMAGE="${GEWY_DEB_OPERATOR_IMAGE:-ubuntu:24.04}"
RPM_IMAGE="${GEWY_RPM_OPERATOR_IMAGE:-fedora:41}"

usage() {
  cat <<'EOF'
Usage: scripts/packaging/container_operator_path_validation.sh [--deb] [--rpm]

Install the latest local native package into a clean Linux container and verify
packaged operator-path protocol chains in a more realistic sequence.

By default, both the DEB and RPM paths run.
EOF
}

container_validation_parse_mode_args usage "$@"
container_validation_require_docker "container operator-path validation"

operator_validation_body() {
  cat <<'EOF'
set -euo pipefail

expect_contains() {
  local file="$1"
  local needle="$2"
  if ! grep -q "$needle" "$file"; then
    echo "expected to find '${needle}' in ${file}" >&2
    exit 1
  fi
}

echo "[operator-path] validating advisory resolution and application paths"

# Advisory resolution and application paths
# DNS -> QUIC -> HTTP/3 operator path
gewyvern --protocol dns --entry udp --json --summary-only >/tmp/path-dns.json
gewyvern --protocol quic --entry initial --json --summary-only >/tmp/path-quic.json
gewyvern --protocol http3 --entry request --json --summary-only >/tmp/path-http3.json

expect_contains /tmp/path-dns.json '"primary_module_kind":"name_resolution"'
expect_contains /tmp/path-dns.json '"operator_guidance_action":"manual_review"'
expect_contains /tmp/path-quic.json '"primary_module_kind":"quic_handshake"'
expect_contains /tmp/path-quic.json '"primary_failure_basis":"missing_transition"'
expect_contains /tmp/path-quic.json '"operator_guidance_action":"collect_more_runtime_evidence"'
expect_contains /tmp/path-http3.json '"primary_module_kind":"http3_request_response"'
expect_contains /tmp/path-http3.json '"operator_guidance_action":"safe_to_escalate_protocol_signal"'

echo "[operator-path] validating secure transport and tunnel paths"

# Secure transport and tunnel paths
# DNS -> TLS -> HTTPS CONNECT operator path
gewyvern --protocol tls --entry client --json --summary-only >/tmp/path-tls.json
gewyvern --protocol https --entry connect --json --summary-only >/tmp/path-https-connect.json

expect_contains /tmp/path-tls.json '"primary_module_kind":"tls_handshake"'
expect_contains /tmp/path-tls.json '"operator_guidance_action":"manual_review"'
expect_contains /tmp/path-https-connect.json '"primary_module_kind":"network_module"'
expect_contains /tmp/path-https-connect.json '"operator_guidance_action":"manual_review"'

# DNS -> SOCKS5 -> HTTPS CONNECT operator path
gewyvern --protocol socks5 --entry auth --json --summary-only >/tmp/path-socks5-auth.json

expect_contains /tmp/path-socks5-auth.json '"primary_module_kind":"proxy_authentication"'
expect_contains /tmp/path-socks5-auth.json '"primary_failure_basis":"missing_transition"'
expect_contains /tmp/path-socks5-auth.json '"operator_guidance_action":"collect_more_runtime_evidence"'

echo "[operator-path] validating secure database and mail paths"

# Secure database and mail paths
# DNS -> TLS -> Postgres operator path
gewyvern --protocol postgres --entry query --json --summary-only >/tmp/path-postgres.json

expect_contains /tmp/path-postgres.json '"primary_module_kind":"database_query"'
expect_contains /tmp/path-postgres.json '"primary_failure_basis":"missing_transition"'
expect_contains /tmp/path-postgres.json '"operator_guidance_action":"collect_more_runtime_evidence"'

# DNS -> TLS -> MySQL operator path
gewyvern --protocol mysql --entry session --json --summary-only >/tmp/path-mysql.json

expect_contains /tmp/path-mysql.json '"primary_module_kind":"database_query"'
expect_contains /tmp/path-mysql.json '"primary_failure_basis":"missing_transition"'
expect_contains /tmp/path-mysql.json '"operator_guidance_action":"collect_more_runtime_evidence"'

# DNS -> TLS -> SMTP auth operator path
gewyvern --protocol smtp --entry auth --json --summary-only >/tmp/path-smtp-auth.json

expect_contains /tmp/path-smtp-auth.json '"primary_module_kind":"authentication_exchange"'
expect_contains /tmp/path-smtp-auth.json '"primary_failure_basis":"missing_transition"'
expect_contains /tmp/path-smtp-auth.json '"operator_guidance_action":"collect_more_runtime_evidence"'

# DNS -> SMTP operator path
gewyvern --protocol smtp --entry session --json --summary-only >/tmp/path-smtp.json

expect_contains /tmp/path-smtp.json '"primary_module_kind":"mail_session"'
expect_contains /tmp/path-smtp.json '"primary_failure_basis":"missing_transition"'
expect_contains /tmp/path-smtp.json '"operator_guidance_action":"collect_more_runtime_evidence"'

echo "[operator-path] validating conservative negative-path guard"

# Negative-path guard: packaged denied demos should not over-collapse
gewyvern --protocol socks5 --entry auth-denied --json --summary-only >/tmp/path-socks5-auth-denied.json

expect_contains /tmp/path-socks5-auth-denied.json '"primary_module_kind":"proxy_authentication"'
expect_contains /tmp/path-socks5-auth-denied.json '"primary_failure_basis":"missing_transition"'
expect_contains /tmp/path-socks5-auth-denied.json '"operator_guidance_action":"collect_more_runtime_evidence"'

echo "container operator path validation: ok"
EOF
}

run_deb_operator_validation() {
  local deb_path
  deb_path="$(container_validation_find_latest_deb "${PACKAGES_DIR}")"
  if [[ -z "${deb_path}" ]]; then
    echo "no .deb artifact found under ${PACKAGES_DIR}" >&2
    exit 1
  fi

  container_validation_run_deb \
    "${PACKAGES_DIR}" \
    "${DEB_IMAGE}" \
    "${deb_path}" \
    "$(operator_validation_body)"

  echo "deb operator path validation: ok (${deb_path})"
}

run_rpm_operator_validation() {
  local rpm_path
  rpm_path="$(container_validation_find_latest_rpm "${PACKAGES_DIR}")"
  if [[ -z "${rpm_path}" ]]; then
    echo "no .rpm artifact found under ${PACKAGES_DIR}/rpm" >&2
    exit 1
  fi

  container_validation_run_rpm \
    "${PACKAGES_DIR}" \
    "${RPM_IMAGE}" \
    "${rpm_path}" \
    "$(operator_validation_body)"

  echo "rpm operator path validation: ok (${rpm_path})"
}

if [[ "${RUN_DEB}" -eq 1 ]]; then
  run_deb_operator_validation
fi

if [[ "${RUN_RPM}" -eq 1 ]]; then
  run_rpm_operator_validation
fi

echo "container operator path validation: ok"
