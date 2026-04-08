#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TMP_DIR="$(mktemp -d "${TMPDIR:-/tmp}/gewyvern-linux-kprobe-smoke.XXXXXX")"
SYMBOL_NAME="${1:-ip_route_output_flow}"
trap 'rm -rf "${TMP_DIR}"' EXIT

BPF_OBJ="${TMP_DIR}/kprobe_min.bpf.o"
LOADER_BIN="${TMP_DIR}/attach_kprobe_smoke"

clang \
  -O2 \
  -g \
  -target bpf \
  -I/usr/include \
  -I/usr/include/$(uname -m)-linux-gnu \
  -c "${ROOT}/ebpf/smoke/kprobe_min.bpf.c" \
  -o "${BPF_OBJ}"

cc \
  -O2 \
  -g \
  "${ROOT}/ebpf/smoke/attach_kprobe_smoke.c" \
  -lbpf \
  -lelf \
  -lz \
  -o "${LOADER_BIN}"

"${LOADER_BIN}" "${BPF_OBJ}" "${SYMBOL_NAME}"
