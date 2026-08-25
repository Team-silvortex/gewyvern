#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
TARGET_DIR="${CARGO_TARGET_DIR:-${ROOT}/target}"
DAEMON_BIN="${LESERPENT_TEST_DAEMON_BIN:-${TARGET_DIR}/debug/leserpentd}"
case "$(uname -s)-$(uname -m)" in
  Darwin-arm64)
    EVIDENCE_NAME="leserpent_registration_recovery_macos_arm64_20260825.json"
    ;;
  Linux-x86_64)
    EVIDENCE_NAME="leserpent_registration_recovery_linux_x86_64_20260825.json"
    ;;
  *)
    EVIDENCE_NAME="leserpent_registration_recovery.json"
    ;;
esac
EVIDENCE_PATH="${LESERPENT_REGISTRATION_RECOVERY_EVIDENCE:-${ROOT}/target/validation/${EVIDENCE_NAME}}"
TEST_PROJECT="${ROOT}/apps/leserpent/tests/Leserpent.SecurityTests/Leserpent.SecurityTests.csproj"

cd "${ROOT}"
cargo build --locked -p leserpentd --bin leserpentd
dotnet restore "${TEST_PROJECT}" --locked-mode
dotnet build "${TEST_PROJECT}" --no-restore

LESERPENT_TEST_DAEMON_BIN="${DAEMON_BIN}" \
LESERPENT_REGISTRATION_RECOVERY_EVIDENCE="${EVIDENCE_PATH}" \
dotnet test "${TEST_PROJECT}" \
  --no-restore \
  --no-build \
  --filter "FullyQualifiedName~RegistrationLostResponsesRecoverAcrossCompatibilityProcessRestart"

test -s "${EVIDENCE_PATH}"
printf 'registration recovery evidence: %s\n' "${EVIDENCE_PATH}"
