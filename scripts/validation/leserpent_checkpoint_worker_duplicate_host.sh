#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
EVIDENCE_PATH="${LESERPENT_CHECKPOINT_WORKER_DUPLICATE_HOST_EVIDENCE:-${ROOT}/target/validation/leserpent_checkpoint_worker_duplicate_host.json}"
if [[ "${EVIDENCE_PATH}" != /* ]]; then
  EVIDENCE_PATH="${ROOT}/${EVIDENCE_PATH}"
fi
TEST_PROJECT="${ROOT}/apps/leserpent/tests/Leserpent.SecurityTests/Leserpent.SecurityTests.csproj"

mkdir -p "$(dirname "${EVIDENCE_PATH}")"
cd "${ROOT}"
dotnet restore "${TEST_PROJECT}" --locked-mode
dotnet build "${TEST_PROJECT}" --no-restore

LESERPENT_CHECKPOINT_WORKER_DUPLICATE_HOST_EVIDENCE="${EVIDENCE_PATH}" \
dotnet test "${TEST_PROJECT}" \
  --no-restore \
  --no-build \
  --filter "FullyQualifiedName~RealDuplicateHostsExposeOneOwnerAndFreshProcessTakeover"

echo "checkpoint worker duplicate-host evidence: ${EVIDENCE_PATH}"
