use std::fs;
use std::path::{Path, PathBuf};

use serde_json::json;

use super::command::{ValidationError, ValidationReport, default_out_dir, run_cargo_status};

struct ProofSuite {
    id: &'static str,
    package: &'static str,
    target_args: &'static [&'static str],
    expected_min_tests: usize,
    invariants: &'static [&'static str],
}

const PROOF_SUITES: &[ProofSuite] = &[
    ProofSuite {
        id: "command-origin-lowering",
        package: "leselang-command",
        target_args: &["--lib"],
        expected_min_tests: 8,
        invariants: &[
            "frontend-neutral-command-semantics",
            "origin-preservation",
            "capability-fail-closed",
            "bounded-plan-codec",
        ],
    },
    ProofSuite {
        id: "domain-authorization-idempotency",
        package: "leserpent-domain",
        target_args: &["--lib"],
        expected_min_tests: 10,
        invariants: &[
            "capability-gated-query",
            "confirmation-and-dry-run-semantics",
            "principal-scoped-idempotency",
            "revision-conflict-replay",
            "snapshot-idempotency-restore",
        ],
    },
    ProofSuite {
        id: "debugger-confirmation-boundary",
        package: "leselang-observe",
        target_args: &["--lib", "debugger_cancel_"],
        expected_min_tests: 3,
        invariants: &[
            "debugger-capability-gate",
            "explicit-confirmation",
            "non-mutating-dry-run",
            "stale-session-rejection",
        ],
    },
    ProofSuite {
        id: "cli-leselang-origin-parity",
        package: "leserpent-cli",
        target_args: &["--test", "leselang_parity"],
        expected_min_tests: 3,
        invariants: &[
            "refresh-command-equivalence",
            "inspect-query-equivalence",
            "history-query-equivalence",
        ],
    },
    ProofSuite {
        id: "vm-reentry-recovery",
        package: "leselang-vm",
        target_args: &["--lib"],
        expected_min_tests: 65,
        invariants: &[
            "continuation-process-restart",
            "journal-pending-recovery",
            "completion-idempotency",
            "dispatch-lease-fencing",
            "deadline-restart-fencing",
            "retry-schedule-restart",
            "structured-merge-recovery",
            "debugger-audit-restart",
            "migration-fail-closed",
        ],
    },
    ProofSuite {
        id: "runtime-recovery-injection",
        package: "leserpent-runtime",
        target_args: &["--lib"],
        expected_min_tests: 35,
        invariants: &[
            "pending-command-recovery",
            "terminal-failure-restart",
            "owner-lease-fencing",
            "snapshot-corruption-rejection",
            "prior-generation-fallback",
            "expired-lease-redelivery",
            "worker-crash-final-attempt",
            "status-projection-replay",
            "refresh-outbox-repair",
            "schema-divergence-rejection",
        ],
    },
    ProofSuite {
        id: "remote-wire-parity-boundary",
        package: "leserpentd",
        target_args: &["--lib", "remote::tests::"],
        expected_min_tests: 4,
        invariants: &[
            "remote-shared-wire-dispatch",
            "authenticated-tls-reentry",
            "remote-input-fail-closed",
            "constant-time-remote-authentication",
            "private-key-file-safety",
            "peer-failure-isolation",
        ],
    },
    ProofSuite {
        id: "remote-cli-command-parity",
        package: "leserpent-cli",
        target_args: &["--test", "https_vertical"],
        expected_min_tests: 1,
        invariants: &[
            "remote-cli-command-parity",
            "remote-cli-query-parity",
            "remote-cli-watch-parity",
            "remote-cli-confirmation-idempotency",
            "remote-cli-auth-failure",
        ],
    },
];

pub fn run_leserpent_parity_recovery_validation(
    out_dir: Option<PathBuf>,
) -> Result<ValidationReport, ValidationError> {
    let out_dir = out_dir.unwrap_or_else(|| default_out_dir("leserpent-parity-recovery"));
    fs::create_dir_all(&out_dir)?;

    let mut files = vec!["proof-summary.json".to_string()];
    let mut checks = Vec::new();
    let mut suites = Vec::with_capacity(PROOF_SUITES.len());
    let mut total_tests = 0;
    for suite in PROOF_SUITES {
        let log = format!("{}.log", suite.id);
        let log_path = out_dir.join(&log);
        let mut args = vec![
            "test".to_string(),
            "-p".to_string(),
            suite.package.to_string(),
        ];
        args.extend(suite.target_args.iter().map(|value| (*value).to_string()));
        args.extend(["--".to_string(), "--nocapture".to_string()]);
        run_cargo_status(&args, &log_path)?;
        let observed_tests = passed_test_count(&log_path)?;
        if observed_tests < suite.expected_min_tests {
            return Err(ValidationError::new(format!(
                "proof suite '{}' ran {observed_tests} tests, expected at least {}",
                suite.id, suite.expected_min_tests
            )));
        }
        total_tests += observed_tests;
        files.push(log);
        checks.extend(suite.invariants.iter().map(|value| (*value).to_string()));
        suites.push(json!({
            "id": suite.id,
            "package": suite.package,
            "target_args": suite.target_args,
            "expected_min_tests": suite.expected_min_tests,
            "observed_tests": observed_tests,
            "invariants": suite.invariants,
            "status": "passed",
        }));
    }

    fs::write(
        out_dir.join("proof-summary.json"),
        serde_json::to_string_pretty(&json!({
            "schema_version": 1,
            "host": {
                "os": std::env::consts::OS,
                "arch": std::env::consts::ARCH,
            },
            "suite_count": suites.len(),
            "observed_tests": total_tests,
            "invariant_count": checks.len(),
            "scope": [
                "current-command-origin-parity",
                "authorization-confirmation-idempotency",
                "vm-continuation-journal-recovery",
                "runtime-sqlite-recovery-injection",
                "authenticated-remote-wire-parity",
                "authenticated-remote-cli-parity",
            ],
            "suites": suites,
        }))?,
    )?;
    fs::write(
        out_dir.join("evidence-index.json"),
        serde_json::to_string_pretty(&json!({
            "schema_version": 1,
            "command": "leserpent-parity-recovery",
            "files": files,
        }))?,
    )?;

    Ok(ValidationReport {
        name: "Leserpent origin parity and recovery-injection shelf".into(),
        out_dir,
        checks,
    })
}

fn passed_test_count(log_path: &Path) -> Result<usize, ValidationError> {
    let transcript = fs::read_to_string(log_path)?;
    transcript
        .lines()
        .filter_map(|line| line.split("test result: ok. ").nth(1))
        .filter_map(|tail| tail.split_whitespace().next())
        .filter_map(|count| count.parse::<usize>().ok())
        .max()
        .ok_or_else(|| {
            ValidationError::new(format!(
                "proof transcript '{}' has no successful test count",
                log_path.display()
            ))
        })
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    #[test]
    fn proof_suite_manifest_has_non_vacuous_coverage() {
        assert_eq!(PROOF_SUITES.len(), 8);
        assert_eq!(
            PROOF_SUITES
                .iter()
                .map(|suite| suite.expected_min_tests)
                .sum::<usize>(),
            129
        );
        assert!(
            PROOF_SUITES
                .iter()
                .flat_map(|suite| suite.invariants)
                .any(|invariant| *invariant == "worker-crash-final-attempt")
        );
        assert!(
            PROOF_SUITES
                .iter()
                .flat_map(|suite| suite.invariants)
                .any(|invariant| *invariant == "remote-shared-wire-dispatch")
        );
        assert!(
            PROOF_SUITES
                .iter()
                .flat_map(|suite| suite.invariants)
                .any(|invariant| *invariant == "remote-cli-command-parity")
        );
    }

    #[test]
    fn transcript_parser_rejects_vacuous_or_failed_output() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "leserpent-parity-recovery-{}-{unique}.log",
            std::process::id()
        ));
        fs::write(
            &path,
            "test result: ok. 35 passed; 0 failed; 0 ignored; 0 measured\n",
        )
        .unwrap();
        assert_eq!(passed_test_count(&path).unwrap(), 35);
        fs::write(&path, "running 0 tests\n").unwrap();
        assert!(passed_test_count(&path).is_err());
        fs::remove_file(path).unwrap();
    }
}
