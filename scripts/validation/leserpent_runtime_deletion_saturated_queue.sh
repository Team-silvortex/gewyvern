#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
case "$(uname -s)-$(uname -m)" in
  Darwin-arm64)
    DEFAULT_EVIDENCE="${ROOT}/docs/fixtures/leserpent_runtime_deletion_saturated_queue_20260723.json"
    ;;
  Linux-x86_64)
    DEFAULT_EVIDENCE="${ROOT}/docs/fixtures/leserpent_runtime_deletion_saturated_queue_linux_x86_64_20260723.json"
    ;;
  *)
    DEFAULT_EVIDENCE="${ROOT}/target/validation/leserpent_runtime_deletion_saturated_queue.json"
    ;;
esac
EVIDENCE_PATH="${LESERPENT_RUNTIME_DELETION_SATURATED_QUEUE_EVIDENCE:-${DEFAULT_EVIDENCE}}"
TEST_PROJECT="${ROOT}/apps/leserpent/tests/Leserpent.SecurityTests/Leserpent.SecurityTests.csproj"

cd "${ROOT}"
dotnet restore "${TEST_PROJECT}" --locked-mode
dotnet build "${TEST_PROJECT}" --no-restore

LESERPENT_RUNTIME_DELETION_SATURATED_QUEUE_EVIDENCE="${EVIDENCE_PATH}" \
dotnet test "${TEST_PROJECT}" \
  --no-restore \
  --no-build \
  --filter "FullyQualifiedName~SaturatedRuntimeDeletionQueueIsFairAndStopsCooperatively"

echo "runtime deletion saturated-queue evidence: ${EVIDENCE_PATH}"
