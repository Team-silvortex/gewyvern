use std::fs;
use std::path::PathBuf;

use serde_json::json;

use super::command::{ValidationError, ValidationReport, default_out_dir, run_cargo_status};

struct ProofSuite {
    id: &'static str,
    package: &'static str,
    target_args: &'static [&'static str],
    invariants: &'static [&'static str],
}

const PROOF_SUITES: &[ProofSuite] = &[
    ProofSuite {
        id: "wire-v1-contract",
        package: "leserpent-protocol",
        target_args: &["--lib"],
        invariants: &[
            "canonical-fixture-decode",
            "explicit-schema-version",
            "bounded-message-decode",
            "legacy-health-decode",
        ],
    },
    ProofSuite {
        id: "legacy-v1-compatibility",
        package: "leserpent-protocol",
        target_args: &["--test", "compatibility_v1"],
        invariants: &[
            "legacy-normalization",
            "idempotent-command-adaptation",
            "identity-confusion-rejection",
            "wire-size-limit",
        ],
    },
    ProofSuite {
        id: "cli-leselang-parity",
        package: "leserpent-cli",
        target_args: &["--test", "leselang_parity"],
        invariants: &[
            "refresh-command-parity",
            "inspect-query-parity",
            "history-query-parity",
        ],
    },
    ProofSuite {
        id: "authenticated-ipc-vertical",
        package: "leserpent-cli",
        target_args: &["--test", "ipc_vertical"],
        invariants: &[
            "real-unix-socket",
            "native-cli-daemon-roundtrip",
            "confirmation-boundary",
            "idempotent-apply",
            "bounded-watch",
        ],
    },
    ProofSuite {
        id: "ipc-security-boundary",
        package: "leserpentd",
        target_args: &["--lib", "ipc::tests::"],
        invariants: &[
            "owner-private-socket",
            "invalid-token-rejection",
            "malformed-frame-rejection",
            "oversized-frame-rejection",
            "endpoint-nondisclosure",
            "authority-health",
        ],
    },
];

pub fn run_leserpent_transport_validation(
    out_dir: Option<PathBuf>,
) -> Result<ValidationReport, ValidationError> {
    if !cfg!(unix) {
        return Err(ValidationError::new(
            "Leserpent local transport proof currently requires a Unix host; Windows named-pipe parity is not implemented",
        ));
    }

    let out_dir = out_dir.unwrap_or_else(|| default_out_dir("leserpent-transport"));
    fs::create_dir_all(&out_dir)?;

    let mut logs = Vec::with_capacity(PROOF_SUITES.len());
    let mut checks = Vec::new();
    let mut suites = Vec::with_capacity(PROOF_SUITES.len());
    for suite in PROOF_SUITES {
        let log = format!("{}.log", suite.id);
        let mut args = vec![
            "test".to_string(),
            "-p".to_string(),
            suite.package.to_string(),
        ];
        args.extend(suite.target_args.iter().map(|value| (*value).to_string()));
        args.extend(["--".to_string(), "--nocapture".to_string()]);
        run_cargo_status(&args, &out_dir.join(&log))?;

        logs.push(log);
        checks.extend(suite.invariants.iter().map(|value| (*value).to_string()));
        suites.push(json!({
            "id": suite.id,
            "package": suite.package,
            "target_args": suite.target_args,
            "invariants": suite.invariants,
            "status": "passed",
        }));
    }

    fs::write(
        out_dir.join("transport-summary.json"),
        serde_json::to_string_pretty(&json!({
            "schema_version": 1,
            "transport_scope": "authenticated-unix-ipc",
            "wire_schema": "v1",
            "host": {
                "os": std::env::consts::OS,
                "arch": std::env::consts::ARCH,
            },
            "suite_count": suites.len(),
            "invariant_count": checks.len(),
            "suites": suites,
            "excluded_future_transports": [
                "windows-named-pipe",
                "authenticated-https",
                "authenticated-websocket",
            ],
        }))?,
    )?;
    let mut files = vec!["transport-summary.json".to_string()];
    files.extend(logs);
    fs::write(
        out_dir.join("evidence-index.json"),
        serde_json::to_string_pretty(&json!({
            "schema_version": 1,
            "command": "leserpent-transport",
            "files": files,
        }))?,
    )?;

    Ok(ValidationReport {
        name: "Leserpent authenticated local transport compatibility shelf".to_string(),
        out_dir,
        checks,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proof_suites_cover_wire_parity_real_ipc_and_security() {
        assert_eq!(PROOF_SUITES.len(), 5);
        let ids = PROOF_SUITES
            .iter()
            .map(|suite| suite.id)
            .collect::<Vec<_>>();
        assert!(ids.contains(&"wire-v1-contract"));
        assert!(ids.contains(&"legacy-v1-compatibility"));
        assert!(ids.contains(&"cli-leselang-parity"));
        assert!(ids.contains(&"authenticated-ipc-vertical"));
        assert!(ids.contains(&"ipc-security-boundary"));
        assert!(
            PROOF_SUITES
                .iter()
                .flat_map(|suite| suite.invariants)
                .any(|invariant| *invariant == "invalid-token-rejection")
        );
    }
}
