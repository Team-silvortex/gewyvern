#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TMP_DIR="$(mktemp -d "${TMPDIR:-/tmp}/gewyvern-linux-tc-smoke.XXXXXX")"
DEV_NAME="${1:-eth0}"
trap 'tc qdisc del dev "${DEV_NAME}" clsact >/dev/null 2>&1 || true; rm -rf "${TMP_DIR}"' EXIT

BPF_OBJ="${TMP_DIR}/tc_min.bpf.o"

clang \
  -O2 \
  -g \
  -target bpf \
  -I/usr/include \
  -I/usr/include/$(uname -m)-linux-gnu \
  -c "${ROOT}/ebpf/smoke/tc_min.bpf.c" \
  -o "${BPF_OBJ}"

tc qdisc add dev "${DEV_NAME}" clsact >/dev/null 2>&1 || true
tc filter replace dev "${DEV_NAME}" ingress bpf da obj "${BPF_OBJ}" sec classifier/tc_ingress

echo "linux tc smoke ok"
