use std::process::{Command, Stdio};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use serde_json::json;

use super::command::{ValidationError, ValidationReport, default_out_dir, repo_root};
use super::{
    RemoteLinuxHostOptions, read_bounded_json_file, read_bounded_nonempty_lines,
    read_bounded_phase_timings, read_bounded_unique_key_value_file,
    run_container_runtime_validation, run_container_validation_summary,
    run_debugger_cross_validation, run_leserpent_parity_recovery_validation,
    run_leserpent_schema_freeze_validation, run_package_install_smoke,
    run_pathological_container_validation, run_remote_linux_host_validation,
    run_three_module_stack_smoke, validate_leserpent_control_plane_aot_evidence,
    validation_command_stdout, validation_log,
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
    pub run_leserpent_proof: bool,
    pub macos_release_preflight: Option<PathBuf>,
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
            run_leserpent_proof: false,
            macos_release_preflight: None,
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
    let install = run_package_install_smoke(mode)?;
    checks.push("package_install_smoke".to_string());

    validation_log("[release-check] ----------------------------------------");
    validation_log("[release-check] running packaged runtime validation");
    let runtime = run_container_runtime_validation(mode)?;
    checks.push("packaged_runtime_validation".to_string());

    validation_log("[release-check] ----------------------------------------");
    validation_log("[release-check] running packaged protocol/operator summary");
    let packaged_summary = run_container_validation_summary(mode)?;
    checks.push("packaged_protocol_operator_summary".to_string());

    validation_log("[release-check] ----------------------------------------");
    validation_log(format!(
        "[release-check] packaged release validation: ok ({})",
        mode.label()
    ));
    validation_log(
        "[release-check] covered packaged checks: package-install-smoke, container-runtime-validation, container-validation-summary",
    );

    let out_dir = default_out_dir("release-container-check");
    write_release_container_evidence(
        &out_dir,
        mode,
        &checks,
        &[
            ("package_install_smoke", &install.out_dir),
            ("container_runtime_validation", &runtime.out_dir),
            ("container_validation_summary", &packaged_summary.out_dir),
        ],
    )?;
    Ok(ValidationReport {
        name: format!("packaged release validation ({})", mode.label()),
        out_dir,
        checks,
    })
}

fn write_release_container_evidence(
    out_dir: &Path,
    mode: ReleaseCheckMode,
    checks: &[String],
    components: &[(&str, &Path)],
) -> Result<(), ValidationError> {
    fs::create_dir_all(out_dir)?;
    for name in ["summary.json", "evidence-index.json"] {
        let path = out_dir.join(name);
        if path.exists() || path.is_symlink() {
            fs::remove_file(path)?;
        }
    }
    let components = components
        .iter()
        .map(|(name, path)| {
            let evidence_dir = path
                .file_name()
                .and_then(|value| value.to_str())
                .ok_or_else(|| ValidationError::new("invalid release component evidence path"))?;
            Ok(json!({
                "name": name,
                "evidence_dir": format!("../{evidence_dir}"),
                "status": "ok",
            }))
        })
        .collect::<Result<Vec<_>, ValidationError>>()?;
    let summary = json!({
        "schema_version": 1,
        "command": "release-container-check",
        "status": "ok",
        "mode": mode.label(),
        "checks": checks,
        "components": components,
    });
    fs::write(
        out_dir.join("summary.json"),
        format!("{}\n", serde_json::to_string_pretty(&summary)?),
    )?;
    let index = json!({
        "schema_version": 1,
        "command": "release-container-check",
        "files": ["summary.json"],
    });
    fs::write(
        out_dir.join("evidence-index.json"),
        format!("{}\n", serde_json::to_string_pretty(&index)?),
    )?;
    Ok(())
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

    if options.run_leserpent_proof {
        validation_log("[release-gate] ----------------------------------------");
        validation_log("[release-gate] running Leserpent parity/recovery proof");
        run_leserpent_parity_recovery_validation(None)?;
        checks.push("leserpent_parity_recovery".to_string());
        validation_log("[release-gate] running Leserpent schema/scope freeze proof");
        run_leserpent_schema_freeze_validation(None)?;
        checks.push("leserpent_schema_freeze".to_string());
    } else {
        validation_log("[release-gate] skipping optional Leserpent combined proof shelves");
    }

    if let Some(path) = options.macos_release_preflight.as_deref() {
        validation_log("[release-gate] ----------------------------------------");
        validation_log(format!(
            "[release-gate] validating macOS release preflight ({})",
            path.display()
        ));
        let (status, report) = validate_macos_release_preflight(path)?;
        let evidence_dir = repo_root().join("target/validation/leserpent-macos-release-preflight");
        fs::create_dir_all(&evidence_dir)?;
        fs::write(
            evidence_dir.join("release-gate-preflight.json"),
            serde_json::to_string_pretty(&report)?,
        )?;
        checks.push(status.check_name().to_string());
        validation_log(format!(
            "[release-gate] macOS release preflight: {}",
            status.label()
        ));
    } else {
        validation_log("[release-gate] skipping optional macOS release preflight");
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
        print_remote_release_gate_summary(&remote_report.out_dir)?;
        if remote_report
            .checks
            .iter()
            .any(|check| check == "remote_leserpent_control_plane_aot")
        {
            checks.push("remote_leserpent_control_plane_aot".to_string());
        }
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

fn print_remote_release_gate_summary(out_dir: &Path) -> Result<(), ValidationError> {
    let run = read_bounded_unique_key_value_file(
        &out_dir.join("remote-run.txt"),
        "release-gate remote run evidence",
        &[
            "host",
            "remote_dir",
            "build_packages",
            "keep_remote_dir",
            "checks",
        ],
    )?;
    let ebpf = read_bounded_unique_key_value_file(
        &out_dir.join("remote-ebpf.txt"),
        "release-gate remote eBPF evidence",
        &["status", "reason", "default_route_device"],
    )?;
    let timings = read_bounded_phase_timings(
        &out_dir.join("remote-phase-timings.txt"),
        "release-gate remote phase timings",
        &[
            "remote_preflight",
            "remote_workspace_create",
            "workspace_sync",
            "remote_workspace_materialize",
            "remote_rust_quality",
            "remote_linux_target_check",
            "remote_package_build",
            "remote_leserpent_control_plane_aot",
            "remote_artifact_verify",
            "remote_package_smoke",
            "remote_runtime_smoke",
            "remote_ebpf_validator_build",
            "remote_ebpf_attach",
            "remote_ebpf_smoke",
            "remote_ebpf_evidence_sync",
            "remote_workspace_cleanup",
            "total",
        ],
        &["total"],
    )?;
    let recent = read_bounded_nonempty_lines(
        &out_dir.join("remote-ebpf-recent.txt"),
        "release-gate remote eBPF recent evidence",
        16 * 1024,
        5,
        512,
    )?;
    let history_summary = read_bounded_json_file(
        &out_dir.join("remote-ebpf-status-summary.json"),
        "release-gate remote eBPF history summary",
        64 * 1024,
    )?;

    require_evidence_keys(&run, &["remote_dir"], "release-gate remote run evidence")?;
    require_evidence_keys(
        &ebpf,
        &["status", "reason", "default_route_device"],
        "release-gate remote eBPF evidence",
    )?;
    let aot_evidence_covered = run.get("checks").is_some_and(|checks| {
        checks
            .split(',')
            .any(|check| check == "remote_leserpent_control_plane_aot")
    });
    if aot_evidence_covered {
        validate_leserpent_control_plane_aot_evidence(
            &out_dir.join("leserpent-control-plane-aot-linux-x64"),
        )?;
        validation_log("[release-gate] Leserpent control-plane NativeAOT evidence: validated");
    }

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
        summarize_remote_release_gate_posture(&ebpf, Some(&history_summary));
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
    if let Some(integrity) = history_summary.get("integrity") {
        validation_log(format!(
            "[release-gate] remote history integrity: {}",
            integrity
                .get("status")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("unknown")
        ));
    }
    if let Some(trend) = summarize_recent_ebpf_trend(Some(&history_summary)) {
        validation_log(format!("[release-gate] remote recent eBPF trend: {trend}"));
    }
    for line in recent.iter().take(3) {
        validation_log(format!("[release-gate] remote recent eBPF: {line}"));
    }
    Ok(())
}

fn require_evidence_keys(
    values: &BTreeMap<String, String>,
    required_keys: &[&str],
    context: &str,
) -> Result<(), ValidationError> {
    for key in required_keys {
        if !values.contains_key(*key) {
            return Err(ValidationError::new(format!("{context} missing {key}")));
        }
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MacosReleasePreflightStatus {
    Ready,
    Blocked,
}

impl MacosReleasePreflightStatus {
    fn check_name(self) -> &'static str {
        match self {
            Self::Ready => "macos_release_preflight_ready",
            Self::Blocked => "macos_release_preflight_blocked",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::Blocked => "blocked",
        }
    }
}

fn validate_macos_release_preflight(
    path: &Path,
) -> Result<(MacosReleasePreflightStatus, serde_json::Value), ValidationError> {
    const MAX_PREFLIGHT_BYTES: u64 = 16 * 1024;
    const EXPECTED_TOOLS: [&str; 8] = [
        "codesign",
        "ditto",
        "notarytool",
        "plutil",
        "security",
        "spctl",
        "stapler",
        "xcrun",
    ];
    const EXPECTED_FIELDS: [&str; 16] = [
        "app",
        "app_executable_sha256",
        "apple_tools",
        "blockers",
        "developer_id_application_identities",
        "daemon_executable_sha256",
        "entitlements_sha256",
        "host_arch",
        "notary_profile_requested",
        "notary_profile_valid",
        "platform",
        "proof",
        "release_ready",
        "result",
        "schema_version",
        "version",
    ];

    let report = read_bounded_json_file(path, "macOS release preflight", MAX_PREFLIGHT_BYTES)?;
    let object = report
        .as_object()
        .ok_or_else(|| ValidationError::new("macOS release preflight must be a JSON object"))?;
    let field_names = object.keys().map(String::as_str).collect::<BTreeSet<_>>();
    if field_names != EXPECTED_FIELDS.into_iter().collect() {
        return Err(ValidationError::new(
            "macOS release preflight contains missing or unknown fields",
        ));
    }
    let require_string = |key: &str| {
        object
            .get(key)
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| ValidationError::new(format!("macOS release preflight missing {key}")))
    };
    let require_bool = |key: &str| {
        object
            .get(key)
            .and_then(serde_json::Value::as_bool)
            .ok_or_else(|| ValidationError::new(format!("macOS release preflight missing {key}")))
    };

    if object
        .get("schema_version")
        .and_then(serde_json::Value::as_u64)
        != Some(2)
        || require_string("proof")? != "leserpent-macos-release-preflight"
        || require_string("platform")? != "macos"
        || require_string("version")? != env!("CARGO_PKG_VERSION")
    {
        return Err(ValidationError::new(
            "macOS release preflight identity or version does not match this release",
        ));
    }
    for key in ["app", "host_arch"] {
        let value = require_string(key)?;
        if value.is_empty()
            || value.len() > 128
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_graphic() || byte == b' ')
        {
            return Err(ValidationError::new(format!(
                "macOS release preflight {key} is unsafe or unbounded"
            )));
        }
    }
    for key in [
        "app_executable_sha256",
        "daemon_executable_sha256",
        "entitlements_sha256",
    ] {
        let hash = require_string(key)?;
        if hash.len() != 64 || !hash.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(ValidationError::new(format!(
                "macOS release preflight {key} is not a SHA-256 digest"
            )));
        }
    }

    let tools = object
        .get("apple_tools")
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| ValidationError::new("macOS release preflight missing apple_tools"))?;
    let tool_names = tools.keys().map(String::as_str).collect::<BTreeSet<_>>();
    if tool_names != EXPECTED_TOOLS.into_iter().collect()
        || tools.values().any(|value| value.as_bool().is_none())
    {
        return Err(ValidationError::new(
            "macOS release preflight Apple tool inventory is incomplete or invalid",
        ));
    }
    let tools_ready = tools.values().all(|value| value.as_bool() == Some(true));
    let identities = object
        .get("developer_id_application_identities")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| {
            ValidationError::new(
                "macOS release preflight missing developer_id_application_identities",
            )
        })?;
    let profile_requested = require_bool("notary_profile_requested")?;
    let profile_valid = require_bool("notary_profile_valid")?;
    let release_ready = require_bool("release_ready")?;
    let result = require_string("result")?;
    let blockers = object
        .get("blockers")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| ValidationError::new("macOS release preflight missing blockers"))?
        .iter()
        .map(|value| {
            value.as_str().ok_or_else(|| {
                ValidationError::new("macOS release preflight blocker must be a string")
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut expected_blockers = Vec::new();
    if !tools_ready {
        expected_blockers.push("apple_release_tool_missing");
    }
    if identities == 0 {
        expected_blockers.push("developer_id_application_identity_missing");
    }
    if !profile_requested {
        expected_blockers.push("notary_keychain_profile_not_requested");
    } else if !profile_valid {
        expected_blockers.push("notary_keychain_profile_unavailable");
    }
    if blockers != expected_blockers {
        return Err(ValidationError::new(
            "macOS release preflight blockers do not match its readiness fields",
        ));
    }
    let expected_ready = expected_blockers.is_empty();
    if release_ready != expected_ready || result != if expected_ready { "ready" } else { "blocked" }
    {
        return Err(ValidationError::new(
            "macOS release preflight result contradicts its readiness fields",
        ));
    }

    Ok((
        if expected_ready {
            MacosReleasePreflightStatus::Ready
        } else {
            MacosReleasePreflightStatus::Blocked
        },
        report,
    ))
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
            "remote_leserpent_control_plane_aot",
            "directory",
            &out_dir
                .join("remote-linux-host-validation")
                .join("leserpent-control-plane-aot-linux-x64"),
            "optional_high_signal",
            Some(
                checks
                    .iter()
                    .any(|check| check == "remote_leserpent_control_plane_aot"),
            ),
            "gewyvern_validate remote-linux-host-validation",
            "strictly revalidated Linux x64 NativeAOT control-plane, persistence, registration, and recovery evidence shelf",
        ),
        release_artifact_entry(
            "leserpent_parity_recovery",
            "directory",
            &out_dir.join("leserpent-parity-recovery"),
            "optional_high_signal",
            Some(
                checks
                    .iter()
                    .any(|check| check == "leserpent_parity_recovery"),
            ),
            "gewyvern_validate release-gate --leserpent-proof",
            "opt-in 13-suite Rust, xUnit, GUI, mobile, and cross-language parity/recovery shelf",
        ),
        release_artifact_entry(
            "leserpent_schema_freeze",
            "directory",
            &out_dir.join("leserpent-schema-freeze"),
            "optional_high_signal",
            Some(
                checks
                    .iter()
                    .any(|check| check == "leserpent_schema_freeze"),
            ),
            "gewyvern_validate release-gate --leserpent-proof",
            "opt-in versioned schema compatibility and closed 2.0 capability-scope shelf",
        ),
        release_artifact_entry(
            "leserpent_macos_release_preflight",
            "file",
            &out_dir
                .join("leserpent-macos-release-preflight")
                .join("release-gate-preflight.json"),
            "optional_blocking",
            Some(checks.iter().any(|check| {
                check == "macos_release_preflight_ready"
                    || check == "macos_release_preflight_blocked"
            })),
            "gewyvern_validate release-gate --macos-release-preflight FILE",
            "strict Apple release readiness evidence; a valid blocked report remains non-shippable",
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
    } else if stage_ran == Some(false) {
        "not_run"
    } else if path.exists() {
        "present"
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
    history_summary: Option<&serde_json::Value>,
) -> (&'static str, &'static str, &'static str) {
    let has_history_integrity_warning = history_summary
        .and_then(|value| value.get("integrity"))
        .and_then(|value| value.get("status"))
        .and_then(serde_json::Value::as_str)
        .is_some_and(|status| status != "clean");
    let has_matrix_coverage_gap = history_summary
        .and_then(|value| value.get("matrix"))
        .and_then(|value| value.get("ready"))
        .and_then(serde_json::Value::as_bool)
        == Some(false);
    match ebpf.get("status").map(String::as_str) {
        Some("ok") if has_history_integrity_warning => (
            "full",
            "watch",
            "inspect remote-ebpf-history-rejected.jsonl before treating this Linux history as a clean release reference",
        ),
        Some("ok") if has_matrix_coverage_gap => (
            "full",
            "coverage_incomplete",
            "collect successful evidence from at least two physical hosts and two kernel releases before treating the Linux matrix as release-ready",
        ),
        Some("ok") => (
            "full",
            "ready",
            "hold this Linux host run as a release reference and watch later regressions against it",
        ),
        Some("skipped") => match ebpf.get("reason").map(String::as_str) {
            Some("sudo_not_available") => (
                "partial",
                "package_runtime_only",
                "rerun with sudo or GEWY_REMOTE_EBPF_ADMIN_USER / GEWY_REMOTE_EBPF_ADMIN_PASSWORD to prove Linux attach confidence for the current release",
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
    const REMOTE_TOTAL_BUDGET_SECONDS: f64 = 180.0;
    const WORKSPACE_SYNC_BUDGET_SECONDS: f64 = 8.0;
    const REMOTE_PACKAGE_BUILD_BUDGET_SECONDS: f64 = 20.0;
    const REMOTE_LESERPENT_CONTROL_PLANE_AOT_BUDGET_SECONDS: f64 = 120.0;
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
                "remote_leserpent_control_plane_aot" => {
                    Some(REMOTE_LESERPENT_CONTROL_PLANE_AOT_BUDGET_SECONDS)
                }
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

#[cfg(test)]
mod tests {
    use super::{
        MacosReleasePreflightStatus, print_remote_release_gate_summary, release_artifact_entry,
        summarize_remote_release_gate_posture, validate_macos_release_preflight,
        write_release_artifact_index,
    };
    use std::collections::BTreeMap;

    fn write_preflight_fixture(name: &str, value: &serde_json::Value) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "gewyvern-macos-preflight-{name}-{}.json",
            std::process::id()
        ));
        std::fs::write(&path, serde_json::to_vec(value).unwrap()).unwrap();
        path
    }

    fn blocked_preflight_fixture() -> serde_json::Value {
        serde_json::from_str(include_str!(
            "../../docs/fixtures/leserpent_macos_release_preflight.json"
        ))
        .unwrap()
    }

    #[test]
    fn macos_preflight_accepts_consistent_ready_and_blocked_reports() {
        let blocked = blocked_preflight_fixture();
        let blocked_path = write_preflight_fixture("blocked", &blocked);
        assert_eq!(
            validate_macos_release_preflight(&blocked_path).unwrap().0,
            MacosReleasePreflightStatus::Blocked
        );

        let mut ready = blocked;
        ready["developer_id_application_identities"] = serde_json::json!(1);
        ready["notary_profile_requested"] = serde_json::json!(true);
        ready["notary_profile_valid"] = serde_json::json!(true);
        ready["release_ready"] = serde_json::json!(true);
        ready["blockers"] = serde_json::json!([]);
        ready["result"] = serde_json::json!("ready");
        let ready_path = write_preflight_fixture("ready", &ready);
        assert_eq!(
            validate_macos_release_preflight(&ready_path).unwrap().0,
            MacosReleasePreflightStatus::Ready
        );

        std::fs::remove_file(blocked_path).unwrap();
        std::fs::remove_file(ready_path).unwrap();
    }

    #[test]
    fn macos_preflight_rejects_contradictions_and_tool_inventory_drift() {
        let mut contradictory = blocked_preflight_fixture();
        contradictory["release_ready"] = serde_json::json!(true);
        let contradictory_path = write_preflight_fixture("contradictory", &contradictory);
        assert!(validate_macos_release_preflight(&contradictory_path).is_err());

        let mut drifted = blocked_preflight_fixture();
        drifted["apple_tools"]["unexpected"] = serde_json::json!(true);
        let drifted_path = write_preflight_fixture("drifted", &drifted);
        assert!(validate_macos_release_preflight(&drifted_path).is_err());

        let mut secret_bearing = blocked_preflight_fixture();
        secret_bearing["password"] = serde_json::json!("must-not-be-copied");
        let secret_path = write_preflight_fixture("secret", &secret_bearing);
        assert!(validate_macos_release_preflight(&secret_path).is_err());

        std::fs::remove_file(contradictory_path).unwrap();
        std::fs::remove_file(drifted_path).unwrap();
        std::fs::remove_file(secret_path).unwrap();
    }

    #[test]
    fn clean_remote_history_keeps_successful_release_signal_ready() {
        let ebpf = BTreeMap::from([("status".to_string(), "ok".to_string())]);
        let history = serde_json::json!({"integrity": {"status": "clean"}});

        let (_, signal, _) = summarize_remote_release_gate_posture(&ebpf, Some(&history));
        assert_eq!(signal, "ready");
    }

    #[test]
    fn repaired_remote_history_downgrades_successful_release_signal() {
        let ebpf = BTreeMap::from([("status".to_string(), "ok".to_string())]);
        let history = serde_json::json!({"integrity": {"status": "repaired"}});

        let (_, signal, next_step) = summarize_remote_release_gate_posture(&ebpf, Some(&history));
        assert_eq!(signal, "watch");
        assert!(next_step.contains("remote-ebpf-history-rejected.jsonl"));
    }

    #[test]
    fn incomplete_physical_matrix_blocks_ready_release_signal() {
        let ebpf = BTreeMap::from([("status".to_string(), "ok".to_string())]);
        let history = serde_json::json!({
            "integrity": {"status": "clean"},
            "matrix": {"ready": false}
        });

        let (posture, signal, next_step) =
            summarize_remote_release_gate_posture(&ebpf, Some(&history));
        assert_eq!(posture, "full");
        assert_eq!(signal, "coverage_incomplete");
        assert!(next_step.contains("two physical hosts"));
    }

    #[test]
    fn artifact_index_does_not_present_stale_files_as_current_stage_evidence() {
        let path = std::env::temp_dir().join(format!(
            "gewyvern-stale-release-artifact-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&path).unwrap();

        let skipped = release_artifact_entry(
            "stage",
            "directory",
            &path,
            "optional",
            Some(false),
            "test",
            "test",
        );
        let current = release_artifact_entry(
            "stage",
            "directory",
            &path,
            "optional",
            Some(true),
            "test",
            "test",
        );

        assert_eq!(skipped["status"], "not_run");
        assert_eq!(current["status"], "present");
        std::fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn artifact_index_records_current_schema_scope_freeze_stage() {
        let out_dir = std::env::temp_dir().join(format!(
            "gewyvern-schema-scope-release-artifact-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&out_dir);
        std::fs::create_dir_all(out_dir.join("leserpent-schema-freeze")).unwrap();
        write_release_artifact_index(&out_dir, &["leserpent_schema_freeze".to_string()]).unwrap();

        let payload: serde_json::Value = serde_json::from_slice(
            &std::fs::read(out_dir.join("release-gate-artifacts.json")).unwrap(),
        )
        .unwrap();
        let entry = payload["artifacts"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["key"] == "leserpent_schema_freeze")
            .expect("schema/scope freeze artifact must be indexed");
        assert_eq!(entry["status"], "present");
        assert_eq!(entry["expectation"], "optional_high_signal");
        std::fs::remove_dir_all(out_dir).unwrap();
    }

    #[test]
    fn remote_summary_rejects_ambiguous_evidence_before_printing_success() {
        let out_dir = std::env::temp_dir().join(format!(
            "gewyvern-release-summary-evidence-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&out_dir);
        std::fs::create_dir_all(&out_dir).unwrap();
        std::fs::write(
            out_dir.join("remote-run.txt"),
            "host=linux\nremote_dir=/tmp/gewyvern\nbuild_packages=true\nkeep_remote_dir=false\nchecks=ok\n",
        )
        .unwrap();
        std::fs::write(
            out_dir.join("remote-ebpf.txt"),
            "status=ok\nreason=all_smokes_passed\ndefault_route_device=eth0\n",
        )
        .unwrap();
        std::fs::write(out_dir.join("remote-phase-timings.txt"), "total=1.0\n").unwrap();
        std::fs::write(out_dir.join("remote-ebpf-recent.txt"), "recent evidence\n").unwrap();
        std::fs::write(
            out_dir.join("remote-ebpf-status-summary.json"),
            r#"{"entries":1,"status_counts":{"ok":1},"integrity":{"status":"clean"},"matrix":{"ready":true}}"#,
        )
        .unwrap();

        assert!(print_remote_release_gate_summary(&out_dir).is_ok());
        std::fs::write(
            out_dir.join("remote-ebpf.txt"),
            "status=ok\nstatus=skipped\nreason=ambiguous\ndefault_route_device=eth0\n",
        )
        .unwrap();
        assert!(print_remote_release_gate_summary(&out_dir).is_err());
        std::fs::remove_dir_all(out_dir).unwrap();
    }
}
