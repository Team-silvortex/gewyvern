use std::process::{Command, Stdio};
use std::{collections::BTreeMap, fs, path::Path};

use super::command::{ValidationError, ValidationReport, repo_root};
use super::{
    RemoteLinuxHostOptions, run_container_runtime_validation, run_container_validation_summary,
    run_package_install_smoke, run_pathological_container_validation,
    run_remote_linux_host_validation, run_three_module_stack_smoke, validation_command_stdout,
    validation_log,
};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ReleaseCheckMode {
    Deb,
    Rpm,
    #[default]
    DebAndRpm,
}

impl ReleaseCheckMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Deb => "deb",
            Self::Rpm => "rpm",
            Self::DebAndRpm => "deb+rpm",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReleaseGateOptions {
    pub run_build: bool,
    pub run_release_check: bool,
    pub run_stack: bool,
    pub run_pathology: bool,
    pub run_remote_host: bool,
    pub remote_host: String,
    pub remote_dir: Option<String>,
    pub keep_remote_dir: bool,
    pub remote_build_packages: bool,
    pub release_mode: ReleaseCheckMode,
}

impl Default for ReleaseGateOptions {
    fn default() -> Self {
        Self {
            run_build: true,
            run_release_check: true,
            run_stack: true,
            run_pathology: true,
            run_remote_host: false,
            remote_host: std::env::var("GEWY_REMOTE_HOST")
                .unwrap_or_else(|_| "kyuubiki-lab".to_string()),
            remote_dir: None,
            keep_remote_dir: false,
            remote_build_packages: true,
            release_mode: ReleaseCheckMode::DebAndRpm,
        }
    }
}

pub fn run_release_container_check(
    mode: ReleaseCheckMode,
) -> Result<ValidationReport, ValidationError> {
    let mut checks = Vec::new();
    validation_log(format!(
        "[release-check] starting packaged release validation ({})",
        mode.label()
    ));

    validation_log("[release-check] ----------------------------------------");
    validation_log("[release-check] running package install smoke");
    run_package_install_smoke(mode)?;
    checks.push("package_install_smoke".to_string());

    validation_log("[release-check] ----------------------------------------");
    validation_log("[release-check] running packaged runtime validation");
    run_container_runtime_validation(mode)?;
    checks.push("packaged_runtime_validation".to_string());

    validation_log("[release-check] ----------------------------------------");
    validation_log("[release-check] running packaged protocol/operator summary");
    run_container_validation_summary(mode)?;
    checks.push("packaged_protocol_operator_summary".to_string());

    validation_log("[release-check] ----------------------------------------");
    validation_log(format!(
        "[release-check] packaged release validation: ok ({})",
        mode.label()
    ));
    validation_log(
        "[release-check] covered packaged checks: package-install-smoke, container-runtime-validation, container-validation-summary",
    );

    Ok(ValidationReport {
        name: format!("packaged release validation ({})", mode.label()),
        out_dir: repo_root().join("target").join("validation"),
        checks,
    })
}

pub fn run_release_gate(options: ReleaseGateOptions) -> Result<ValidationReport, ValidationError> {
    let mut checks = Vec::new();

    if options.run_build {
        run_step(
            "release-gate",
            "building fresh native artifacts",
            "scripts/packaging/build_packages_in_container.sh",
            &["--format", "all"],
        )?;
        checks.push("build_packages_in_container".to_string());
    } else {
        validation_log("[release-gate] skipping package rebuild");
    }

    if options.run_release_check {
        validation_log("[release-gate] ----------------------------------------");
        match options.release_mode {
            ReleaseCheckMode::DebAndRpm => {
                validation_log("[release-gate] running packaged release validation");
                validation_log(
                    "[release-gate] packaged release scope: package-install-smoke + container-runtime-validation + container-validation-summary (deb+rpm)",
                );
            }
            mode => {
                validation_log(format!(
                    "[release-gate] running packaged release validation ({})",
                    mode.label()
                ));
                validation_log(format!(
                    "[release-gate] packaged release scope: package-install-smoke + container-runtime-validation + container-validation-summary ({})",
                    mode.label()
                ));
            }
        }
        run_release_container_check(options.release_mode)?;
        checks.push("release_container_check".to_string());
    } else {
        validation_log("[release-gate] skipping packaged release validation");
    }

    if options.run_stack {
        validation_log("[release-gate] ----------------------------------------");
        validation_log("[release-gate] running three-module stack smoke");
        run_three_module_stack_smoke()?;
        checks.push("three_module_stack_smoke".to_string());
    } else {
        validation_log("[release-gate] skipping three-module stack smoke");
    }

    if options.run_pathology {
        validation_log("[release-gate] ----------------------------------------");
        validation_log("[release-gate] running pathological container validation");
        run_pathological_container_validation(None)?;
        checks.push("pathological_container_validation".to_string());
    } else {
        validation_log("[release-gate] skipping pathological container validation");
    }

    if options.run_remote_host {
        validation_log("[release-gate] ----------------------------------------");
        validation_log(format!(
            "[release-gate] running remote linux host validation ({})",
            options.remote_host
        ));
        let remote_report = run_remote_linux_host_validation(RemoteLinuxHostOptions {
            host: options.remote_host,
            remote_dir: options.remote_dir,
            build_packages: options.remote_build_packages,
            keep_remote_dir: options.keep_remote_dir,
        })?;
        checks.push("remote_linux_host_validation".to_string());
        print_remote_release_gate_summary(&remote_report.out_dir);
        if remote_report
            .checks
            .iter()
            .any(|check| check == "remote_ebpf_smoke")
        {
            validation_log("[release-gate] remote Linux eBPF attach evidence: ok");
            checks.push("remote_ebpf_smoke".to_string());
        } else if remote_report
            .checks
            .iter()
            .any(|check| check == "remote_ebpf_smoke_skipped")
        {
            validation_log(
                "[release-gate] remote Linux eBPF attach evidence: skipped (see remote-ebpf.txt)",
            );
            validation_log(
                "[release-gate] WARNING: remote Linux proof is partial; do not treat package/runtime-only confidence as a hidden green light",
            );
            checks.push("remote_ebpf_smoke_skipped".to_string());
        }
    } else {
        validation_log("[release-gate] skipping remote linux host validation");
    }

    validation_log("[release-gate] ----------------------------------------");
    validation_log("[release-gate] release gate: ok");

    Ok(ValidationReport {
        name: "release gate".to_string(),
        out_dir: repo_root().join("target").join("validation"),
        checks,
    })
}

fn run_step(
    prefix: &str,
    label: &str,
    script_relative_path: &str,
    args: &[&str],
) -> Result<(), ValidationError> {
    validation_log(format!(
        "[{prefix}] ----------------------------------------"
    ));
    validation_log(format!("[{prefix}] {label}"));
    run_repo_script(script_relative_path, args)
}

fn run_repo_script(script_relative_path: &str, args: &[&str]) -> Result<(), ValidationError> {
    let status = Command::new("bash")
        .current_dir(repo_root())
        .arg(repo_root().join(script_relative_path))
        .args(args)
        .stdin(Stdio::null())
        .stdout(validation_command_stdout())
        .stderr(Stdio::inherit())
        .status()
        .map_err(|err| {
            ValidationError::new(format!("failed to launch `{script_relative_path}`: {err}"))
        })?;

    if !status.success() {
        return Err(ValidationError::new(format!(
            "`{script_relative_path}` exited with status {status}"
        )));
    }

    Ok(())
}

fn print_remote_release_gate_summary(out_dir: &Path) {
    let run = parse_key_value_file(&out_dir.join("remote-run.txt"));
    let ebpf = parse_key_value_file(&out_dir.join("remote-ebpf.txt"));
    let timings = parse_phase_timings(&out_dir.join("remote-phase-timings.txt"));
    let recent = read_trimmed_lines(&out_dir.join("remote-ebpf-recent.txt"));
    let history_summary = parse_json_file(&out_dir.join("remote-ebpf-status-summary.json"));

    if let Some(remote_dir) = run.get("remote_dir") {
        validation_log(format!("[release-gate] remote dir: {remote_dir}"));
    }
    if let Some(status) = ebpf.get("status") {
        let reason = ebpf.get("reason").map(String::as_str).unwrap_or("unknown");
        validation_log(format!(
            "[release-gate] remote eBPF summary: {status} ({reason})"
        ));
    }
    let (validation_posture, release_gate_signal, next_step) =
        summarize_remote_release_gate_posture(&ebpf);
    validation_log(format!(
        "[release-gate] validation-posture: {validation_posture}"
    ));
    validation_log(format!(
        "[release-gate] release-gate-signal: {release_gate_signal}"
    ));
    validation_log(format!("[release-gate] next-step: {next_step}"));

    let mut slowest = timings
        .into_iter()
        .filter(|(name, _)| name != "total")
        .collect::<Vec<_>>();
    slowest.sort_by(|left, right| right.1.total_cmp(&left.1));
    if !slowest.is_empty() {
        let summary = slowest
            .iter()
            .take(3)
            .map(|(name, seconds)| format!("{name}={seconds:.3}s"))
            .collect::<Vec<_>>()
            .join(", ");
        validation_log(format!("[release-gate] remote slowest phases: {summary}"));
    }
    if let Some(trend) = summarize_recent_ebpf_trend(history_summary.as_ref()) {
        validation_log(format!("[release-gate] remote recent eBPF trend: {trend}"));
    }
    for line in recent.iter().take(3) {
        validation_log(format!("[release-gate] remote recent eBPF: {line}"));
    }
}

fn summarize_remote_release_gate_posture(
    ebpf: &BTreeMap<String, String>,
) -> (&'static str, &'static str, &'static str) {
    match ebpf.get("status").map(String::as_str) {
        Some("ok") => (
            "full",
            "ready",
            "hold this Linux host run as a release reference and watch later regressions against it",
        ),
        Some("skipped") => match ebpf.get("reason").map(String::as_str) {
            Some("sudo_not_available") => (
                "partial",
                "package_runtime_only",
                "rerun with sudo or GEWY_REMOTE_EBPF_ADMIN_USER / GEWY_REMOTE_EBPF_ADMIN_PASSWORD to prove Linux attach confidence before 1.0.0",
            ),
            Some("default_route_device_not_detected") => (
                "partial",
                "route_device_missing",
                "rerun on a host with a detectable default-route device so the tc attach proof can complete",
            ),
            _ => (
                "partial",
                "incomplete_linux_evidence",
                "inspect the remote eBPF reason and rerun once the missing Linux prerequisite is available",
            ),
        },
        _ => (
            "unknown",
            "needs_review",
            "inspect the remote evidence shelf before treating this release-gate run as a Linux reference",
        ),
    }
}

fn parse_key_value_file(path: &Path) -> BTreeMap<String, String> {
    let Ok(contents) = fs::read_to_string(path) else {
        return BTreeMap::new();
    };
    let mut values = BTreeMap::new();
    for line in contents.lines() {
        if let Some((key, value)) = line.split_once('=') {
            values.insert(key.to_string(), value.to_string());
        }
    }
    values
}

fn parse_phase_timings(path: &Path) -> Vec<(String, f64)> {
    parse_key_value_file(path)
        .into_iter()
        .filter_map(|(name, value)| value.parse::<f64>().ok().map(|seconds| (name, seconds)))
        .collect()
}

fn parse_json_file(path: &Path) -> Option<serde_json::Value> {
    let body = fs::read_to_string(path).ok()?;
    serde_json::from_str(&body).ok()
}

fn read_trimmed_lines(path: &Path) -> Vec<String> {
    fs::read_to_string(path)
        .ok()
        .map(|body| {
            body.lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .map(ToOwned::to_owned)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn summarize_recent_ebpf_trend(history_summary: Option<&serde_json::Value>) -> Option<String> {
    let history_summary = history_summary?;
    let entries = history_summary
        .get("entries")
        .and_then(|value| value.as_u64())?;
    let status_counts = history_summary.get("status_counts")?.as_object()?;
    let ok_count = status_counts
        .get("ok")
        .and_then(|value| value.as_u64())
        .unwrap_or_default();
    let skipped_count = status_counts
        .get("skipped")
        .and_then(|value| value.as_u64())
        .unwrap_or_default();
    Some(format!(
        "{ok_count}/{entries} ok, {skipped_count}/{entries} skipped"
    ))
}
