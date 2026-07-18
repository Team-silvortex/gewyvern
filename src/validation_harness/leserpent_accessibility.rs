use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::{Value, json};

use super::command::{ValidationError, ValidationReport, default_out_dir, repo_root};

const FIXTURES: &[&str] = &[
    "renderer-conformance-v1.json",
    "renderer-debugger-conformance-v1.json",
    "renderer-log-conformance-v1.json",
    "renderer-workspace-conformance-v1.json",
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
        let mut command = if needs_xvfb {
            let mut command = Command::new("xvfb-run");
            command.args(["-a", "-s", "-screen 0 1280x800x24", "dotnet"]);
            command
        } else {
            Command::new("dotnet")
        };
        let output = command
            .arg(&assembly)
            .arg("--verify-controls")
            .arg(&fixture_path)
            .output()
            .map_err(|err| {
                let dependency = if needs_xvfb {
                    "; install xvfb and xauth on the Linux host"
                } else {
                    ""
                };
                ValidationError::new(format!(
                    "failed to run accessibility fixture `{fixture}`: {err}{dependency}"
                ))
            })?;
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

    fs::write(
        out_dir.join("accessibility-summary.json"),
        serde_json::to_string_pretty(&json!({
            "schema_version": 1,
            "os": env::consts::OS,
            "arch": env::consts::ARCH,
            "minimum_required_contrast": 4.5,
            "fixtures": summaries,
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
            "isolated_dotnet_artifacts".to_string(),
        ],
    })
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

fn run_logged(
    command: &mut Command,
    log_path: &Path,
    context: &str,
) -> Result<(), ValidationError> {
    let output = command
        .output()
        .map_err(|err| ValidationError::new(format!("{context}: {err}")))?;
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
}
