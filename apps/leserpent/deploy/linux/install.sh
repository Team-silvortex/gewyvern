#!/usr/bin/env bash
set -euo pipefail
export LC_ALL=C

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

if [[ ! "${KEEP_RELEASES}" =~ ^[1-9][0-9]*$ ]] \
  || ((KEEP_RELEASES < 2 || KEEP_RELEASES > 64)); then
  printf '%s\n' '--keep-releases must be an integer from 2 through 64' >&2
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
unit_path="${unit_dir}/leserpent.service"
unit_pending="${unit_dir}/.leserpent.service.pending.$$"
unit_backup="${unit_dir}/.leserpent.service.backup.$$"
unit_transaction_started=false
unit_existed=false
unit_touched=false

for managed_dir in "${prefix}" "${prefix}/releases" "${config_dir}" "${state_dir}" "${unit_dir}"; do
  if [[ (-e "${managed_dir}" || -L "${managed_dir}") \
    && (! -d "${managed_dir}" || -L "${managed_dir}") ]]; then
    printf 'refusing unsafe Leserpent managed directory: %s\n' "${managed_dir}" >&2
    exit 1
  fi
done

verify_bundle() {
  local root="$1" line hash separator path previous="" unsafe index size
  local manifest_files manifest_bytes actual_files=0 actual_bytes=0
  local -a expected_paths=() actual_paths=() components=()
  [[ -d "${root}" && ! -L "${root}" ]] || {
    printf 'invalid Leserpent bundle root: %s\n' "${root}" >&2
    return 1
  }
  for required in Leserpent leserpent-compat-bridge leserpentd libe_sqlite3.so \
    wwwroot wwwroot/index.html \
    deploy/install.sh deploy/leserpent.service deploy/leserpent.env.example \
    bundle-manifest.toml SHA256SUMS; do
    [[ -e "${root}/${required}" && ! -L "${root}/${required}" ]] || {
      printf 'invalid Leserpent bundle: missing or unsafe %s\n' "${required}" >&2
      return 1
    }
  done
  [[ -d "${root}/wwwroot" ]] || {
    printf '%s\n' 'invalid Leserpent bundle: wwwroot is not a directory' >&2
    return 1
  }
  for executable in Leserpent leserpent-compat-bridge leserpentd deploy/install.sh; do
    [[ -f "${root}/${executable}" && -x "${root}/${executable}" ]] || {
      printf 'invalid Leserpent bundle executable: %s\n' "${executable}" >&2
      return 1
    }
  done
  for regular in libe_sqlite3.so wwwroot/index.html deploy/leserpent.service \
    deploy/leserpent.env.example bundle-manifest.toml SHA256SUMS; do
    [[ -f "${root}/${regular}" ]] || {
      printf 'invalid Leserpent bundle regular file: %s\n' "${regular}" >&2
      return 1
    }
  done
  command -v sha256sum >/dev/null 2>&1 || {
    printf '%s\n' 'sha256sum is required to verify the Leserpent bundle' >&2
    return 1
  }
  unsafe="$(find "${root}" -mindepth 1 ! -type d ! -type f -print -quit)"
  [[ -z "${unsafe}" ]] || {
    printf 'invalid Leserpent bundle node: %s\n' "${unsafe}" >&2
    return 1
  }
  [[ "$(stat -c '%s' "${root}/SHA256SUMS")" -le 1048576 ]] || {
    printf '%s\n' 'Leserpent checksum inventory exceeds its size limit' >&2
    return 1
  }
  [[ "$(stat -c '%s' "${root}/bundle-manifest.toml")" -le 16384 ]] || {
    printf '%s\n' 'Leserpent bundle manifest exceeds its size limit' >&2
    return 1
  }
  while IFS= read -r line || [[ -n "${line}" ]]; do
    [[ "${#line}" -ge 67 ]] || {
      printf '%s\n' 'Leserpent checksum inventory contains a short line' >&2
      return 1
    }
    hash="${line:0:64}"
    separator="${line:64:2}"
    path="${line:66}"
    [[ "${hash}" =~ ^[0-9a-f]{64}$ && "${separator}" == "  " \
      && "${path}" =~ ^[A-Za-z0-9._-]+(/[A-Za-z0-9._-]+)*$ \
      && "${path}" != -* ]] || {
      printf 'Leserpent checksum inventory contains an unsafe line: %s\n' "${line}" >&2
      return 1
    }
    IFS='/' read -r -a components <<<"${path}"
    for component in "${components[@]}"; do
      [[ "${component}" != "." && "${component}" != ".." ]] || {
        printf 'Leserpent checksum path is unsafe: %s\n' "${path}" >&2
        return 1
      }
    done
    [[ -z "${previous}" || "${path}" > "${previous}" ]] || {
      printf 'Leserpent checksum paths are duplicate or unsorted: %s\n' "${path}" >&2
      return 1
    }
    expected_paths+=("${path}")
    previous="${path}"
  done <"${root}/SHA256SUMS"
  ((${#expected_paths[@]} > 0)) || {
    printf '%s\n' 'Leserpent checksum inventory is empty' >&2
    return 1
  }
  ((${#expected_paths[@]} <= 4096)) || {
    printf '%s\n' 'Leserpent checksum inventory exceeds its file limit' >&2
    return 1
  }
  mapfile -t actual_paths < <(
    cd "${root}"
    find . -type f -printf '%P\n' | sed '/^SHA256SUMS$/d' | sort
  )
  [[ "${#actual_paths[@]}" -eq "${#expected_paths[@]}" ]] || {
    printf '%s\n' 'Leserpent bundle file inventory does not match SHA256SUMS' >&2
    return 1
  }
  for index in "${!expected_paths[@]}"; do
    [[ "${expected_paths[index]}" == "${actual_paths[index]}" ]] || {
      printf 'Leserpent bundle inventory mismatch: expected %s, found %s\n' \
        "${expected_paths[index]}" "${actual_paths[index]}" >&2
      return 1
    }
  done
  for path in "${actual_paths[@]}"; do
    [[ "${path}" == bundle-manifest.toml ]] && continue
    size="$(stat -c '%s' "${root}/${path}")"
    [[ "${size}" =~ ^(0|[1-9][0-9]*)$ ]] || {
      printf 'invalid Leserpent bundle file size: %s\n' "${path}" >&2
      return 1
    }
    ((size <= 1073741824 - actual_bytes)) || {
      printf '%s\n' 'Leserpent bundle exceeds its byte limit' >&2
      return 1
    }
    actual_bytes=$((actual_bytes + size))
    actual_files=$((actual_files + 1))
  done
  [[ "$(wc -l <"${root}/bundle-manifest.toml")" -eq 8 ]] || {
    printf '%s\n' 'Leserpent bundle manifest must contain exactly eight fields' >&2
    return 1
  }
  grep -Fqx 'schema_version = 1' "${root}/bundle-manifest.toml"
  grep -Fqx 'product = "leserpent-control"' "${root}/bundle-manifest.toml"
  grep -Eq '^version = "[A-Za-z0-9][A-Za-z0-9.+-]*"$' "${root}/bundle-manifest.toml"
  grep -Fqx 'rid = "linux-x64"' "${root}/bundle-manifest.toml"
  grep -Fqx 'hash_algorithm = "sha256"' "${root}/bundle-manifest.toml"
  grep -Fqx 'inventory = "SHA256SUMS"' "${root}/bundle-manifest.toml"
  manifest_files="$(sed -n 's/^payload_files = \([0-9][0-9]*\)$/\1/p' "${root}/bundle-manifest.toml")"
  manifest_bytes="$(sed -n 's/^payload_bytes = \([0-9][0-9]*\)$/\1/p' "${root}/bundle-manifest.toml")"
  [[ "${manifest_files}" =~ ^(0|[1-9][0-9]{0,3})$ \
    && "${manifest_bytes}" =~ ^(0|[1-9][0-9]{0,10})$ \
    && "${manifest_files}" -eq "${actual_files}" \
    && "${manifest_bytes}" -eq "${actual_bytes}" ]] || {
    printf '%s\n' 'Leserpent bundle manifest does not match its payload' >&2
    return 1
  }
  (cd "${root}" && sha256sum --strict --check SHA256SUMS >/dev/null)
}

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

begin_service_unit_transaction() {
  [[ "${unit_transaction_started}" == false ]] || return 1
  for temporary in "${unit_pending}" "${unit_backup}"; do
    [[ ! -e "${temporary}" && ! -L "${temporary}" ]] || {
      printf 'refusing stale Leserpent unit transaction file: %s\n' "${temporary}" >&2
      return 1
    }
  done
  if [[ -e "${unit_path}" || -L "${unit_path}" ]]; then
    [[ -f "${unit_path}" && ! -L "${unit_path}" ]] || {
      printf 'refusing unsafe Leserpent systemd unit: %s\n' "${unit_path}" >&2
      return 1
    }
    unit_existed=true
  fi
  unit_transaction_started=true
  if [[ "${unit_existed}" == true ]]; then
    cp --preserve=mode,timestamps --no-preserve=ownership -- \
      "${unit_path}" "${unit_backup}"
  fi
}

install_service_unit() {
  local source="$1"
  [[ "${unit_transaction_started}" == true ]] || return 1
  [[ -f "${source}" && ! -L "${source}" ]] || {
    printf 'invalid Leserpent systemd unit source: %s\n' "${source}" >&2
    return 1
  }
  install -m 0644 -- "${source}" "${unit_pending}"
  mv -Tf -- "${unit_pending}" "${unit_path}"
  unit_touched=true
}

restore_service_unit() {
  [[ "${unit_transaction_started}" == true ]] || return 0
  rm -f -- "${unit_pending}"
  if [[ "${unit_existed}" == true ]]; then
    mv -Tf -- "${unit_backup}" "${unit_path}"
  else
    rm -f -- "${unit_path}" "${unit_backup}"
  fi
  unit_transaction_started=false
  unit_touched=false
}

commit_service_unit() {
  rm -f -- "${unit_pending}" "${unit_backup}"
  unit_transaction_started=false
  unit_touched=false
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

if [[ -z "${DESTDIR}" ]]; then
  command -v flock >/dev/null 2>&1 || {
    printf '%s\n' 'flock is required for serialized Leserpent installation' >&2
    exit 1
  }
  exec 9>/run/lock/leserpent-install.lock
  flock -w 120 9 || {
    printf '%s\n' 'timed out waiting for another Leserpent installation' >&2
    exit 1
  }
fi

verify_bundle "${SOURCE_DIR}"

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
  verify_bundle "${prefix}/${current_target}"
  verify_bundle "${prefix}/${rollback_target}"

  rollback_committed=false
  rollback_links_switched=false
  rollback_service_touched=false
  cleanup_failed_rollback() {
    local status=$?
    trap - EXIT
    if [[ "${rollback_committed}" != true ]]; then
      set +e
      if [[ "${rollback_links_switched}" == true ]]; then
        replace_release_link previous "${rollback_target}"
        replace_release_link current "${current_target}"
      fi
      restore_service_unit
      if [[ -z "${DESTDIR}" ]]; then
        systemctl daemon-reload
        if [[ "${rollback_service_touched}" == true ]]; then
          systemctl restart leserpent.service
        fi
      fi
    fi
    exit "${status}"
  }
  trap cleanup_failed_rollback EXIT

  begin_service_unit_transaction
  rollback_links_switched=true
  replace_release_link previous "${current_target}"
  replace_release_link current "${rollback_target}"
  install_service_unit "${prefix}/${rollback_target}/deploy/leserpent.service"
  if [[ -n "${DESTDIR}" ]]; then
    commit_service_unit
    rollback_committed=true
    trap - EXIT
    printf 'rolled back staged Leserpent from %s to %s\n' "${current_target}" "${rollback_target}"
    exit 0
  fi
  systemctl daemon-reload
  rollback_service_touched=true
  systemctl restart leserpent.service
  if ! wait_until_healthy; then
    printf '%s\n' 'Leserpent rollback health check failed; restored original release' >&2
    exit 1
  fi
  commit_service_unit
  rollback_committed=true
  trap - EXIT
  printf 'Leserpent rollback is healthy: %s\n' "${rollback_target}"
  exit 0
fi

release_hash="$(sha256sum "${SOURCE_DIR}/SHA256SUMS" | cut -c1-20)"
release_id="$(date -u +%Y%m%d%H%M%S%N)-${release_hash}"
release_dir="${prefix}/releases/${release_id}"
previous_target=""
release_link_switched=false
release_committed=false
service_touched=false

cleanup_uncommitted_release() {
  local status=$?
  local unit_was_touched="${unit_touched}"
  trap - EXIT
  if [[ "${release_committed}" != true ]]; then
    set +e
    if [[ "${release_link_switched}" == true ]]; then
      if [[ -n "${previous_target}" ]]; then
        replace_release_link current "${previous_target}"
      else
        rm -f "${prefix}/current"
      fi
    fi
    restore_service_unit
    if [[ -z "${DESTDIR}" && "${unit_was_touched}" == true ]]; then
      systemctl daemon-reload
    fi
    if [[ "${service_touched}" == true ]]; then
      if [[ -n "${previous_target}" ]]; then
        systemctl restart leserpent.service
      else
        systemctl stop leserpent.service
      fi
    fi
    rm -rf "${release_dir}"
  fi
  exit "${status}"
}
trap cleanup_uncommitted_release EXIT

[[ ! -e "${release_dir}" && ! -L "${release_dir}" ]] || {
  printf 'refusing existing Leserpent release directory: %s\n' "${release_dir}" >&2
  exit 1
}

if [[ -e "${prefix}/current" || -L "${prefix}/current" ]]; then
  previous_target="$(release_link_target current)" || {
    printf '%s\n' 'cannot upgrade: current release link is missing or unsafe' >&2
    exit 1
  }
fi

install -d -m 0755 "${prefix}/releases" "${release_dir}" "${config_dir}" "${unit_dir}"
cp -a --no-preserve=ownership "${SOURCE_DIR}/." "${release_dir}/"
verify_bundle "${release_dir}"
find "${release_dir}" -type d -exec chmod 0755 {} +
find "${release_dir}" -type f -exec chmod 0644 {} +
chmod 0755 "${release_dir}/Leserpent"
chmod 0755 "${release_dir}/leserpent-compat-bridge"
chmod 0755 "${release_dir}/leserpentd"
chmod 0755 "${release_dir}/deploy/install.sh"

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
begin_service_unit_transaction
replace_release_link current "releases/${release_id}"
release_link_switched=true
install_service_unit "${release_dir}/deploy/leserpent.service"

if [[ -n "${DESTDIR}" ]]; then
  if [[ -n "${previous_target}" ]]; then
    replace_release_link previous "${previous_target}"
  fi
  commit_service_unit
  release_committed=true
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
  commit_service_unit
  release_committed=true
  printf 'installed Leserpent %s; systemd was not changed\n' "${release_id}"
  exit 0
fi

service_touched=true
systemctl daemon-reload
systemctl enable leserpent.service >/dev/null
systemctl restart leserpent.service


if ! wait_until_healthy; then
  printf '%s\n' 'Leserpent health check failed; restoring the previous release' >&2
  systemctl --no-pager --full status leserpent.service >&2 || true
  exit 1
fi

if [[ -n "${previous_target}" ]]; then
  replace_release_link previous "${previous_target}"
fi
commit_service_unit
release_committed=true

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
