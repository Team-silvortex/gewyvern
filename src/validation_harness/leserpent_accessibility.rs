use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::{Value, json};

use super::command::{
    DOTNET_PROOF_TIMEOUT, PROOF_FIXTURE_TIMEOUT, ValidationError, ValidationReport,
    default_out_dir, repo_root, run_command_output_with_timeout,
};

const FIXTURES: &[&str] = &[
    "renderer-conformance-v1.json",
    "renderer-debugger-conformance-v1.json",
    "renderer-log-conformance-v1.json",
    "renderer-workspace-conformance-v1.json",
];

struct ProductProbe {
    id: &'static str,
    argument: &'static str,
    success_prefix: &'static str,
    required_fragments: &'static [&'static str],
}

const PRODUCT_PROBES: &[ProductProbe] = &[
    ProductProbe {
        id: "hub-topology",
        argument: "--verify-hub-topology",
        success_prefix: "Hub topology valid:",
        required_fragments: &[
            "client_root=true",
            "runtime_actions=6",
            "refresh_all_control=true",
            "authoritative_workspace_gate=true",
            "bounded_dynamic_text=true",
            "open_source_core=true",
        ],
    },
    ProductProbe {
        id: "remote-shell",
        argument: "--verify-remote-shell-controls",
        success_prefix: "remote shell controls valid:",
        required_fragments: &[
            "native_plans=true",
            "queued_cancel=true",
            "debugger_presentation_reentry=true",
            "registration_confirmation=true",
            "registration_mutation_fence=true",
            "network_started=false",
        ],
    },
    ProductProbe {
        id: "daemon-bootstrap",
        argument: "--verify-bootstrap-controls",
        success_prefix: "bootstrap controls valid:",
        required_fragments: &[
            "unconfirmed_submit_blocked=true",
            "submit=true",
            "bind=true",
            "local_promotion=true",
            "automation=true",
            "late_completion_close_fence=true",
            "polling_restart_after_close=false",
            "settled_lifetime_disposal=true",
        ],
    },
    ProductProbe {
        id: "gewyvern-provisioning",
        argument: "--verify-provisioning-controls",
        success_prefix: "provisioning controls valid:",
        required_fragments: &[
            "unconfirmed_submit_blocked=true",
            "stable_identity=true",
            "observation_limit_no_reconcile=true",
            "terminal_state=true",
            "automation=true",
            "late_completion_close_fence=true",
            "polling_restart_after_close=false",
            "settled_lifetime_disposal=true",
        ],
    },
    ProductProbe {
        id: "gewyvern-retirement",
        argument: "--verify-retirement-controls",
        success_prefix: "retirement controls valid:",
        required_fragments: &[
            "provisioning_bound=true",
            "unconfirmed_submit_blocked=true",
            "terminal_state=true",
            "failure_preserves_registration=true",
            "automation=true",
            "late_completion_close_fence=true",
            "polling_restart_after_close=false",
            "settled_lifetime_disposal=true",
        ],
    },
    ProductProbe {
        id: "daemon-retirement",
        argument: "--verify-daemon-retirement-controls",
        success_prefix: "daemon retirement controls valid:",
        required_fragments: &[
            "bootstrap_bound=true",
            "authority_omitting=true",
            "unconfirmed_submit_blocked=true",
            "terminal_state=true",
            "retry_guidance=true",
            "automation=true",
            "late_completion_close_fence=true",
            "polling_restart_after_close=false",
            "settled_lifetime_disposal=true",
        ],
    },
];

pub fn run_leserpent_accessibility_validation(
    out_dir: Option<PathBuf>,
) -> Result<ValidationReport, ValidationError> {
    let needs_xvfb = match (env::consts::OS, env::consts::ARCH) {
        ("macos", "aarch64") => false,
        ("linux", "x86_64") => true,
        _ => {
            return Err(ValidationError::new(format!(
                "Leserpent accessibility validation does not support host {}-{}",
                env::consts::OS,
                env::consts::ARCH
            )));
        }
    };
    let out_dir = out_dir.unwrap_or_else(|| default_out_dir("leserpent-accessibility"));
    fs::create_dir_all(&out_dir)?;
    clear_previous_evidence(&out_dir)?;
    let dotnet_artifacts = out_dir.join("dotnet-artifacts");
    if dotnet_artifacts.exists() {
        fs::remove_dir_all(&dotnet_artifacts)?;
    }
    let root = repo_root();
    let app = root.join("apps/leserpent-avalonia/src/Leserpent.Avalonia/Leserpent.Avalonia.csproj");

    run_logged(
        Command::new("dotnet")
            .current_dir(&root)
            .arg("restore")
            .arg(&app)
            .arg("--locked-mode")
            .arg("--artifacts-path")
            .arg(&dotnet_artifacts),
        &out_dir.join("restore.log"),
        "locked Avalonia restore failed",
    )?;
    run_logged(
        Command::new("dotnet")
            .current_dir(&root)
            .arg("build")
            .arg(&app)
            .args(["--no-restore", "-c", "Release"])
            .arg("--artifacts-path")
            .arg(&dotnet_artifacts),
        &out_dir.join("build.log"),
        "Avalonia accessibility build failed",
    )?;

    let assembly = dotnet_artifacts.join("bin/Leserpent.Avalonia/release/Leserpent.Avalonia.dll");
    if !assembly.is_file() {
        return Err(ValidationError::new(format!(
            "Avalonia accessibility assembly not found: {}",
            assembly.display()
        )));
    }
    let fixtures_dir = root.join("apps/leserpent-avalonia/fixtures");
    let mut summaries = Vec::new();
    for fixture in FIXTURES {
        let fixture_path = fixtures_dir.join(fixture);
        let mut command = desktop_command(needs_xvfb);
        command
            .arg(&assembly)
            .arg("--verify-controls")
            .arg(&fixture_path);
        let output = run_command_output_with_timeout(
            &mut command,
            PROOF_FIXTURE_TIMEOUT,
            &format!(
                "accessibility fixture `{fixture}`{}",
                desktop_dependency(needs_xvfb)
            ),
        )?;
        let log_path = out_dir.join(format!("fixture-{fixture}.log"));
        write_output(&log_path, &output)?;
        if !output.status.success() {
            return Err(command_failure(
                &format!("accessibility fixture `{fixture}` failed"),
                &output,
            ));
        }
        let text = String::from_utf8_lossy(&output.stdout);
        let proof = require_accessibility_proof(&text, fixture)?;
        summaries.push(proof.to_json(fixture));
    }

    let mut product_probes = Vec::with_capacity(PRODUCT_PROBES.len());
    for probe in PRODUCT_PROBES {
        let mut command = desktop_command(needs_xvfb);
        command.arg(&assembly).arg(probe.argument);
        let output = run_command_output_with_timeout(
            &mut command,
            PROOF_FIXTURE_TIMEOUT,
            &format!(
                "product function-chain probe `{}`{}",
                probe.id,
                desktop_dependency(needs_xvfb)
            ),
        )?;
        let log_path = out_dir.join(format!("probe-{}.log", probe.id));
        write_output(&log_path, &output)?;
        if !output.status.success() {
            return Err(command_failure(
                &format!("product function-chain probe `{}` failed", probe.id),
                &output,
            ));
        }
        let text = std::str::from_utf8(&output.stdout).map_err(|_| {
            ValidationError::new(format!(
                "product function-chain probe `{}` emitted non-UTF-8 stdout",
                probe.id
            ))
        })?;
        product_probes.push(require_product_probe(text, probe)?);
    }

    fs::write(
        out_dir.join("accessibility-summary.json"),
        serde_json::to_string_pretty(&json!({
            "schema_version": 2,
            "os": env::consts::OS,
            "arch": env::consts::ARCH,
            "minimum_required_contrast": 4.5,
            "subprocess_limits": {
                "dotnet_seconds": DOTNET_PROOF_TIMEOUT.as_secs(),
                "fixture_seconds": PROOF_FIXTURE_TIMEOUT.as_secs(),
            },
            "fixtures": summaries,
            "product_function_chains": product_probes,
        }))?,
    )?;
    let mut files = vec![
        "restore.log".to_string(),
        "build.log".to_string(),
        "accessibility-summary.json".to_string(),
    ];
    files.extend(
        FIXTURES
            .iter()
            .map(|fixture| format!("fixture-{fixture}.log")),
    );
    files.extend(
        PRODUCT_PROBES
            .iter()
            .map(|probe| format!("probe-{}.log", probe.id)),
    );
    fs::write(
        out_dir.join("evidence-index.json"),
        serde_json::to_string_pretty(&json!({
            "schema_version": 1,
            "command": "leserpent-accessibility",
            "files": files,
        }))?,
    )?;
    fs::remove_dir_all(&dotnet_artifacts)?;

    Ok(ValidationReport {
        name: "Leserpent Avalonia accessibility proof".to_string(),
        out_dir,
        checks: vec![
            "unique_automation_ids".to_string(),
            "complete_automation_names".to_string(),
            "explicit_action_labels".to_string(),
            "automation_help_text_mapping".to_string(),
            "wcag_aa_text_contrast".to_string(),
            "four_real_control_fixtures".to_string(),
            "six_product_function_chain_probes".to_string(),
            "strict_product_probe_success_markers".to_string(),
            "isolated_dotnet_artifacts".to_string(),
            "bounded_dotnet_and_fixture_subprocesses".to_string(),
            "stale_evidence_invalidation".to_string(),
            "failed_fixture_log_retention".to_string(),
        ],
    })
}

fn desktop_command(needs_xvfb: bool) -> Command {
    if needs_xvfb {
        let mut command = Command::new("xvfb-run");
        command.args(["-a", "-s", "-screen 0 1280x800x24", "dotnet"]);
        command
    } else {
        Command::new("dotnet")
    }
}

fn desktop_dependency(needs_xvfb: bool) -> &'static str {
    if needs_xvfb {
        "; xvfb-run requires xvfb and xauth on the Linux host"
    } else {
        ""
    }
}

fn require_product_probe(output: &str, probe: &ProductProbe) -> Result<Value, ValidationError> {
    let matching_lines = output
        .lines()
        .filter(|line| line.starts_with(probe.success_prefix))
        .collect::<Vec<_>>();
    if matching_lines.len() != 1 {
        return Err(ValidationError::new(format!(
            "product function-chain probe `{}` emitted {} `{}` success lines, expected exactly one",
            probe.id,
            matching_lines.len(),
            probe.success_prefix
        )));
    }
    let missing = probe
        .required_fragments
        .iter()
        .filter(|fragment| !matching_lines[0].contains(**fragment))
        .copied()
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        return Err(ValidationError::new(format!(
            "product function-chain probe `{}` omitted required markers: {}",
            probe.id,
            missing.join(", ")
        )));
    }
    Ok(json!({
        "id": probe.id,
        "argument": probe.argument,
        "success_prefix": probe.success_prefix,
        "required_fragments": probe.required_fragments,
        "status": "passed",
    }))
}

#[derive(Clone, Copy, Debug)]
pub(super) struct AccessibilityProof {
    controls: usize,
    names: usize,
    labels: usize,
    actions: usize,
    initial_actions: usize,
    help_texts: usize,
    minimum_contrast: f64,
}

impl AccessibilityProof {
    fn to_json(self, fixture: &str) -> Value {
        json!({
            "fixture": fixture,
            "controls": self.controls,
            "names": self.names,
            "labels": self.labels,
            "actions": self.actions,
            "initial_actions": self.initial_actions,
            "help_texts": self.help_texts,
            "minimum_contrast": self.minimum_contrast,
        })
    }
}

pub(super) fn require_accessibility_proof(
    output: &str,
    fixture: &str,
) -> Result<AccessibilityProof, ValidationError> {
    if !output.contains("accessibility_valid=true") {
        return Err(ValidationError::new(format!(
            "fixture `{fixture}` omitted the accessibility proof marker"
        )));
    }
    let proof = AccessibilityProof {
        controls: metric(output, "accessibility_controls")?,
        names: metric(output, "accessibility_names")?,
        labels: metric(output, "accessibility_labels")?,
        actions: metric(output, "accessibility_actions")?,
        initial_actions: metric(output, "initial_accessibility_actions")?,
        help_texts: metric(output, "accessibility_help_texts")?,
        minimum_contrast: decimal_metric(output, "minimum_contrast")?,
    };
    if proof.controls == 0 || proof.controls != proof.names || proof.minimum_contrast < 4.5 {
        return Err(ValidationError::new(format!(
            "fixture `{fixture}` failed the accessibility count or contrast invariant"
        )));
    }
    if fixture == "renderer-debugger-conformance-v1.json" && proof.initial_actions != 1 {
        return Err(ValidationError::new(
            "debugger fixture did not expose exactly one accessible initial action",
        ));
    }
    Ok(proof)
}

fn metric(output: &str, name: &str) -> Result<usize, ValidationError> {
    metric_text(output, name)?
        .parse()
        .map_err(|_| ValidationError::new(format!("invalid accessibility metric `{name}`")))
}

fn decimal_metric(output: &str, name: &str) -> Result<f64, ValidationError> {
    metric_text(output, name)?
        .parse()
        .map_err(|_| ValidationError::new(format!("invalid accessibility metric `{name}`")))
}

fn metric_text<'a>(output: &'a str, name: &str) -> Result<&'a str, ValidationError> {
    let prefix = format!("{name}=");
    output
        .split([',', '\n'])
        .map(str::trim)
        .find_map(|field| field.strip_prefix(&prefix))
        .ok_or_else(|| ValidationError::new(format!("missing accessibility metric `{name}`")))
}

fn clear_previous_evidence(out_dir: &Path) -> Result<(), ValidationError> {
    for name in [
        "restore.log".to_string(),
        "build.log".to_string(),
        "accessibility-summary.json".to_string(),
        "evidence-index.json".to_string(),
    ]
    .into_iter()
    .chain(
        FIXTURES
            .iter()
            .map(|fixture| format!("fixture-{fixture}.log")),
    )
    .chain(
        PRODUCT_PROBES
            .iter()
            .map(|probe| format!("probe-{}.log", probe.id)),
    ) {
        match fs::remove_file(out_dir.join(&name)) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(ValidationError::new(format!(
                    "failed to remove stale accessibility evidence `{name}`: {error}"
                )));
            }
        }
    }
    Ok(())
}

fn run_logged(
    command: &mut Command,
    log_path: &Path,
    context: &str,
) -> Result<(), ValidationError> {
    let output = run_command_output_with_timeout(command, DOTNET_PROOF_TIMEOUT, context)?;
    write_output(log_path, &output)?;
    if !output.status.success() {
        return Err(command_failure(context, &output));
    }
    Ok(())
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

fn command_failure(context: &str, output: &Output) -> ValidationError {
    ValidationError::new(format!(
        "{context}: {}\n{}",
        output.status,
        String::from_utf8_lossy(&output.stderr).trim()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_complete_accessibility_proof() {
        let proof = require_accessibility_proof(
            "initial_accessibility_actions=1, accessibility_controls=9, accessibility_names=9, accessibility_labels=2, accessibility_actions=0, accessibility_help_texts=1, minimum_contrast=4.723, accessibility_valid=true",
            "renderer-debugger-conformance-v1.json",
        )
        .unwrap();

        assert_eq!(proof.controls, 9);
        assert_eq!(proof.help_texts, 1);
    }

    #[test]
    fn rejects_missing_names_or_low_contrast() {
        let missing_name = "initial_accessibility_actions=0, accessibility_controls=9, accessibility_names=8, accessibility_labels=2, accessibility_actions=0, accessibility_help_texts=0, minimum_contrast=4.723, accessibility_valid=true";
        assert!(require_accessibility_proof(missing_name, "fixture.json").is_err());
        let low_contrast = missing_name
            .replace("accessibility_names=8", "accessibility_names=9")
            .replace("4.723", "3.841");
        assert!(require_accessibility_proof(&low_contrast, "fixture.json").is_err());
    }

    #[test]
    fn product_probe_manifest_covers_the_closed_desktop_lifecycle_chains() {
        let ids = PRODUCT_PROBES
            .iter()
            .map(|probe| probe.id)
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(PRODUCT_PROBES.len(), 6);
        assert_eq!(ids.len(), PRODUCT_PROBES.len());
        assert!(ids.contains("hub-topology"));
        assert!(ids.contains("remote-shell"));
        assert!(ids.contains("daemon-bootstrap"));
        assert!(ids.contains("gewyvern-provisioning"));
        assert!(ids.contains("gewyvern-retirement"));
        assert!(ids.contains("daemon-retirement"));
    }

    #[test]
    fn product_probe_success_contract_rejects_missing_or_duplicate_markers() {
        let probe = &PRODUCT_PROBES[0];
        let valid = format!(
            "{} {}\n",
            probe.success_prefix,
            probe.required_fragments.join(", ")
        );
        assert!(require_product_probe(&valid, probe).is_ok());
        assert!(require_product_probe(probe.success_prefix, probe).is_err());
        assert!(require_product_probe(&format!("{valid}{valid}"), probe).is_err());
    }
}
