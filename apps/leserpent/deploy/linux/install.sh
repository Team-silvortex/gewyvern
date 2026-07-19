#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
SOURCE_DIR="$(cd -- "${SCRIPT_DIR}/.." && pwd)"
DESTDIR="${DESTDIR:-}"
NO_START=false
KEEP_RELEASES=3
ACTION=install

usage() {
  cat <<'EOF'
Usage: install.sh [options]

Install or atomically upgrade a published Leserpent Native AOT bundle.

Options:
  --source DIR       Published bundle root (default: parent of this script)
  --rollback         Atomically switch current and previous retained releases
  --no-start         Install files without enabling or restarting systemd
  --keep-releases N  Number of releases retained after a healthy upgrade (default: 3)
  -h, --help         Show this help

Set DESTDIR to stage the filesystem layout without users, systemd, or health checks.
EOF
}

while (($#)); do
  case "$1" in
    --source)
      SOURCE_DIR="${2:?--source requires a directory}"
      shift 2
      ;;
    --no-start)
      NO_START=true
      shift
      ;;
    --rollback)
      ACTION=rollback
      shift
      ;;
    --keep-releases)
      KEEP_RELEASES="${2:?--keep-releases requires a number}"
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      printf 'unknown option: %s\n' "$1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

if [[ ! "${KEEP_RELEASES}" =~ ^[2-9][0-9]*$ ]]; then
  printf '%s\n' '--keep-releases must be an integer of at least 2' >&2
  exit 2
fi

if [[ -z "${DESTDIR}" && "${EUID}" -ne 0 ]]; then
  printf '%s\n' 'run as root, or set DESTDIR for a staged installation' >&2
  exit 1
fi

prefix="${DESTDIR}/opt/leserpent"
config_dir="${DESTDIR}/etc/leserpent"
state_dir="${DESTDIR}/var/lib/leserpent"
unit_dir="${DESTDIR}/etc/systemd/system"

release_link_target() {
  local name="$1" target
  [[ -L "${prefix}/${name}" ]] || return 1
  target="$(readlink "${prefix}/${name}")"
  [[ "${target}" =~ ^releases/[A-Za-z0-9._-]+$ ]] || return 1
  [[ -d "${prefix}/${target}" ]] || return 1
  printf '%s' "${target}"
}

replace_release_link() {
  local name="$1" target="$2"
  local pending="${prefix}/.${name}.new"
  rm -f "${pending}"
  ln -s "${target}" "${pending}"
  mv -Tf "${pending}" "${prefix}/${name}"
}

health_check() {
  if command -v curl >/dev/null 2>&1; then
    curl --fail --silent --show-error http://127.0.0.1:5210/health >/dev/null 2>&1
    return
  fi

  local status_line
  exec 3<>/dev/tcp/127.0.0.1:5210 || return 1
  printf 'GET /health HTTP/1.0\r\nHost: 127.0.0.1\r\n\r\n' >&3
  IFS= read -r status_line <&3 || true
  exec 3>&-
  exec 3<&-
  [[ "${status_line}" == *" 200 "* ]]
}

wait_until_healthy() {
  for _ in {1..30}; do
    health_check && return 0
    sleep 1
  done
  return 1
}

if [[ "${ACTION}" == rollback ]]; then
  current_target="$(release_link_target current)" || {
    printf '%s\n' 'cannot rollback: current release link is missing or unsafe' >&2
    exit 1
  }
  rollback_target="$(release_link_target previous)" || {
    printf '%s\n' 'cannot rollback: previous release link is missing or unsafe' >&2
    exit 1
  }
  [[ "${current_target}" != "${rollback_target}" ]] || {
    printf '%s\n' 'cannot rollback: current and previous releases are identical' >&2
    exit 1
  }
  replace_release_link previous "${current_target}"
  replace_release_link current "${rollback_target}"
  if [[ -n "${DESTDIR}" ]]; then
    printf 'rolled back staged Leserpent from %s to %s\n' "${current_target}" "${rollback_target}"
    exit 0
  fi
  systemctl restart leserpent.service
  if ! wait_until_healthy; then
    replace_release_link previous "${rollback_target}"
    replace_release_link current "${current_target}"
    systemctl restart leserpent.service || true
    printf '%s\n' 'Leserpent rollback health check failed; restored original release' >&2
    exit 1
  fi
  printf 'Leserpent rollback is healthy: %s\n' "${rollback_target}"
  exit 0
fi

for required in Leserpent leserpent-compat-bridge libe_sqlite3.so wwwroot deploy/leserpent.service deploy/leserpent.env.example; do
  if [[ ! -e "${SOURCE_DIR}/${required}" ]]; then
    printf 'invalid Leserpent bundle: missing %s\n' "${required}" >&2
    exit 1
  fi
done

release_hash="$(sha256sum "${SOURCE_DIR}/Leserpent" | cut -c1-12)"
release_id="$(date -u +%Y%m%d%H%M%S%N)-${release_hash}"
release_dir="${prefix}/releases/${release_id}"
previous_target=""

if [[ -e "${prefix}/current" || -L "${prefix}/current" ]]; then
  previous_target="$(release_link_target current)" || {
    printf '%s\n' 'cannot upgrade: current release link is missing or unsafe' >&2
    exit 1
  }
fi

install -d -m 0755 "${prefix}/releases" "${release_dir}" "${config_dir}" "${unit_dir}"
cp -a "${SOURCE_DIR}/." "${release_dir}/"
rm -rf "${release_dir}/deploy"
find "${release_dir}" -type d -exec chmod 0755 {} +
find "${release_dir}" -type f -exec chmod 0644 {} +
find "${release_dir}" -type f -name '*.dbg' -delete
chmod 0755 "${release_dir}/Leserpent"
chmod 0755 "${release_dir}/leserpent-compat-bridge"
install -m 0644 "${SOURCE_DIR}/deploy/leserpent.service" "${unit_dir}/leserpent.service"

if [[ ! -f "${config_dir}/leserpent.env" ]]; then
  install -m 0600 "${SOURCE_DIR}/deploy/leserpent.env.example" "${config_dir}/leserpent.env"
  if command -v openssl >/dev/null 2>&1; then
    admin_token="$(openssl rand -hex 32)"
  else
    admin_token="$(od -An -N32 -tx1 /dev/urandom | tr -d ' \n')"
  fi
  printf 'LESERPENT_ADMIN_TOKEN=%s\n' "${admin_token}" >>"${config_dir}/leserpent.env"
fi

install -d -m 0750 "${state_dir}"
replace_release_link current "releases/${release_id}"

if [[ -n "${DESTDIR}" ]]; then
  if [[ -n "${previous_target}" ]]; then
    replace_release_link previous "${previous_target}"
  fi
  printf 'staged Leserpent %s under %s\n' "${release_id}" "${DESTDIR}"
  exit 0
fi

if ! getent group leserpent >/dev/null; then
  groupadd --system leserpent
fi
if ! id -u leserpent >/dev/null 2>&1; then
  useradd --system --gid leserpent --home-dir /var/lib/leserpent --shell /usr/sbin/nologin leserpent
fi
chown -R root:leserpent /opt/leserpent /etc/leserpent
chown -R leserpent:leserpent /var/lib/leserpent
chmod 0750 /etc/leserpent
chmod 0600 /etc/leserpent/leserpent.env

if [[ "${NO_START}" == true ]]; then
  if [[ -n "${previous_target}" ]]; then
    replace_release_link previous "${previous_target}"
  fi
  printf 'installed Leserpent %s; systemd was not changed\n' "${release_id}"
  exit 0
fi

systemctl daemon-reload
systemctl enable leserpent.service >/dev/null
systemctl restart leserpent.service


if ! wait_until_healthy; then
  printf '%s\n' 'Leserpent health check failed; restoring the previous release' >&2
  if [[ -n "${previous_target}" ]]; then
    replace_release_link current "${previous_target}"
    systemctl restart leserpent.service || true
  else
    systemctl stop leserpent.service || true
  fi
  systemctl --no-pager --full status leserpent.service >&2 || true
  exit 1
fi

if [[ -n "${previous_target}" ]]; then
  replace_release_link previous "${previous_target}"
fi

mapfile -t old_releases < <(find "${prefix}/releases" -mindepth 1 -maxdepth 1 -type d -printf '%f\n' | sort -r | tail -n "+$((KEEP_RELEASES + 1))")
current_release="$(basename "$(release_link_target current)")"
previous_release=""
if [[ -L "${prefix}/previous" ]]; then
  previous_release="$(basename "$(release_link_target previous)")"
fi
for old_release in "${old_releases[@]}"; do
  if [[ "${old_release}" == "${current_release}" || "${old_release}" == "${previous_release}" ]]; then
    continue
  fi
  rm -rf "${prefix}/releases/${old_release}"
done

printf 'Leserpent %s is healthy at http://127.0.0.1:5210/\n' "${release_id}"
printf 'configuration: /etc/leserpent/leserpent.env\n'
printf 'status: systemctl status leserpent\n'
printf 'logs: journalctl -u leserpent -f\n'
