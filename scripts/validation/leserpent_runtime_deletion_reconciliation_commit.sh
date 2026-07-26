#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
DAEMON_TARGET_ROOT="${CARGO_TARGET_DIR:-${ROOT}/target}"
DAEMON_BIN="${LESERPENT_TEST_DAEMON_BIN:-${DAEMON_TARGET_ROOT}/debug/leserpentd}"
case "$(uname -s)-$(uname -m)" in
  Darwin-arm64)
    DEFAULT_EVIDENCE="${ROOT}/docs/fixtures/leserpent_runtime_deletion_reconciliation_commit_20260726.json"
    ;;
  Linux-x86_64)
    DEFAULT_EVIDENCE="${ROOT}/docs/fixtures/leserpent_runtime_deletion_reconciliation_commit_linux_x86_64_20260726.json"
    ;;
  *)
    DEFAULT_EVIDENCE="${ROOT}/target/validation/leserpent_runtime_deletion_reconciliation_commit.json"
    ;;
esac
EVIDENCE_PATH="${LESERPENT_RUNTIME_DELETION_RECONCILIATION_COMMIT_EVIDENCE:-${DEFAULT_EVIDENCE}}"
if [[ "${EVIDENCE_PATH}" != /* ]]; then
  EVIDENCE_PATH="${ROOT}/${EVIDENCE_PATH}"
fi
TEST_PROJECT="${ROOT}/apps/leserpent/tests/Leserpent.SecurityTests/Leserpent.SecurityTests.csproj"

cd "${ROOT}"
cargo build --locked -p leserpentd --bin leserpentd
dotnet restore "${TEST_PROJECT}" --locked-mode
dotnet build "${TEST_PROJECT}" --no-restore

LESERPENT_TEST_DAEMON_BIN="${DAEMON_BIN}" \
LESERPENT_RUNTIME_DELETION_RECONCILIATION_COMMIT_EVIDENCE="${EVIDENCE_PATH}" \
dotnet test "${TEST_PROJECT}" \
  --no-restore \
  --no-build \
  --filter "FullyQualifiedName~RuntimeDeletionReconciliationCommitIsAtomicAcrossHostTermination"

echo "runtime deletion reconciliation commit evidence: ${EVIDENCE_PATH}"
