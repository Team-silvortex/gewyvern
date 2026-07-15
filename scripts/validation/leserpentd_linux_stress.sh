#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
source "${ROOT}/scripts/remote/container_execution.sh"
gewy_container_maybe_run_remote "$@"

OUT_DIR="${1:-${ROOT}/target/validation/leserpentd-linux-stress}"
case "${OUT_DIR}" in
  "${ROOT}"/target/validation/*) ;;
  *)
    echo "output directory must stay under target/validation" >&2
    exit 2
    ;;
esac

mkdir -p "${OUT_DIR}"
STAMP="$(date -u +%Y%m%dT%H%M%SZ)"
TEMP="${OUT_DIR}/.${STAMP}.$$.tmp"
RUN="${OUT_DIR}/${STAMP}.json"
trap 'rm -f "${TEMP}"' EXIT

cd "${ROOT}"
cargo run --quiet -p leserpentd --example linux_stress >"${TEMP}"
mv "${TEMP}" "${RUN}"
cp "${RUN}" "${OUT_DIR}/latest.json"
printf 'leserpentd Linux stress evidence: %s\n' "${RUN}"
