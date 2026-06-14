#!/usr/bin/env bash
set -euo pipefail

API_ADDR="${1:-127.0.0.1:9910}"
OUT_DIR="${2:-/tmp/gewyvern-training-dataset-demo}"
TARGET_PATH_SEGMENT="${3:-}"
LIMIT="${4:-0}"

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "missing required command: $1" >&2
    exit 1
  fi
}

require_cmd curl
require_cmd python3

mkdir -p "${OUT_DIR}"

if [ -n "${TARGET_PATH_SEGMENT}" ]; then
  MANIFEST_ROUTE="/v1/latest/targets/${TARGET_PATH_SEGMENT}/training-dataset.json"
else
  MANIFEST_ROUTE="/v1/latest/training-dataset.json"
fi

MANIFEST_URL="http://${API_ADDR}${MANIFEST_ROUTE}"
MANIFEST_PATH="${OUT_DIR}/training-dataset.json"

wait_for_manifest() {
  local url="$1"
  local out="$2"
  for _ in $(seq 1 120); do
    if curl -fsS "${url}" > "${out}" 2>/dev/null; then
      return 0
    fi
    sleep 0.1
  done
  return 1
}

if ! wait_for_manifest "${MANIFEST_URL}" "${MANIFEST_PATH}"; then
  echo "training dataset manifest did not become ready at ${MANIFEST_URL}" >&2
  exit 1
fi

python3 - "${MANIFEST_PATH}" "${OUT_DIR}" "${API_ADDR}" "${LIMIT}" <<'PY'
import json
import pathlib
import sys
import urllib.request

manifest_path = pathlib.Path(sys.argv[1])
out_dir = pathlib.Path(sys.argv[2])
api_addr = sys.argv[3]
limit = int(sys.argv[4])

with manifest_path.open("r", encoding="utf-8") as fh:
    manifest = json.load(fh)

samples = manifest.get("samples", [])
if limit > 0:
    samples = samples[:limit]

if not samples:
    print("no samples found in manifest", file=sys.stderr)
    sys.exit(1)

def fetch_json(url: str):
    with urllib.request.urlopen(url) as response:
        return json.loads(response.read().decode("utf-8"))

checked = 0
for index, sample in enumerate(samples):
    name = sample["name"]
    manifest_sample_id = sample["sample_id"]
    sample_path = sample["sample_path"]
    sample_url = f"http://{api_addr}{sample_path}"
    payload = fetch_json(sample_url)
    payload_sample_id = payload.get("sample_id")
    if payload_sample_id != manifest_sample_id:
        print(
            f"sample_id mismatch for {name}: manifest={manifest_sample_id} payload={payload_sample_id}",
            file=sys.stderr,
        )
        sys.exit(2)
    sample_file = out_dir / f"sample-{index:03d}.json"
    with sample_file.open("w", encoding="utf-8") as fh:
        json.dump(payload, fh, ensure_ascii=True, indent=2)
        fh.write("\n")
    checked += 1

summary = {
    "manifest_path": str(manifest_path),
    "sample_count_checked": checked,
    "default_split_policy": manifest.get("split_policies", {}).get("default"),
    "sample_ids_verified": True,
    "output_dir": str(out_dir),
}

summary_path = out_dir / "roundtrip-summary.json"
with summary_path.open("w", encoding="utf-8") as fh:
    json.dump(summary, fh, ensure_ascii=True, indent=2)
    fh.write("\n")

print(json.dumps(summary, ensure_ascii=True))
PY
