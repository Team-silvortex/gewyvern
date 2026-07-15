#!/usr/bin/env bash

# Dispatch container-heavy entrypoints away from developer macOS hosts while
# keeping Linux builders and CI self-contained.
gewy_container_maybe_run_remote() {
  local mode="${GEWY_DOCKER_EXECUTION:-auto}"
  local host_os
  local caller
  local relative
  local root

  for argument in "$@"; do
    if [[ "${argument}" == "-h" || "${argument}" == "--help" ]]; then
      return 0
    fi
  done

  host_os="$(uname -s)"
  if [[ "${mode}" == "auto" ]]; then
    if [[ -z "${CI:-}" && "${host_os}" == "Darwin" ]]; then
      mode="remote"
    else
      mode="local"
    fi
  fi

  case "${mode}" in
    local)
      return 0
      ;;
    remote)
      ;;
    *)
      echo "GEWY_DOCKER_EXECUTION must be auto, local, or remote" >&2
      return 2
      ;;
  esac

  root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
  caller="${BASH_SOURCE[1]}"
  if [[ "${caller}" != /* ]]; then
    caller="$(cd "$(dirname "${caller}")" && pwd)/$(basename "${caller}")"
  fi
  relative="${caller#"${root}/"}"
  if [[ "${relative}" == "${caller}" || "${relative}" == *".."* ]]; then
    echo "container entrypoint must be inside the repository: ${caller}" >&2
    return 2
  fi

  exec "${root}/scripts/remote/run_on_linux_host.sh" -- bash "${relative}" "$@"
}
