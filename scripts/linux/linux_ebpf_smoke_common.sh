#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

linux_smoke_include_flags() {
  printf '%s\n' \
    "-I/usr/include" \
    "-I/usr/include/$(uname -m)-linux-gnu"
}

compile_bpf_smoke_object() {
  local source_file="$1"
  local output_file="$2"
  mapfile -t include_flags < <(linux_smoke_include_flags)
  clang \
    -O2 \
    -g \
    -target bpf \
    "${include_flags[@]}" \
    -c "${ROOT}/${source_file}" \
    -o "${output_file}"
}

compile_linux_smoke_loader() {
  local source_file="$1"
  local output_file="$2"
  cc \
    -O2 \
    -g \
    "${ROOT}/${source_file}" \
    -lbpf \
    -lelf \
    -lz \
    -o "${output_file}"
}
