#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
DAEMON_BIN="${LESERPENT_TEST_DAEMON_BIN:-${ROOT}/target/debug/leserpentd}"
ITERATIONS="${LESERPENT_RUNTIME_DELETION_FAULT_ITERATIONS:-3}"
case "$(uname -s)-$(uname -m)" in
  Darwin-arm64)
    DEFAULT_EVIDENCE="${ROOT}/docs/fixtures/leserpent_runtime_deletion_fault_campaign_20260723.json"
    ;;
  Linux-x86_64)
    DEFAULT_EVIDENCE="${ROOT}/docs/fixtures/leserpent_runtime_deletion_fault_campaign_linux_x86_64_20260723.json"
    ;;
  *)
    DEFAULT_EVIDENCE="${ROOT}/target/validation/leserpent_runtime_deletion_fault_campaign.json"
    ;;
esac
EVIDENCE_PATH="${LESERPENT_RUNTIME_DELETION_CAMPAIGN_EVIDENCE:-${DEFAULT_EVIDENCE}}"
TEST_PROJECT="${ROOT}/apps/leserpent/tests/Leserpent.SecurityTests/Leserpent.SecurityTests.csproj"

cd "${ROOT}"
cargo build --locked -p leserpentd --bin leserpentd
dotnet restore "${TEST_PROJECT}" --locked-mode
dotnet build "${TEST_PROJECT}" --no-restore

LESERPENT_TEST_DAEMON_BIN="${DAEMON_BIN}" \
LESERPENT_RUNTIME_DELETION_FAULT_ITERATIONS="${ITERATIONS}" \
LESERPENT_RUNTIME_DELETION_CAMPAIGN_EVIDENCE="${EVIDENCE_PATH}" \
dotnet test "${TEST_PROJECT}" \
  --no-restore \
  --no-build \
  --filter "FullyQualifiedName~RuntimeDeletionFaultCampaignRecoversAcrossEveryDurableTransition"

echo "runtime deletion fault campaign evidence: ${EVIDENCE_PATH}"
