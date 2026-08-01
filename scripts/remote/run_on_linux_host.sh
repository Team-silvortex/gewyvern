#!/usr/bin/env bash

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
HOST="${GEWY_REMOTE_HOST:-gewyvern-lab}"
SYNC_BACK=1
TTY=0

usage() {
  cat <<'EOF'
Usage: scripts/remote/run_on_linux_host.sh [--host <ssh-host>] [--no-sync-back] [--tty] -- <command> [args...]

Incrementally sync the repository to the trusted Linux validation host, run a
command there, and copy validation/package artifacts back. Credentials are
resolved by SSH and are never accepted as command arguments.
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --host)
      [[ $# -ge 2 ]] || { echo "--host requires a value" >&2; exit 2; }
      HOST="$2"
      shift 2
      ;;
    --no-sync-back)
      SYNC_BACK=0
      shift
      ;;
    --tty)
      TTY=1
      shift
      ;;
    --)
      shift
      break
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "unknown option: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

[[ $# -gt 0 ]] || { echo "remote command is required" >&2; exit 2; }
[[ "${HOST}" != -* && "${HOST}" =~ ^[A-Za-z0-9_.@-]+$ ]] || {
  echo "unsafe SSH host: ${HOST}" >&2
  exit 2
}

for required in ssh rsync; do
  command -v "${required}" >/dev/null 2>&1 || {
    echo "${required} is required for remote container execution" >&2
    exit 1
  }
done

SSH_ARGS=(-o BatchMode=yes -o ConnectTimeout=15)
REMOTE_HOME="$(ssh "${SSH_ARGS[@]}" "${HOST}" 'printf %s "$HOME"')"
[[ "${REMOTE_HOME}" == /* && "${REMOTE_HOME}" =~ ^[A-Za-z0-9_./-]+$ && "${REMOTE_HOME}" != *".."* ]] || {
  echo "remote host returned an unsafe home directory" >&2
  exit 1
}

REMOTE_WORKSPACE="${GEWY_REMOTE_DOCKER_WORKSPACE:-${REMOTE_HOME}/.cache/gewyvern/docker-workspace}"
REMOTE_CACHE_ROOT="${REMOTE_HOME}/.cache/gewyvern"
[[ "${REMOTE_WORKSPACE}" == "${REMOTE_CACHE_ROOT}/"* && "${REMOTE_WORKSPACE}" =~ ^[A-Za-z0-9_./-]+$ && "${REMOTE_WORKSPACE}" != *".."* ]] || {
  echo "GEWY_REMOTE_DOCKER_WORKSPACE must stay under ${REMOTE_CACHE_ROOT}" >&2
  exit 2
}

printf '[remote-docker] host=%s workspace=%s\n' "${HOST}" "${REMOTE_WORKSPACE}"
ssh "${SSH_ARGS[@]}" "${HOST}" "mkdir -p '${REMOTE_WORKSPACE}' '${REMOTE_CACHE_ROOT}/docker-target'"

rsync -az --delete --delete-excluded \
  --exclude='.git/' \
  --exclude='target/' \
  --exclude='node_modules/' \
  --exclude='**/bin/' \
  --exclude='**/obj/' \
  --exclude='**/TestResults/' \
  --exclude='**/__pycache__/' \
  --exclude='apps/leserpent/src/Leserpent/.leserpent-state/' \
  --exclude='apps/leserpent/src/Leserpent/data/control-plane-state.json*' \
  -e "ssh -o BatchMode=yes -o ConnectTimeout=15" \
  "${ROOT}/" "${HOST}:${REMOTE_WORKSPACE}/"

shell_quote() {
  printf '%q' "$1"
}

REMOTE_COMMAND="cd $(shell_quote "${REMOTE_WORKSPACE}") && flock -o -w 120 $(shell_quote "${REMOTE_CACHE_ROOT}/docker-workspace.lock") env GEWY_DOCKER_EXECUTION=local CARGO_TARGET_DIR=$(shell_quote "${REMOTE_CACHE_ROOT}/docker-target")"
for name in \
  GEWY_CONTAINER_VALIDATION_TIMEOUT_SECONDS \
  GEWY_DEB_APT_MIRROR GEWY_RPM_DNF_MIRROR \
  GEWY_DOCKER_IMAGE_TAG DOCKER_BASE_IMAGE DOCKER_APT_MIRROR \
  DOCKER_RUSTUP_INIT_URL DOCKER_RUSTUP_INIT_FALLBACK_URL \
  DOCKER_RUSTUP_DIST_SERVER DOCKER_RUSTUP_UPDATE_ROOT \
  DOCKER_RUSTUP_INSTALL_TIMEOUT_SECONDS CARGO_NET_OFFLINE \
  JUICE_SHOP_IMAGE FTP_DENIED_IMAGE LDAP_BIND_DENIED_IMAGE; do
  if [[ -n "${!name:-}" ]]; then
    REMOTE_COMMAND+=" ${name}=$(shell_quote "${!name}")"
  fi
done
for argument in "$@"; do
  REMOTE_COMMAND+=" $(shell_quote "${argument}")"
done

set +e
if [[ "${TTY}" -eq 1 ]]; then
  ssh -tt "${SSH_ARGS[@]}" "${HOST}" "${REMOTE_COMMAND}"
else
  ssh "${SSH_ARGS[@]}" "${HOST}" "${REMOTE_COMMAND}"
fi
STATUS=$?
set -e

if [[ "${SYNC_BACK}" -eq 1 ]]; then
  mkdir -p "${ROOT}/target/validation" "${ROOT}/target/packages"
  if ssh "${SSH_ARGS[@]}" "${HOST}" "test -d '${REMOTE_WORKSPACE}/target/validation'"; then
    rsync -az -e "ssh -o BatchMode=yes -o ConnectTimeout=15" \
      "${HOST}:${REMOTE_WORKSPACE}/target/validation/" "${ROOT}/target/validation/"
  fi
  if ssh "${SSH_ARGS[@]}" "${HOST}" "test -d '${REMOTE_WORKSPACE}/target/packages'"; then
    rsync -az -e "ssh -o BatchMode=yes -o ConnectTimeout=15" \
      "${HOST}:${REMOTE_WORKSPACE}/target/packages/" "${ROOT}/target/packages/"
  fi
fi

exit "${STATUS}"
