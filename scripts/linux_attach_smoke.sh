#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "${ROOT}/scripts/linux_ebpf_smoke_common.sh"
TMP_DIR="$(mktemp -d "${TMPDIR:-/tmp}/gewyvern-linux-attach-smoke.XXXXXX")"
HOOKPOINT_NAME="${1:-syscalls/sys_enter_nanosleep}"
HOOKPOINT_CATEGORY="${HOOKPOINT_NAME%%/*}"
HOOKPOINT_EVENT="${HOOKPOINT_NAME#*/}"
trap 'rm -rf "${TMP_DIR}"' EXIT

BPF_OBJ="${TMP_DIR}/tracepoint_min.bpf.o"
LOADER_BIN="${TMP_DIR}/attach_smoke"

compile_bpf_smoke_object "ebpf/smoke/tracepoint_min.bpf.c" "${BPF_OBJ}"
compile_linux_smoke_loader "ebpf/smoke/attach_smoke.c" "${LOADER_BIN}"

"${LOADER_BIN}" "${BPF_OBJ}" "${HOOKPOINT_CATEGORY}" "${HOOKPOINT_EVENT}"
