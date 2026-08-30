#!/usr/bin/env bash
set -euo pipefail
export LC_ALL=C

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
for required in Leserpent leserpent-compat-bridge leserpentd libe_sqlite3.so \
  deploy/install.sh bundle-manifest.toml SHA256SUMS; do
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
file "${BUNDLE}/leserpentd" | grep -q 'ELF 64-bit.*x86-64'

tampered_source="${work}/tampered-source"
cp -a "${BUNDLE}" "${tampered_source}"
printf '\n<!-- tampered -->\n' >>"${tampered_source}/wwwroot/index.html"
if DESTDIR="${work}/tampered-root" "${tampered_source}/deploy/install.sh" \
  --source "${tampered_source}" >"${work}/tampered.log" 2>&1; then
  echo "installer accepted a tampered bundle" >&2
  exit 1
fi
grep -Eq 'manifest does not match|checksum.*does not match|FAILED' "${work}/tampered.log"
[[ ! -e "${work}/tampered-root/opt/leserpent/current" ]]

mkdir -p "${work}/unsafe-root/opt/leserpent"
ln -s ../../outside "${work}/unsafe-root/opt/leserpent/current"
if DESTDIR="${work}/unsafe-root" "${BUNDLE}/deploy/install.sh" --source "${BUNDLE}" >"${work}/unsafe.log" 2>&1; then
  echo "installer accepted an unsafe existing current link" >&2
  exit 1
fi
grep -q 'cannot upgrade: current release link is missing or unsafe' "${work}/unsafe.log"

DESTDIR="${work}/root" "${BUNDLE}/deploy/install.sh" --source "${BUNDLE}" >"${work}/install.log"
current="$(readlink -f "${work}/root/opt/leserpent/current")"
original_current="${current}"
case "${current}" in
  "${work}/root/opt/leserpent/releases/"*) ;;
  *) echo "staged current link escapes the release root" >&2; exit 1 ;;
esac
[[ "$(stat -c '%a' "${work}/root/etc/leserpent/leserpent.env")" == "600" ]]
[[ -x "${current}/Leserpent" && -x "${current}/leserpent-compat-bridge" ]]
[[ -x "${current}/leserpentd" && -x "${current}/deploy/install.sh" ]]
(cd "${current}" && sha256sum --strict --check SHA256SUMS >/dev/null)
cmp -s "${BUNDLE}/deploy/leserpent.service" \
  "${work}/root/etc/systemd/system/leserpent.service"
bundle_identity="$(sha256sum "${BUNDLE}/SHA256SUMS" | cut -c1-20)"
[[ "$(basename "${current}")" == *-"${bundle_identity}" ]]
if find "${current}" -type f -perm /022 -print -quit | grep -q .; then
  echo "staged release contains group/world-writable files" >&2
  exit 1
fi

printf 'state-preserved\n' >"${work}/root/var/lib/leserpent/rollback-sentinel"
config_before="$(sha256sum "${work}/root/etc/leserpent/leserpent.env" | cut -d' ' -f1)"
upgrade_source="${work}/upgrade-source"
cp -a "${BUNDLE}" "${upgrade_source}"
sed -i 's/^Description=Leserpent/Description=leserpent/' \
  "${upgrade_source}/deploy/leserpent.service"
(
  cd "${upgrade_source}"
  find . -type f ! -name SHA256SUMS -printf '%P\n' | sort |
    while IFS= read -r path; do
      sha256sum -- "${path}"
    done
) >"${work}/upgrade.SHA256SUMS"
mv "${work}/upgrade.SHA256SUMS" "${upgrade_source}/SHA256SUMS"
upgrade_identity="$(sha256sum "${upgrade_source}/SHA256SUMS" | cut -c1-20)"
[[ "${upgrade_identity}" != "${bundle_identity}" ]]
release_count_before="$(find "${work}/root/opt/leserpent/releases" -mindepth 1 -maxdepth 1 -type d | wc -l)"
mkdir "${work}/root/opt/leserpent/previous"
if DESTDIR="${work}/root" "${upgrade_source}/deploy/install.sh" \
  --source "${upgrade_source}" >"${work}/failed-upgrade.log" 2>&1; then
  echo "installer accepted an obstructed previous-release link" >&2
  exit 1
fi
[[ "$(readlink -f "${work}/root/opt/leserpent/current")" == "${original_current}" ]]
cmp -s "${BUNDLE}/deploy/leserpent.service" \
  "${work}/root/etc/systemd/system/leserpent.service"
[[ "$(find "${work}/root/opt/leserpent/releases" -mindepth 1 -maxdepth 1 -type d | wc -l)" \
  -eq "${release_count_before}" ]]
rmdir "${work}/root/opt/leserpent/previous"

DESTDIR="${work}/root" "${upgrade_source}/deploy/install.sh" --source "${upgrade_source}" >"${work}/upgrade.log"
upgraded_current="$(readlink -f "${work}/root/opt/leserpent/current")"
[[ "${upgraded_current}" != "${original_current}" ]]
[[ "$(basename "${upgraded_current}")" == *-"${upgrade_identity}" ]]
(
  cd "${upgraded_current}"
  sha256sum --strict --check SHA256SUMS >/dev/null
)
[[ "$(readlink -f "${work}/root/opt/leserpent/previous")" == "${original_current}" ]]
[[ "$(sha256sum "${work}/root/etc/leserpent/leserpent.env" | cut -d' ' -f1)" == "${config_before}" ]]
[[ "$(cat "${work}/root/var/lib/leserpent/rollback-sentinel")" == "state-preserved" ]]
cmp -s "${upgrade_source}/deploy/leserpent.service" \
  "${work}/root/etc/systemd/system/leserpent.service"

DESTDIR="${work}/root" "${BUNDLE}/deploy/install.sh" --source "${BUNDLE}" --rollback >"${work}/rollback.log"
current="$(readlink -f "${work}/root/opt/leserpent/current")"
[[ "${current}" == "${original_current}" ]]
[[ "$(readlink -f "${work}/root/opt/leserpent/previous")" == "${upgraded_current}" ]]
[[ "$(sha256sum "${work}/root/etc/leserpent/leserpent.env" | cut -d' ' -f1)" == "${config_before}" ]]
[[ "$(cat "${work}/root/var/lib/leserpent/rollback-sentinel")" == "state-preserved" ]]
cmp -s "${BUNDLE}/deploy/leserpent.service" \
  "${work}/root/etc/systemd/system/leserpent.service"

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
  "bundle_identity": "$(sha256sum "${BUNDLE}/SHA256SUMS" | cut -d' ' -f1)",
  "leserpent_sha256": "$(sha256sum "${BUNDLE}/Leserpent" | cut -d' ' -f1)",
  "leserpent_bytes": $(stat -c '%s' "${BUNDLE}/Leserpent"),
  "compat_bridge_sha256": "$(sha256sum "${BUNDLE}/leserpent-compat-bridge" | cut -d' ' -f1)",
  "compat_bridge_bytes": $(stat -c '%s' "${BUNDLE}/leserpent-compat-bridge"),
  "daemon_sha256": "$(sha256sum "${BUNDLE}/leserpentd" | cut -d' ' -f1)",
  "daemon_bytes": $(stat -c '%s' "${BUNDLE}/leserpentd"),
  "checks": [
    "native-aot-elf-x86_64",
    "rust-compat-bridge-elf-x86_64",
    "rust-daemon-elf-x86_64",
    "tampered-bundle-rejection-before-mutation",
    "exact-bundle-inventory",
    "installed-bundle-checksum-verification",
    "content-addressed-release-identity",
    "unsafe-existing-current-link-rejection",
    "staged-atomic-current-link",
    "staged-upgrade-current-previous-link",
    "configuration-preserved-across-upgrade-rollback",
    "state-preserved-across-upgrade-rollback",
    "transactional-systemd-unit-upgrade-rollback",
    "failed-upgrade-release-and-unit-restoration",
    "explicit-atomic-rollback",
    "rolled-back-live-native-aot-health",
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
