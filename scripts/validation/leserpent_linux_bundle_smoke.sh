#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
BUNDLE="${1:-${ROOT}/artifacts/leserpent/linux-x64}"
OUT="${2:-${ROOT}/target/validation/leserpent-linux-bundle-smoke/latest.json}"

if [[ "$(uname -s)" != "Linux" || "$(uname -m)" != "x86_64" ]]; then
  echo "Leserpent Linux bundle smoke requires Linux x86_64" >&2
  exit 2
fi
for tool in curl file sha256sum stat; do
  command -v "${tool}" >/dev/null || { echo "missing required tool: ${tool}" >&2; exit 2; }
done
for required in Leserpent leserpent-compat-bridge libe_sqlite3.so deploy/install.sh; do
  [[ -e "${BUNDLE}/${required}" ]] || { echo "bundle is missing ${required}" >&2; exit 1; }
done

work="$(mktemp -d "${TMPDIR:-/tmp}/leserpent-linux-bundle.XXXXXX")"
pid=""
cleanup() {
  if [[ -n "${pid}" ]] && kill -0 "${pid}" 2>/dev/null; then
    kill "${pid}" 2>/dev/null || true
    wait "${pid}" 2>/dev/null || true
  fi
  rm -rf "${work}"
}
trap cleanup EXIT

file "${BUNDLE}/Leserpent" | grep -q 'ELF 64-bit.*x86-64'
file "${BUNDLE}/leserpent-compat-bridge" | grep -q 'ELF 64-bit.*x86-64'

DESTDIR="${work}/root" "${BUNDLE}/deploy/install.sh" --source "${BUNDLE}" >"${work}/install.log"
current="$(readlink -f "${work}/root/opt/leserpent/current")"
case "${current}" in
  "${work}/root/opt/leserpent/releases/"*) ;;
  *) echo "staged current link escapes the release root" >&2; exit 1 ;;
esac
[[ "$(stat -c '%a' "${work}/root/etc/leserpent/leserpent.env")" == "600" ]]
[[ -x "${current}/Leserpent" && -x "${current}/leserpent-compat-bridge" ]]
if find "${current}" -type f -perm /022 -print -quit | grep -q .; then
  echo "staged release contains group/world-writable files" >&2
  exit 1
fi

payload="$(tr -d '\r\n' <"${ROOT}/crates/leserpent-protocol/tests/fixtures/legacy-runtime-list-response-v1.json")"
printf '{"request_id":"linux-smoke","operation":"validate_runtime_list","payload":%s}\n' "${payload}" \
  | "${current}/leserpent-compat-bridge" >"${work}/bridge.json"
grep -q '"request_id":"linux-smoke"' "${work}/bridge.json"
grep -q '"ok":true' "${work}/bridge.json"

port="${LESERPENT_SMOKE_PORT:-35210}"
mkdir -p "${work}/state"
ASPNETCORE_ENVIRONMENT=Production \
ASPNETCORE_URLS="http://127.0.0.1:${port}" \
LESERPENT_STATE_PATH="${work}/state/control-plane.json" \
LESERPENT_DATABASE_PATH="${work}/state/control-plane.db" \
LESERPENT_RUST_BRIDGE_BIN="${current}/leserpent-compat-bridge" \
  "${current}/Leserpent" >"${work}/leserpent.log" 2>&1 &
pid=$!
healthy=false
for _ in {1..60}; do
  if curl --fail --silent "http://127.0.0.1:${port}/health" >"${work}/health.json" 2>/dev/null; then
    healthy=true
    break
  fi
  if ! kill -0 "${pid}" 2>/dev/null; then
    cat "${work}/leserpent.log" >&2
    exit 1
  fi
  sleep 0.25
done
[[ "${healthy}" == true ]] || { cat "${work}/leserpent.log" >&2; exit 1; }

mkdir -p "$(dirname "${OUT}")"
temp="${OUT}.tmp.$$"
cat >"${temp}" <<EOF
{
  "schema_version": 1,
  "proof": "leserpent-linux-bundle-smoke",
  "platform": "linux-x86_64",
  "kernel": "$(uname -r)",
  "leserpent_sha256": "$(sha256sum "${BUNDLE}/Leserpent" | cut -d' ' -f1)",
  "leserpent_bytes": $(stat -c '%s' "${BUNDLE}/Leserpent"),
  "compat_bridge_sha256": "$(sha256sum "${BUNDLE}/leserpent-compat-bridge" | cut -d' ' -f1)",
  "compat_bridge_bytes": $(stat -c '%s' "${BUNDLE}/leserpent-compat-bridge"),
  "checks": [
    "native-aot-elf-x86_64",
    "rust-compat-bridge-elf-x86_64",
    "staged-atomic-current-link",
    "private-generated-configuration",
    "non-writable-release-files",
    "live-compatibility-request",
    "live-native-aot-health"
  ],
  "result": "passed"
}
EOF
mv "${temp}" "${OUT}"
printf 'Leserpent Linux bundle smoke: passed (%s)\n' "${OUT}"
