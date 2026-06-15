#!/usr/bin/env bash

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
source "${ROOT}/scripts/packaging/container_validation_common.sh"
PACKAGES_DIR="${ROOT}/target/packages"
DEB_IMAGE="${GEWY_DEB_PROTOCOL_IMAGE:-ubuntu:24.04}"
RPM_IMAGE="${GEWY_RPM_PROTOCOL_IMAGE:-fedora:41}"

usage() {
  cat <<'EOF'
Usage: scripts/packaging/container_protocol_validation.sh [--deb] [--rpm]

Install the latest local native package into a clean Linux container and run a
packaged high-frequency protocol validation path.

By default, both the DEB and RPM paths run.
EOF
}

container_validation_parse_mode_args usage "$@"
container_validation_require_docker "container protocol validation"

protocol_validation_body() {
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

echo "[protocol] validating protocol registry visibility"

gewyvern --list-protocols >/tmp/list-protocols.txt
expect_contains /tmp/list-protocols.txt 'dns (default: udp)'
expect_contains /tmp/list-protocols.txt 'http (default: request)'
expect_contains /tmp/list-protocols.txt 'tls (default: client)'
expect_contains /tmp/list-protocols.txt 'http3 (default: request)'
expect_contains /tmp/list-protocols.txt 'quic (default: initial)'
expect_contains /tmp/list-protocols.txt 'ssh (default: session)'
expect_contains /tmp/list-protocols.txt 'socks5 (default: session)'
expect_contains /tmp/list-protocols.txt 'mysql (default: session)'
expect_contains /tmp/list-protocols.txt 'postgres (default: query)'
expect_contains /tmp/list-protocols.txt 'smtp (default: session)'
expect_contains /tmp/list-protocols.txt 'ldap (default: sync)'
expect_contains /tmp/list-protocols.txt 'redis (default: ping)'
expect_contains /tmp/list-protocols.txt 'mqtt (default: connect)'
expect_contains /tmp/list-protocols.txt 'amqp (default: session)'
expect_contains /tmp/list-protocols.txt 'radius (default: access)'
expect_contains /tmp/list-protocols.txt 'snmp (default: get)'
expect_contains /tmp/list-protocols.txt 'ftp (default: session)'
expect_contains /tmp/list-protocols.txt 'imap (default: auth)'
expect_contains /tmp/list-protocols.txt 'pop3 (default: auth)'
expect_contains /tmp/list-protocols.txt 'kerberos (default: as)'
expect_contains /tmp/list-protocols.txt 'rtsp (default: options)'

echo "[protocol] validating resolution, web, and secure transport families"

gewyvern --protocol dns --entry udp --json --summary-only >/tmp/dns.json
gewyvern --protocol http --entry request --json --summary-only >/tmp/http.json
gewyvern --protocol tls --entry client --json --summary-only >/tmp/tls.json
gewyvern --protocol http3 --entry request --json --summary-only >/tmp/http3.json
gewyvern --protocol quic --entry initial --json --summary-only >/tmp/quic.json

expect_contains /tmp/dns.json '"primary_module_kind":"name_resolution"'
expect_contains /tmp/dns.json '"operator_guidance_action":"manual_review"'
expect_contains /tmp/http.json '"primary_module_kind":"http_request_response"'
expect_contains /tmp/http.json '"operator_guidance_action":"manual_review"'
expect_contains /tmp/tls.json '"primary_module_kind":"tls_handshake"'
expect_contains /tmp/tls.json '"operator_guidance_action":"manual_review"'
expect_contains /tmp/http3.json '"primary_module_kind":"http3_request_response"'
expect_contains /tmp/http3.json '"operator_guidance_action":"safe_to_escalate_protocol_signal"'
expect_contains /tmp/quic.json '"primary_module_kind":"quic_handshake"'
expect_contains /tmp/quic.json '"operator_guidance_action":"collect_more_runtime_evidence"'

echo "[protocol] validating remote access and proxy families"

gewyvern --protocol ssh --entry session --json --summary-only >/tmp/ssh.json
gewyvern --protocol socks5 --entry auth --json --summary-only >/tmp/socks5.json

expect_contains /tmp/ssh.json '"primary_module_kind":"remote_access_session"'
expect_contains /tmp/ssh.json '"operator_guidance_action":"collect_more_runtime_evidence"'
expect_contains /tmp/socks5.json '"primary_module_kind":"proxy_authentication"'
expect_contains /tmp/socks5.json '"operator_guidance_action":"collect_more_runtime_evidence"'

echo "[protocol] validating database, messaging, and directory families"

gewyvern --protocol mysql --entry session --json --summary-only >/tmp/mysql.json
gewyvern --protocol mysql --entry query --json --summary-only >/tmp/mysql-query.json
gewyvern --protocol postgres --entry query --json --summary-only >/tmp/postgres.json
gewyvern --protocol smtp --entry session --json --summary-only >/tmp/smtp.json
gewyvern --protocol ldap --entry sync --json --summary-only >/tmp/ldap.json

expect_contains /tmp/mysql.json '"primary_module_kind":"database_query"'
expect_contains /tmp/mysql.json '"operator_guidance_action":"collect_more_runtime_evidence"'
expect_contains /tmp/mysql-query.json '"primary_module_kind":"database_query"'
expect_contains /tmp/mysql-query.json '"operator_guidance_action":"collect_more_runtime_evidence"'
expect_contains /tmp/postgres.json '"primary_module_kind":"database_query"'
expect_contains /tmp/postgres.json '"operator_guidance_action":"collect_more_runtime_evidence"'
expect_contains /tmp/smtp.json '"primary_module_kind":"mail_session"'
expect_contains /tmp/smtp.json '"operator_guidance_action":"collect_more_runtime_evidence"'
expect_contains /tmp/ldap.json '"primary_module_kind":"directory_sync"'
expect_contains /tmp/ldap.json '"operator_guidance_action":"collect_more_runtime_evidence"'

echo "[protocol] validating cache, broker, auth, management, and signaling families"

gewyvern --protocol redis --entry ping --json --summary-only >/tmp/redis.json
gewyvern --protocol mqtt --entry connect --json --summary-only >/tmp/mqtt.json
gewyvern --protocol amqp --entry start --json --summary-only >/tmp/amqp.json
gewyvern --protocol radius --entry access --json --summary-only >/tmp/radius.json
gewyvern --protocol snmp --entry get --json --summary-only >/tmp/snmp.json
gewyvern --protocol ftp --entry session --json --summary-only >/tmp/ftp.json
gewyvern --protocol imap --entry auth --json --summary-only >/tmp/imap.json
gewyvern --protocol pop3 --entry auth --json --summary-only >/tmp/pop3.json
gewyvern --protocol kerberos --entry as --json --summary-only >/tmp/kerberos.json
gewyvern --protocol rtsp --entry describe --json --summary-only >/tmp/rtsp.json

expect_contains /tmp/redis.json '"primary_module_kind":"cache_access"'
expect_contains /tmp/redis.json '"operator_guidance_action":"collect_more_runtime_evidence"'
expect_contains /tmp/mqtt.json '"primary_module_kind":"message_session"'
expect_contains /tmp/mqtt.json '"operator_guidance_action":"collect_more_runtime_evidence"'
expect_contains /tmp/amqp.json '"primary_module_kind":"message_session"'
expect_contains /tmp/amqp.json '"operator_guidance_action":"collect_more_runtime_evidence"'
expect_contains /tmp/radius.json '"primary_module_kind":"authentication_exchange"'
expect_contains /tmp/radius.json '"operator_guidance_action":"collect_more_runtime_evidence"'
expect_contains /tmp/snmp.json '"primary_module_kind":"management_query"'
expect_contains /tmp/snmp.json '"operator_guidance_action":"collect_more_runtime_evidence"'
expect_contains /tmp/ftp.json '"primary_module_kind":"authentication_exchange"'
expect_contains /tmp/ftp.json '"operator_guidance_action":"collect_more_runtime_evidence"'
expect_contains /tmp/imap.json '"primary_module_kind":"authentication_exchange"'
expect_contains /tmp/imap.json '"operator_guidance_action":"collect_more_runtime_evidence"'
expect_contains /tmp/pop3.json '"primary_module_kind":"authentication_exchange"'
expect_contains /tmp/pop3.json '"operator_guidance_action":"collect_more_runtime_evidence"'
expect_contains /tmp/kerberos.json '"primary_module_kind":"authentication_exchange"'
expect_contains /tmp/kerberos.json '"operator_guidance_action":"collect_more_runtime_evidence"'
expect_contains /tmp/rtsp.json '"primary_module_kind":"signaling_session"'
expect_contains /tmp/rtsp.json '"operator_guidance_action":"collect_more_runtime_evidence"'

echo "[protocol] validating full packaged registry sweep"

gewyvern --scan-all --json --summary-only >/tmp/scan-all.json
expect_contains /tmp/scan-all.json '"total_targets":'

echo "container protocol validation: ok"
EOF
}

run_deb_protocol_validation() {
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
    "$(protocol_validation_body)"

  echo "deb protocol validation: ok (${deb_path})"
}

run_rpm_protocol_validation() {
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
    "$(protocol_validation_body)"

  echo "rpm protocol validation: ok (${rpm_path})"
}

if [[ "${RUN_DEB}" -eq 1 ]]; then
  run_deb_protocol_validation
fi

if [[ "${RUN_RPM}" -eq 1 ]]; then
  run_rpm_protocol_validation
fi

echo "container protocol validation: ok"
