use std::process::{Command, Stdio};
use std::{collections::BTreeMap, fs, path::Path};

use serde_json::json;

use super::command::{ValidationError, ValidationReport, repo_root};
use super::{
    RemoteLinuxHostOptions, run_container_runtime_validation, run_container_validation_summary,
    run_debugger_cross_validation, run_package_install_smoke,
    run_pathological_container_validation, run_remote_linux_host_validation,
    run_three_module_stack_smoke, validation_command_stdout, validation_log,
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
    pub run_debugger_cross: bool,
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
            run_debugger_cross: true,
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

    if options.run_debugger_cross {
        validation_log("[release-gate] ----------------------------------------");
        validation_log("[release-gate] running debugger cross validation");
        run_debugger_cross_validation(None)?;
        checks.push("debugger_cross_validation".to_string());
    } else {
        validation_log("[release-gate] skipping debugger cross validation");
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

    write_release_artifact_index(&repo_root().join("target").join("validation"), &checks)?;

    validation_log("[release-gate] ----------------------------------------");
    validation_log(
        "[release-gate] release artifacts: target/validation/release-gate-artifacts.json",
    );
    validation_log(
        "[release-gate] release artifact summary: target/validation/release-gate-artifacts.txt",
    );
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

    let budget_warnings = remote_phase_budget_warnings(&timings);

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
    for warning in budget_warnings {
        validation_log(format!("[release-gate] remote budget warning: {warning}"));
    }
    if let Some(trend) = summarize_recent_ebpf_trend(history_summary.as_ref()) {
        validation_log(format!("[release-gate] remote recent eBPF trend: {trend}"));
    }
    for line in recent.iter().take(3) {
        validation_log(format!("[release-gate] remote recent eBPF: {line}"));
    }
}

fn write_release_artifact_index(out_dir: &Path, checks: &[String]) -> Result<(), ValidationError> {
    fs::create_dir_all(out_dir)?;

    let artifact_index_path = out_dir.join("release-gate-artifacts.json");
    let artifact_summary_path = out_dir.join("release-gate-artifacts.txt");

    let entries = vec![
        release_artifact_entry(
            "release_gate_root",
            "directory",
            out_dir,
            "required",
            Some(true),
            "release-gate",
            "shared root for release-facing validation evidence",
        ),
        release_artifact_entry(
            "debugger_cross_validation",
            "directory",
            &out_dir.join("debugger-cross-validation"),
            "required",
            Some(
                checks
                    .iter()
                    .any(|check| check == "debugger_cross_validation"),
            ),
            "gewyvern_validate debugger-cross",
            "cross-surface debugger evidence shelf",
        ),
        release_artifact_entry(
            "pathological_container_validation",
            "directory",
            &out_dir.join("pathological-container"),
            "required",
            Some(
                checks
                    .iter()
                    .any(|check| check == "pathological_container_validation"),
            ),
            "gewyvern_validate pathological-container-validation",
            "degraded-but-live resilience evidence shelf",
        ),
        release_artifact_entry(
            "three_module_resilience_summary",
            "file",
            &out_dir.join("resilience-summary.txt"),
            "required",
            Some(
                checks
                    .iter()
                    .any(|check| check == "three_module_stack_smoke"),
            ),
            "three_module_stack_smoke.sh",
            "compact resilience summary from the multi-project Docker smoke",
        ),
        release_artifact_entry(
            "remote_linux_host_validation",
            "directory",
            &out_dir.join("remote-linux-host-validation"),
            "optional",
            Some(
                checks
                    .iter()
                    .any(|check| check == "remote_linux_host_validation"),
            ),
            "gewyvern_validate remote-linux-host-validation",
            "structured Linux host proof shelf for package, runtime, and eBPF validation",
        ),
        release_artifact_entry(
            "remote_linux_ebpf",
            "directory",
            &out_dir
                .join("remote-linux-host-validation")
                .join("remote-ebpf"),
            "optional",
            Some(
                checks.iter().any(|check| {
                    check == "remote_ebpf_smoke" || check == "remote_ebpf_smoke_skipped"
                }),
            ),
            "gewyvern_validate remote-linux-host-validation",
            "nested remote eBPF attach evidence shelf when the remote Linux path runs",
        ),
        release_artifact_entry(
            "juice_shop_container_validation",
            "directory",
            &out_dir.join("juice-shop-container"),
            "optional_high_signal",
            None,
            "gewyvern_validate juice-shop-container-validation",
            "practical Linux target-lab shelf that preserves suspicious target-side HTTP evidence plus same-host attach proof",
        ),
        release_artifact_entry(
            "ftp_denied_container_validation",
            "directory",
            &out_dir.join("ftp-denied-container"),
            "optional_high_signal",
            None,
            "gewyvern_validate ftp-denied-container-validation",
            "practical Linux target-lab shelf that preserves client-side FTP 530 denial evidence, target-side FAIL LOGIN logs, and same-host attach proof",
        ),
        release_artifact_entry(
            "ldap_bind_denied_container_validation",
            "directory",
            &out_dir.join("ldap-bind-denied-container"),
            "optional_high_signal",
            None,
            "gewyvern_validate ldap-bind-denied-container-validation",
            "practical Linux target-lab shelf that preserves client-side LDAP err=49 denial evidence, target-side bind logs, and same-host attach proof",
        ),
        release_artifact_entry(
            "release_gate_artifact_index",
            "file",
            &artifact_index_path,
            "required",
            Some(true),
            "gewyvern_validate release-gate",
            "machine-readable release artifact index",
        ),
        release_artifact_entry(
            "release_gate_artifact_summary",
            "file",
            &artifact_summary_path,
            "required",
            Some(true),
            "gewyvern_validate release-gate",
            "human-readable release artifact summary",
        ),
    ];

    let payload = json!({
        "schema_version": 1,
        "kind": "release_artifact_index",
        "name": "release gate artifacts",
        "root": out_dir.display().to_string(),
        "artifacts": entries,
    });
    fs::write(
        &artifact_index_path,
        serde_json::to_string_pretty(&payload)?,
    )?;

    let summary = payload["artifacts"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|entry| {
            format!(
                "{} [{}] {} {}\n  path={}\n  producer={}\n  note={}",
                entry["key"].as_str().unwrap_or("unknown"),
                entry["expectation"].as_str().unwrap_or("unknown"),
                entry["status"].as_str().unwrap_or("unknown"),
                entry["kind"].as_str().unwrap_or("artifact"),
                entry["path"].as_str().unwrap_or(""),
                entry["producer"].as_str().unwrap_or(""),
                entry["note"].as_str().unwrap_or(""),
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(
        &artifact_summary_path,
        format!(
            "release gate artifacts: ok\nroot={}\nindex={}\nsummary={}\n\n{}\n",
            out_dir.display(),
            artifact_index_path.display(),
            artifact_summary_path.display(),
            summary
        ),
    )?;

    Ok(())
}

fn release_artifact_entry(
    key: &str,
    kind: &str,
    path: &Path,
    expectation: &str,
    stage_ran: Option<bool>,
    producer: &str,
    note: &str,
) -> serde_json::Value {
    let status = if key == "release_gate_artifact_index" || key == "release_gate_artifact_summary" {
        "present"
    } else if path.exists() {
        "present"
    } else if stage_ran == Some(false) {
        "not_run"
    } else {
        "absent"
    };

    json!({
        "key": key,
        "kind": kind,
        "path": path.display().to_string(),
        "status": status,
        "expectation": expectation,
        "producer": producer,
        "note": note,
    })
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

fn remote_phase_budget_warnings(timings: &[(String, f64)]) -> Vec<String> {
    const REMOTE_TOTAL_BUDGET_SECONDS: f64 = 45.0;
    const WORKSPACE_SYNC_BUDGET_SECONDS: f64 = 8.0;
    const REMOTE_PACKAGE_BUILD_BUDGET_SECONDS: f64 = 20.0;
    const REMOTE_PACKAGE_SMOKE_BUDGET_SECONDS: f64 = 2.0;
    const REMOTE_RUNTIME_SMOKE_BUDGET_SECONDS: f64 = 3.0;
    const REMOTE_EBPF_SMOKE_BUDGET_SECONDS: f64 = 10.0;
    const REMOTE_EBPF_SYNC_BUDGET_SECONDS: f64 = 5.0;

    timings
        .iter()
        .filter_map(|(name, seconds)| {
            let budget = match name.as_str() {
                "total" => Some(REMOTE_TOTAL_BUDGET_SECONDS),
                "workspace_sync" => Some(WORKSPACE_SYNC_BUDGET_SECONDS),
                "remote_package_build" => Some(REMOTE_PACKAGE_BUILD_BUDGET_SECONDS),
                "remote_package_smoke" => Some(REMOTE_PACKAGE_SMOKE_BUDGET_SECONDS),
                "remote_runtime_smoke" => Some(REMOTE_RUNTIME_SMOKE_BUDGET_SECONDS),
                "remote_ebpf_smoke" => Some(REMOTE_EBPF_SMOKE_BUDGET_SECONDS),
                "remote_ebpf_evidence_sync" => Some(REMOTE_EBPF_SYNC_BUDGET_SECONDS),
                _ => None,
            }?;
            (seconds > &budget)
                .then(|| format!("{name} exceeded budget ({seconds:.3}s > {budget:.3}s)"))
        })
        .collect()
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
