use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::json;

use super::command::{
    ValidationError, ValidationReport, default_out_dir, repo_root, run_cargo_status,
};

enum ProofCommand {
    Cargo {
        package: &'static str,
        target_args: &'static [&'static str],
        test_args: &'static [&'static str],
    },
    Dotnet {
        project: &'static str,
        success_marker: &'static str,
    },
}

struct ProofSuite {
    id: &'static str,
    command: ProofCommand,
    expected_min_tests: usize,
    invariants: &'static [&'static str],
}

const PROOF_SUITES: &[ProofSuite] = &[
    ProofSuite {
        id: "command-origin-lowering",
        command: ProofCommand::Cargo {
            package: "leselang-command",
            target_args: &["--lib"],
            test_args: &[],
        },
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
        command: ProofCommand::Cargo {
            package: "leserpent-domain",
            target_args: &["--lib"],
            test_args: &[],
        },
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
        command: ProofCommand::Cargo {
            package: "leselang-observe",
            target_args: &["--lib", "debugger_cancel_"],
            test_args: &[],
        },
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
        command: ProofCommand::Cargo {
            package: "leserpent-cli",
            target_args: &["--test", "leselang_parity"],
            test_args: &[],
        },
        expected_min_tests: 3,
        invariants: &[
            "refresh-command-equivalence",
            "inspect-query-equivalence",
            "history-query-equivalence",
        ],
    },
    ProofSuite {
        id: "vm-reentry-recovery",
        command: ProofCommand::Cargo {
            package: "leselang-vm",
            target_args: &["--lib"],
            test_args: &[],
        },
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
        command: ProofCommand::Cargo {
            package: "leserpent-runtime",
            target_args: &["--lib"],
            test_args: &[],
        },
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
        command: ProofCommand::Cargo {
            package: "leserpentd",
            target_args: &["--lib", "remote::tests::"],
            test_args: &[],
        },
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
        command: ProofCommand::Cargo {
            package: "leserpent-cli",
            target_args: &["--test", "https_vertical"],
            test_args: &[],
        },
        expected_min_tests: 1,
        invariants: &[
            "remote-cli-command-parity",
            "remote-cli-query-parity",
            "remote-cli-watch-parity",
            "remote-cli-confirmation-idempotency",
            "remote-cli-auth-failure",
        ],
    },
    ProofSuite {
        id: "avalonia-remote-state-conformance",
        command: ProofCommand::Dotnet {
            project: "apps/leserpent-avalonia/src/Leserpent.RemoteConformance/Leserpent.RemoteConformance.csproj",
            success_marker: "remote state conformance valid: codec=true, stale=true, reconnect_attempts=8, manual_resume=true, endpoint_cache=true, credential_resolution=true, trust_identity=true, workspace_atomic=true, logs_bounded=true, endpoint_retained=false",
        },
        expected_min_tests: 1,
        invariants: &[
            "strict-aot-event-codec",
            "monotonic-event-revision",
            "bounded-gui-reconnect",
            "cursor-reset-resync",
            "endpoint-bound-snapshot-cache",
            "explicit-stale-state",
            "platform-credential-precedence",
            "environment-credential-fallback",
            "invalid-stored-credential-fail-closed",
        ],
    },
    ProofSuite {
        id: "rust-dotnet-remote-vertical",
        command: ProofCommand::Cargo {
            package: "leserpent-cli",
            target_args: &["--test", "dotnet_remote_vertical"],
            test_args: &["--ignored"],
        },
        expected_min_tests: 1,
        invariants: &[
            "cross-language-revision-parity",
            "authenticated-dotnet-runtime-refresh",
            "explicit-confirmed-remote-mutation",
            "optimistic-runtime-revision-fence",
            "websocket-mutation-observation",
            "explicit-ca-hostname-validation",
            "bearer-subprotocol-negotiation",
            "nonempty-runtime-snapshot",
            "endpoint-redacted-client-cache",
            "private-client-cache-permissions",
            "authenticated-dotnet-runtime-inspect",
            "same-revision-workspace-composition",
            "bounded-runtime-history",
            "bounded-sanitized-runtime-logs",
            "endpoint-redacted-workspace-output",
        ],
    },
    ProofSuite {
        id: "mobile-lifecycle-conformance",
        command: ProofCommand::Dotnet {
            project: "apps/leserpent-mobile/src/Leserpent.MobileConformance/Leserpent.MobileConformance.csproj",
            success_marker: "mobile lifecycle conformance valid: foreground=true, background_disconnect=true, credential_reload=true, generation_fence=true, failure_cleanup=true",
        },
        expected_min_tests: 1,
        invariants: &[
            "mobile-foreground-session-ownership",
            "mobile-background-disconnect",
            "mobile-hydrated-cache-handoff",
            "mobile-credential-reload-on-reentry",
            "mobile-session-generation-fence",
            "mobile-startup-failure-cleanup",
            "mobile-terminal-disposal-idempotency",
            "mobile-missing-credential-fail-closed",
            "mobile-credential-endpoint-isolation",
            "mobile-credential-alias-redaction",
            "mobile-credential-write-validation",
            "mobile-credential-read-validation",
            "mobile-credential-delete",
            "mobile-credential-cancellation-fence",
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
        let (observed_tests, command) = execute_suite(suite, &log_path)?;
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
            "command": command,
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
                "avalonia-remote-state-parity",
                "rust-dotnet-remote-vertical",
                "rust-dotnet-workspace-query-vertical",
                "desktop-platform-credential-resolution",
                "mobile-lifecycle-conformance",
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

fn execute_suite(
    suite: &ProofSuite,
    log_path: &Path,
) -> Result<(usize, serde_json::Value), ValidationError> {
    match &suite.command {
        ProofCommand::Cargo {
            package,
            target_args,
            test_args,
        } => {
            let mut args = vec!["test".to_string(), "-p".to_string(), (*package).to_string()];
            args.extend(target_args.iter().map(|value| (*value).to_string()));
            args.push("--".to_string());
            args.extend(test_args.iter().map(|value| (*value).to_string()));
            args.push("--nocapture".to_string());
            run_cargo_status(&args, log_path)?;
            Ok((
                passed_test_count(log_path)?,
                json!({
                    "runner": "cargo",
                    "package": package,
                    "target_args": target_args,
                    "test_args": test_args,
                }),
            ))
        }
        ProofCommand::Dotnet {
            project,
            success_marker,
        } => {
            let output = Command::new("dotnet")
                .current_dir(repo_root())
                .args(["run", "--project", project, "--configuration", "Release"])
                .output()
                .map_err(|error| ValidationError::new(format!("failed to run dotnet: {error}")))?;
            write_output(log_path, &output)?;
            if !output.status.success() {
                return Err(ValidationError::new(format!(
                    "dotnet conformance '{}' failed with status {}",
                    suite.id, output.status
                )));
            }
            let stdout = String::from_utf8_lossy(&output.stdout);
            let marker_count = stdout
                .lines()
                .filter(|line| *line == *success_marker)
                .count();
            if marker_count != 1 {
                return Err(ValidationError::new(format!(
                    "dotnet conformance '{}' emitted {marker_count} success markers, expected exactly one",
                    suite.id
                )));
            }
            Ok((
                1,
                json!({
                    "runner": "dotnet",
                    "project": project,
                    "success_marker": success_marker,
                }),
            ))
        }
    }
}

fn write_output(path: &Path, output: &Output) -> Result<(), ValidationError> {
    let mut transcript = Vec::new();
    transcript.extend_from_slice(b"stdout:\n");
    transcript.extend_from_slice(&output.stdout);
    transcript.extend_from_slice(b"\n\nstderr:\n");
    transcript.extend_from_slice(&output.stderr);
    fs::write(path, transcript)?;
    Ok(())
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
        assert_eq!(PROOF_SUITES.len(), 11);
        assert_eq!(
            PROOF_SUITES
                .iter()
                .map(|suite| suite.expected_min_tests)
                .sum::<usize>(),
            132
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
        assert!(
            PROOF_SUITES
                .iter()
                .flat_map(|suite| suite.invariants)
                .any(|invariant| *invariant == "cross-language-revision-parity")
        );
        assert!(
            PROOF_SUITES
                .iter()
                .flat_map(|suite| suite.invariants)
                .any(|invariant| *invariant == "same-revision-workspace-composition")
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
