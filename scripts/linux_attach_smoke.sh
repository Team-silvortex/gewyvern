#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TMP_DIR="$(mktemp -d "${TMPDIR:-/tmp}/gewyvern-linux-attach-smoke.XXXXXX")"
HOOKPOINT_NAME="${1:-syscalls/sys_enter_nanosleep}"
HOOKPOINT_CATEGORY="${HOOKPOINT_NAME%%/*}"
HOOKPOINT_EVENT="${HOOKPOINT_NAME#*/}"
trap 'rm -rf "${TMP_DIR}"' EXIT

BPF_OBJ="${TMP_DIR}/tracepoint_min.bpf.o"
LOADER_BIN="${TMP_DIR}/attach_smoke"

clang \
  -O2 \
  -g \
  -target bpf \
  -I/usr/include \
  -I/usr/include/$(uname -m)-linux-gnu \
  -c "${ROOT}/ebpf/smoke/tracepoint_min.bpf.c" \
  -o "${BPF_OBJ}"

cc \
  -O2 \
  -g \
  "${ROOT}/ebpf/smoke/attach_smoke.c" \
  -lbpf \
  -lelf \
  -lz \
  -o "${LOADER_BIN}"

"${LOADER_BIN}" "${BPF_OBJ}" "${HOOKPOINT_CATEGORY}" "${HOOKPOINT_EVENT}"
