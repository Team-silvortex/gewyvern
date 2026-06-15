#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
source "${ROOT}/scripts/linux/linux_ebpf_smoke_common.sh"
TMP_DIR="$(mktemp -d "${TMPDIR:-/tmp}/gewyvern-linux-tc-smoke.XXXXXX")"
DEV_NAME="${1:-eth0}"
trap 'tc qdisc del dev "${DEV_NAME}" clsact >/dev/null 2>&1 || true; rm -rf "${TMP_DIR}"' EXIT

BPF_OBJ="${TMP_DIR}/tc_min.bpf.o"

compile_bpf_smoke_object "ebpf/smoke/tc_min.bpf.c" "${BPF_OBJ}"

tc qdisc add dev "${DEV_NAME}" clsact >/dev/null 2>&1 || true
tc filter replace dev "${DEV_NAME}" ingress bpf da obj "${BPF_OBJ}" sec classifier/tc_ingress

echo "linux tc smoke ok"
