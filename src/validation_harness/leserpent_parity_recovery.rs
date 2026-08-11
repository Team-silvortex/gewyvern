use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::json;

use super::command::{
    DOTNET_PROOF_TIMEOUT, TOOL_PROBE_TIMEOUT, ValidationError, ValidationReport, default_out_dir,
    repo_root, run_cargo_status, run_command_output_with_timeout,
};
use super::dotnet_proof::run_locked_dotnet_test;

enum ProofCommand {
    Cargo {
        package: &'static str,
        target_args: &'static [&'static str],
        test_args: &'static [&'static str],
    },
    Dotnet {
        project: &'static str,
        app_args: &'static [&'static str],
        success_marker: &'static str,
    },
    DotnetTest {
        project: &'static str,
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
        id: "dotnet-control-plane-security",
        command: ProofCommand::DotnetTest {
            project: "apps/leserpent/tests/Leserpent.SecurityTests/Leserpent.SecurityTests.csproj",
        },
        expected_min_tests: 72,
        invariants: &[
            "non-vacuous-dotnet-test-discovery",
            "serialized-durable-state-save",
            "failed-save-snapshot-preservation",
        ],
    },
    ProofSuite {
        id: "avalonia-remote-state-conformance",
        command: ProofCommand::Dotnet {
            project: "apps/leserpent-avalonia/src/Leserpent.RemoteConformance/Leserpent.RemoteConformance.csproj",
            app_args: &[],
            success_marker: "remote state conformance valid: codec=true, stale=true, snapshot_revision=true, heartbeat_snapshot_fence=true, topology_state=true, authority_bound_topology=true, unproved_live_rejection=true, retained_topology=true, topology_regression_fence=true, reconnect_attempts=8, manual_resume=true, endpoint_cache=true, credential_resolution=true, trust_identity=true, workspace_atomic=true, logs_bounded=true, endpoint_retained=false, incremental_logs=true",
        },
        expected_min_tests: 1,
        invariants: &[
            "strict-health-codec",
            "authority-health-fail-closed",
            "gui-leselang-canonical-export",
            "gui-workspace-query-leselang-export",
            "explicit-copy-without-execution",
            "strict-aot-event-codec",
            "monotonic-event-revision",
            "bounded-gui-reconnect",
            "cursor-reset-resync",
            "endpoint-bound-snapshot-cache",
            "explicit-stale-state",
            "snapshot-revision-fence",
            "heartbeat-snapshot-fence",
            "authority-bound-topology",
            "unproved-live-rejection",
            "retained-topology-recovery",
            "topology-regression-fence",
            "platform-credential-precedence",
            "environment-credential-fallback",
            "invalid-stored-credential-fail-closed",
        ],
    },
    ProofSuite {
        id: "avalonia-workspace-log-filter",
        command: ProofCommand::Dotnet {
            project: "apps/leserpent-avalonia/src/Leserpent.Avalonia/Leserpent.Avalonia.csproj",
            app_args: &["--verify-workspace-diagnostics"],
            success_marker: "workspace diagnostics valid: local_only=true, query=true, level=true, combined=true, bounded=true, empty_state=true, command_identity=true, explicit_export=true, file_export=true, maximal_escape=true, live_refresh=true, bounded_retry=true, manual_recovery=true, skip_neutral=true, delta_summary=true, severity_signal=true, snapshot_fence=true, severity_ack=true, incremental_logs=true",
        },
        expected_min_tests: 1,
        invariants: &[
            "local-only-workspace-log-filter",
            "bounded-sanitized-log-query",
            "strict-log-level-selector",
            "filtered-empty-state-distinction",
            "history-command-identity",
            "explicit-bounded-diagnostic-export",
            "explicit-system-picker-diagnostic-file-export",
            "bounded-utf8-diagnostic-file",
            "safe-diagnostic-filename",
            "overwrite-confirmed-replacement-write",
            "maximally-escaped-diagnostic-export",
            "explicit-opt-in-live-log-refresh",
            "single-flight-workspace-poll",
            "inactive-window-poll-suspension",
            "bounded-live-refresh-backoff",
            "consecutive-failure-live-refresh-stop",
            "successful-manual-query-backoff-reset",
            "skipped-live-query-backoff-neutrality",
            "manual-query-live-timer-ownership",
            "bounded-workspace-delta-summary",
            "log-window-rollover-visibility",
            "workspace-revision-regression-rejection",
            "new-error-assertive-workspace-signal",
            "new-warning-workspace-signal",
            "initial-snapshot-no-severity-realert",
            "independent-snapshot-log-order-fence",
            "independent-snapshot-log-level-fence",
            "independent-snapshot-window-bound",
            "independent-snapshot-history-bound",
            "explicit-workspace-severity-acknowledgement",
            "pending-error-signal-retention",
            "severity-signal-nondowngrade",
            "acknowledged-signal-no-realert",
            "cursor-bound-live-log-query",
            "periodic-full-log-resync",
            "revision-change-full-log-resync",
            "full-batch-log-resync",
            "bounded-incremental-log-merge",
            "stale-incremental-cursor-rejection",
            "manual-full-workspace-reload",
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
            "authenticated-dotnet-health-preflight",
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
            "dotnet-workspace-leselang-rust-parse",
            "workspace-structured-read-query-lowering",
            "nested-dotnet-artifact-isolation",
        ],
    },
    ProofSuite {
        id: "mobile-lifecycle-conformance",
        command: ProofCommand::Dotnet {
            project: "apps/leserpent-mobile/src/Leserpent.MobileConformance/Leserpent.MobileConformance.csproj",
            app_args: &[],
            success_marker: "mobile lifecycle conformance valid: foreground=true, background_disconnect=true, credential_reload=true, generation_fence=true, failure_cleanup=true, application_entry=true, duplicate_callbacks=true, reconfigure=true, workspace_policy=true, ui_projection=true, mutation_fence=true, action_availability=true, authority_health=true",
        },
        expected_min_tests: 1,
        invariants: &[
            "mobile-foreground-session-ownership",
            "mobile-background-disconnect",
            "mobile-hydrated-cache-handoff",
            "mobile-credential-reload-on-reentry",
            "mobile-session-generation-fence",
            "mobile-startup-failure-cleanup",
            "mobile-application-entry",
            "mobile-duplicate-callback-coalescing",
            "mobile-safe-reconfiguration",
            "mobile-terminal-disposal-idempotency",
            "mobile-missing-credential-fail-closed",
            "mobile-credential-endpoint-isolation",
            "mobile-credential-alias-redaction",
            "mobile-credential-write-validation",
            "mobile-credential-read-validation",
            "mobile-credential-delete",
            "mobile-credential-cancellation-fence",
            "renderer-independent-workspace-policy-core",
            "mobile-reusable-workspace-policy-reference",
            "zero-avalonia-workspace-policy-dependency",
            "renderer-neutral-remote-fleet-projection",
            "renderer-neutral-runtime-workspace-projection",
            "mobile-reusable-ui-document-projection",
            "frontend-independent-mutation-revision-fence",
            "authoritative-snapshot-unknown-outcome-fence",
            "mobile-reusable-mutation-fence-policy",
            "shared-action-availability-policy",
            "mutation-unavailability-reason-precedence",
            "stale-inspect-and-mutation-disablement",
            "single-source-workspace-action-availability",
            "shared-authority-health-presentation",
            "mobile-queue-saturation-presentation",
            "proof-local-dotnet-suite-artifacts",
        ],
    },
];

pub fn run_leserpent_parity_recovery_validation(
    out_dir: Option<PathBuf>,
) -> Result<ValidationReport, ValidationError> {
    let out_dir = out_dir.unwrap_or_else(|| default_out_dir("leserpent-parity-recovery"));
    fs::create_dir_all(&out_dir)?;
    clear_previous_proof_evidence(&out_dir)?;
    let dotnet_artifacts = out_dir.join("dotnet-artifacts");
    if dotnet_artifacts.exists() {
        fs::remove_dir_all(&dotnet_artifacts)?;
    }

    let mut files = vec!["proof-summary.json".to_string()];
    let mut checks = Vec::new();
    let mut suites = Vec::with_capacity(PROOF_SUITES.len());
    let mut total_tests = 0;
    let host = proof_host_metadata()?;
    for suite in PROOF_SUITES {
        let log = format!("{}.log", suite.id);
        let log_path = out_dir.join(&log);
        let (observed_tests, command) = execute_suite(suite, &log_path, &dotnet_artifacts)?;
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
            "host": host,
            "suite_count": suites.len(),
            "observed_tests": total_tests,
            "invariant_count": checks.len(),
            "subprocess_limits": {
                "tool_probe_seconds": TOOL_PROBE_TIMEOUT.as_secs(),
                "suite_seconds": DOTNET_PROOF_TIMEOUT.as_secs(),
            },
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
    if dotnet_artifacts.exists() {
        fs::remove_dir_all(&dotnet_artifacts)?;
    }

    Ok(ValidationReport {
        name: "Leserpent origin parity and recovery-injection shelf".into(),
        out_dir,
        checks,
    })
}

fn clear_previous_proof_evidence(out_dir: &Path) -> Result<(), ValidationError> {
    for name in ["proof-summary.json", "evidence-index.json"]
        .into_iter()
        .chain(PROOF_SUITES.iter().map(|suite| suite.id))
    {
        let path = if name.ends_with(".json") {
            out_dir.join(name)
        } else {
            out_dir.join(format!("{name}.log"))
        };
        match fs::remove_file(&path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(ValidationError::new(format!(
                    "failed to remove stale proof evidence '{}': {error}",
                    path.display()
                )));
            }
        }
    }
    Ok(())
}

fn proof_host_metadata() -> Result<serde_json::Value, ValidationError> {
    let captured_unix_seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| ValidationError::new(format!("host clock predates Unix epoch: {error}")))?
        .as_secs();
    Ok(json!({
        "os": std::env::consts::OS,
        "arch": std::env::consts::ARCH,
        "kernel": kernel_release()?,
        "rustc": command_version("rustc", &["--version"])?,
        "cargo": command_version("cargo", &["--version"])?,
        "dotnet": command_version("dotnet", &["--version"])?,
        "captured_unix_seconds": captured_unix_seconds,
    }))
}

#[cfg(unix)]
fn kernel_release() -> Result<String, ValidationError> {
    command_version("uname", &["-r"])
}

#[cfg(not(unix))]
fn kernel_release() -> Result<String, ValidationError> {
    Ok("unavailable".to_string())
}

fn command_version(command: &str, args: &[&str]) -> Result<String, ValidationError> {
    const MAX_OUTPUT_BYTES: usize = 1024;
    const MAX_LINE_BYTES: usize = 256;

    let mut probe = Command::new(command);
    probe.args(args);
    let output = run_command_output_with_timeout(
        &mut probe,
        TOOL_PROBE_TIMEOUT,
        &format!("{command} version probe"),
    )?;
    if !output.status.success() {
        return Err(ValidationError::new(format!(
            "{command} version probe failed with status {}",
            output.status
        )));
    }
    parse_bounded_version_output(command, &output.stdout, MAX_OUTPUT_BYTES, MAX_LINE_BYTES)
}

fn parse_bounded_version_output(
    command: &str,
    stdout: &[u8],
    max_output_bytes: usize,
    max_line_bytes: usize,
) -> Result<String, ValidationError> {
    if stdout.len() > max_output_bytes {
        return Err(ValidationError::new(format!(
            "{command} version output exceeds {max_output_bytes} bytes"
        )));
    }
    let stdout = std::str::from_utf8(stdout).map_err(|error| {
        ValidationError::new(format!("{command} version is not UTF-8: {error}"))
    })?;
    let line = stdout
        .lines()
        .find(|line| !line.trim().is_empty())
        .map(str::trim)
        .ok_or_else(|| ValidationError::new(format!("{command} version output is empty")))?;
    if line.len() > max_line_bytes || line.chars().any(char::is_control) {
        return Err(ValidationError::new(format!(
            "{command} version line violates the bounded text contract"
        )));
    }
    Ok(line.to_string())
}

fn execute_suite(
    suite: &ProofSuite,
    log_path: &Path,
    dotnet_artifacts: &Path,
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
            app_args,
            success_marker,
        } => {
            let suite_artifacts = dotnet_artifacts.join(suite.id);
            let mut command = Command::new("dotnet");
            command
                .current_dir(repo_root())
                .args([
                    "run",
                    "--project",
                    project,
                    "--configuration",
                    "Release",
                    "--artifacts-path",
                ])
                .arg(&suite_artifacts);
            if !app_args.is_empty() {
                command.arg("--").args(*app_args);
            }
            let output = run_command_output_with_timeout(
                &mut command,
                DOTNET_PROOF_TIMEOUT,
                &format!("dotnet conformance '{}'", suite.id),
            )?;
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
            fs::remove_dir_all(&suite_artifacts)?;
            Ok((
                1,
                json!({
                    "runner": "dotnet",
                    "project": project,
                    "app_args": app_args,
                    "success_marker": success_marker,
                }),
            ))
        }
        ProofCommand::DotnetTest { project } => {
            let suite_artifacts = dotnet_artifacts.join(suite.id);
            let observed_tests = run_locked_dotnet_test(project, None, &suite_artifacts, log_path)?;
            Ok((
                observed_tests,
                json!({
                    "runner": "dotnet-test",
                    "project": project,
                    "restore_locked": true,
                    "output_locale": "en-US",
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
        assert_eq!(PROOF_SUITES.len(), 13);
        assert_eq!(
            PROOF_SUITES
                .iter()
                .map(|suite| suite.expected_min_tests)
                .sum::<usize>(),
            205
        );
        let dotnet_vertical = PROOF_SUITES
            .iter()
            .find(|suite| suite.id == "rust-dotnet-remote-vertical")
            .expect("Rust/.NET vertical proof suite must remain registered");
        assert_eq!(dotnet_vertical.expected_min_tests, 1);
        assert!(
            dotnet_vertical
                .invariants
                .contains(&"cross-language-revision-parity")
        );
        assert!(
            dotnet_vertical
                .invariants
                .contains(&"same-revision-workspace-composition")
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
        assert!(
            PROOF_SUITES
                .iter()
                .flat_map(|suite| suite.invariants)
                .any(|invariant| *invariant == "local-only-workspace-log-filter")
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

    #[test]
    fn version_output_parser_is_bounded_and_single_line() {
        assert_eq!(
            parse_bounded_version_output("tool", b"tool 1.2.3\nextra\n", 64, 32).unwrap(),
            "tool 1.2.3"
        );
        assert!(parse_bounded_version_output("tool", &[0xff], 64, 32).is_err());
        assert!(parse_bounded_version_output("tool", b"tool\t1.2.3\n", 64, 32).is_err());
        assert!(parse_bounded_version_output("tool", &[b'x'; 65], 64, 64).is_err());
    }

    #[test]
    fn proof_start_removes_only_known_stale_evidence() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let out_dir = std::env::temp_dir().join(format!(
            "leserpent-stale-proof-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir_all(&out_dir).unwrap();
        fs::write(out_dir.join("proof-summary.json"), "stale").unwrap();
        fs::write(out_dir.join("evidence-index.json"), "stale").unwrap();
        fs::write(out_dir.join(format!("{}.log", PROOF_SUITES[0].id)), "stale").unwrap();
        fs::write(out_dir.join("operator-note.txt"), "retain").unwrap();

        clear_previous_proof_evidence(&out_dir).unwrap();

        assert!(!out_dir.join("proof-summary.json").exists());
        assert!(!out_dir.join("evidence-index.json").exists());
        assert!(!out_dir.join(format!("{}.log", PROOF_SUITES[0].id)).exists());
        assert!(out_dir.join("operator-note.txt").is_file());
        fs::remove_dir_all(out_dir).unwrap();
    }
}
