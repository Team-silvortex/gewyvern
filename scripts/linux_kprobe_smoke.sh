#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "${ROOT}/scripts/linux_ebpf_smoke_common.sh"
TMP_DIR="$(mktemp -d "${TMPDIR:-/tmp}/gewyvern-linux-kprobe-smoke.XXXXXX")"
SYMBOL_NAME="${1:-ip_route_output_flow}"
trap 'rm -rf "${TMP_DIR}"' EXIT

BPF_OBJ="${TMP_DIR}/kprobe_min.bpf.o"
LOADER_BIN="${TMP_DIR}/attach_kprobe_smoke"

compile_bpf_smoke_object "ebpf/smoke/kprobe_min.bpf.c" "${BPF_OBJ}"
compile_linux_smoke_loader "ebpf/smoke/attach_kprobe_smoke.c" "${LOADER_BIN}"

"${LOADER_BIN}" "${BPF_OBJ}" "${SYMBOL_NAME}"
