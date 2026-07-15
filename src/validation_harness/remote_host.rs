use std::env;
use std::ffi::OsString;
use std::fs;
use std::io::Write;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use serde_json::json;

use super::command::{
    ValidationError, ValidationReport, default_out_dir, repo_root, validation_command_stdout,
    validation_log,
};

static EVIDENCE_TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);
static REMOTE_RUN_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RemoteLinuxHostOptions {
    pub host: String,
    pub remote_dir: Option<String>,
    pub build_packages: bool,
    pub keep_remote_dir: bool,
}

impl Default for RemoteLinuxHostOptions {
    fn default() -> Self {
        Self {
            host: env::var("GEWY_REMOTE_HOST").unwrap_or_else(|_| "kyuubiki-lab".to_string()),
            remote_dir: None,
            build_packages: true,
            keep_remote_dir: false,
        }
    }
}

pub fn run_remote_linux_host_validation(
    options: RemoteLinuxHostOptions,
) -> Result<ValidationReport, ValidationError> {
    validate_remote_host(&options.host)?;
    require_cmd("ssh")?;
    require_cmd("rsync")?;
    let admin_auth = remote_ebpf_admin_auth()?;
    ensure_ssh_control_master(&options.host, admin_auth.as_ref())?;

    let out_dir = default_out_dir("remote-linux-host-validation");
    fs::create_dir_all(&out_dir)?;
    let _run_lock = acquire_remote_validation_run_lock(&out_dir)?;
    let mut phase_timings = Vec::new();

    let remote_dir = options
        .remote_dir
        .clone()
        .unwrap_or_else(default_remote_dir);
    let remote_path = remote_workspace_path(&remote_dir);
    let release_line = env::var("GEWY_RELEASE_LINE").unwrap_or_else(|_| "v1.0.0".to_string());

    validation_log(format!("[remote-host] host: {}", options.host));
    validation_log(format!(
        "[remote-host] requested remote workspace: {}",
        remote_path
    ));
    validation_log(format!(
        "[remote-host] build packages: {}",
        options.build_packages
    ));
    validation_log(format!(
        "[remote-host] keep remote dir: {}",
        options.keep_remote_dir
    ));

    let result: Result<ValidationReport, ValidationError> = (|| {
        validation_log("[remote-host] ----------------------------------------");
        validation_log("[remote-host] collecting remote preflight");
        let preflight = measure_phase(&mut phase_timings, "remote_preflight", || {
            collect_remote_preflight(admin_auth.as_ref(), &options.host, options.build_packages)
        })?;
        fs::write(out_dir.join("remote-preflight.txt"), preflight.render())?;
        let resolved_remote_path =
            resolve_remote_workspace_path(&remote_path, &preflight.home_dir)?;
        let remote_source_cache = remote_source_cache_dir(&preflight.home_dir);
        let validation_workspace = remote_source_cache.clone();
        let remote_source_cache_quoted = shell_single_quote(&remote_source_cache);
        let remote_parent_dir = Path::new(&resolved_remote_path)
            .parent()
            .and_then(|path| path.to_str())
            .unwrap_or(&preflight.home_dir)
            .to_string();
        let remote_parent_dir_quoted = shell_single_quote(&remote_parent_dir);
        validation_log(format!(
            "[remote-host] resolved remote workspace: {}",
            resolved_remote_path
        ));
        validation_log(format!(
            "[remote-host] remote source cache: {}",
            remote_source_cache
        ));
        validation_log(format!(
            "[remote-host] remote cargo target cache: {}",
            remote_cargo_target_dir(&preflight.home_dir)
        ));

        validation_log("[remote-host] ----------------------------------------");
        validation_log("[remote-host] creating remote workspace roots");
        measure_phase(&mut phase_timings, "remote_workspace_create", || {
            run_ssh_command(
                admin_auth.as_ref(),
                &options.host,
                &format!("mkdir -p {remote_source_cache_quoted} {remote_parent_dir_quoted}"),
                "failed to create remote workspace roots",
            )
        })?;

        validation_log("[remote-host] ----------------------------------------");
        validation_log("[remote-host] syncing current workspace into remote source cache");
        measure_phase(&mut phase_timings, "workspace_sync", || {
            let workspace_sync_key = compute_local_workspace_sync_key()?;
            sync_workspace(
                admin_auth.as_ref(),
                &options.host,
                &remote_source_cache,
                &workspace_sync_key,
            )
        })?;

        validation_log("[remote-host] ----------------------------------------");
        validation_log("[remote-host] materializing remote workspace from source cache");
        measure_phase(&mut phase_timings, "remote_workspace_materialize", || {
            materialize_remote_workspace(
                admin_auth.as_ref(),
                &options.host,
                &remote_source_cache,
                &resolved_remote_path,
            )
        })?;

        let mut checks = vec!["workspace_synced".to_string()];
        checks.insert(0, "remote_preflight".to_string());
        checks.push("remote_workspace_materialized".to_string());

        if options.build_packages {
            validation_log("[remote-host] ----------------------------------------");
            validation_log("[remote-host] building x86_64 packages on remote host");
            measure_phase(&mut phase_timings, "remote_package_build", || {
                let target_dir = shell_single_quote(&remote_cargo_target_dir(&preflight.home_dir));
                run_ssh_command(
                    admin_auth.as_ref(),
                    &options.host,
                    &format!(
                        "mkdir -p {target_dir} && cd {validation_workspace} && CARGO_TARGET_DIR={target_dir} ./scripts/packaging/build_packages.sh --format all"
                    ),
                    "remote package build failed",
                )
            })?;
            checks.push("remote_package_build".to_string());
        } else {
            validation_log("[remote-host] skipping remote package build");
        }

        validation_log("[remote-host] ----------------------------------------");
        validation_log("[remote-host] verifying remote package artifacts");
        let artifact_manifest =
            measure_phase(&mut phase_timings, "remote_artifact_verify", || {
                collect_remote_artifact_manifest(
                    admin_auth.as_ref(),
                    &options.host,
                    &validation_workspace,
                )
            })?;
        fs::write(
            out_dir.join("remote-artifacts.txt"),
            artifact_manifest.render(),
        )?;
        if options.build_packages {
            if let Ok(timings) = collect_remote_package_build_timings(
                admin_auth.as_ref(),
                &options.host,
                &validation_workspace,
            ) {
                fs::write(out_dir.join("remote-package-build-timings.txt"), timings)?;
                checks.push("remote_package_build_timings".to_string());
            }
        }
        checks.push("remote_artifacts_present".to_string());

        validation_log("[remote-host] ----------------------------------------");
        validation_log("[remote-host] running remote package smoke");
        measure_phase(&mut phase_timings, "remote_package_smoke", || {
            run_ssh_script(
                admin_auth.as_ref(),
                &options.host,
                &format!("cd {validation_workspace} && bash -s"),
                &remote_package_smoke_script(&release_line),
                "remote package smoke failed",
            )
        })?;
        checks.push("remote_package_smoke".to_string());
        if let Ok(timings) = collect_remote_package_smoke_timings(
            admin_auth.as_ref(),
            &options.host,
            &validation_workspace,
        ) {
            fs::write(out_dir.join("remote-package-smoke-timings.txt"), timings)?;
            checks.push("remote_package_smoke_timings".to_string());
        }

        validation_log("[remote-host] ----------------------------------------");
        validation_log("[remote-host] running remote runtime smoke");
        measure_phase(&mut phase_timings, "remote_runtime_smoke", || {
            run_ssh_script(
                admin_auth.as_ref(),
                &options.host,
                &format!("cd {validation_workspace} && bash -s"),
                REMOTE_RUNTIME_SMOKE_SCRIPT,
                "remote runtime smoke failed",
            )
        })?;
        checks.push("remote_runtime_smoke".to_string());
        if let Ok(timings) = collect_remote_runtime_smoke_timings(
            admin_auth.as_ref(),
            &options.host,
            &validation_workspace,
        ) {
            fs::write(out_dir.join("remote-runtime-smoke-timings.txt"), timings)?;
            checks.push("remote_runtime_smoke_timings".to_string());
        }

        validation_log("[remote-host] ----------------------------------------");
        validation_log("[remote-host] collecting remote eBPF smoke evidence");
        let ebpf_evidence = collect_remote_ebpf_evidence(
            &options.host,
            &validation_workspace,
            &preflight,
            admin_auth.as_ref(),
            &mut phase_timings,
        )?;
        fs::write(out_dir.join("remote-ebpf.txt"), ebpf_evidence.render())?;
        if ebpf_evidence.status == "ok" {
            validation_log("[remote-host] syncing remote eBPF evidence");
            measure_phase(&mut phase_timings, "remote_ebpf_evidence_sync", || {
                sync_remote_ebpf_evidence(
                    admin_auth.as_ref(),
                    &options.host,
                    &validation_workspace,
                    &preflight.home_dir,
                    &out_dir,
                )
            })?;
            checks.push("remote_ebpf_evidence_synced".to_string());
        }
        checks.push(match ebpf_evidence.status.as_str() {
            "ok" => "remote_ebpf_smoke".to_string(),
            _ => "remote_ebpf_smoke_skipped".to_string(),
        });
        fs::write(
            out_dir.join("remote-phase-timings.txt"),
            render_phase_timings(&phase_timings),
        )?;
        checks.push("remote_phase_timings".to_string());

        let summary = format!(
            "host={}\nremote_dir={}\nbuild_packages={}\nkeep_remote_dir={}\nchecks={}\n",
            options.host,
            resolved_remote_path,
            options.build_packages,
            options.keep_remote_dir,
            checks.join(",")
        );
        fs::write(out_dir.join("remote-run.txt"), summary)?;

        if options.keep_remote_dir {
            validation_log(format!(
                "[remote-host] keeping remote workspace: {}",
                remote_path
            ));
        } else {
            validation_log("[remote-host] ----------------------------------------");
            validation_log("[remote-host] removing remote workspace");
            measure_phase(&mut phase_timings, "remote_workspace_cleanup", || {
                remove_remote_workspace(
                    &options.host,
                    &resolved_remote_path,
                    &preflight.home_dir,
                    admin_auth.as_ref(),
                )
            })?;
            fs::write(
                out_dir.join("remote-phase-timings.txt"),
                render_phase_timings(&phase_timings),
            )?;
        }
        write_remote_ebpf_history(
            &out_dir,
            &options,
            &preflight,
            &ebpf_evidence,
            &phase_timings,
            &checks,
        )?;

        Ok(ValidationReport {
            name: format!("remote linux host validation ({})", options.host),
            out_dir,
            checks,
        })
    })();

    let close_master_result = close_ssh_control_master(&options.host, admin_auth.as_ref());
    if let Err(err) = &close_master_result {
        validation_log(format!(
            "[remote-host] ssh control master cleanup skipped: {}",
            err
        ));
    }

    result.map_err(|err: ValidationError| {
        ValidationError::new(format!(
            "{err}\nremote workspace retained at {}:{}",
            options.host, remote_path
        ))
    })
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct PhaseTiming {
    name: String,
    elapsed: Duration,
}

fn measure_phase<T>(
    phase_timings: &mut Vec<PhaseTiming>,
    name: &str,
    operation: impl FnOnce() -> Result<T, ValidationError>,
) -> Result<T, ValidationError> {
    let started_at = Instant::now();
    let result = operation();
    phase_timings.push(PhaseTiming {
        name: name.to_string(),
        elapsed: started_at.elapsed(),
    });
    result
}

fn render_phase_timings(phase_timings: &[PhaseTiming]) -> String {
    let total = phase_timings
        .iter()
        .fold(Duration::ZERO, |acc, timing| acc + timing.elapsed);
    let mut lines = Vec::with_capacity(phase_timings.len() + 1);
    for timing in phase_timings {
        lines.push(format!(
            "{}={:.3}",
            timing.name,
            timing.elapsed.as_secs_f64()
        ));
    }
    lines.push(format!("total={:.3}", total.as_secs_f64()));
    format!("{}\n", lines.join("\n"))
}

fn write_remote_ebpf_history(
    out_dir: &Path,
    options: &RemoteLinuxHostOptions,
    preflight: &RemotePreflight,
    ebpf_evidence: &RemoteEbpfEvidence,
    phase_timings: &[PhaseTiming],
    checks: &[String],
) -> Result<(), ValidationError> {
    const HISTORY_RETENTION: usize = 32;

    let _history_lock = acquire_remote_ebpf_history_lock(out_dir)?;
    let history_path = out_dir.join("remote-ebpf-history.jsonl");
    let rejected_path = out_dir.join("remote-ebpf-history-rejected.jsonl");
    let latest_path = out_dir.join("remote-ebpf-latest.json");
    let recent_path = out_dir.join("remote-ebpf-recent.txt");
    let summary_path = out_dir.join("remote-ebpf-status-summary.json");
    let observed_at_unix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let total_seconds = phase_timings
        .iter()
        .fold(Duration::ZERO, |acc, timing| acc + timing.elapsed)
        .as_secs_f64();
    let phase_timings_json = phase_timings
        .iter()
        .map(|timing| (timing.name.clone(), json!(timing.elapsed.as_secs_f64())))
        .collect::<serde_json::Map<String, serde_json::Value>>();

    let entry = json!({
        "schema_version": 1,
        "observed_at_unix": observed_at_unix,
        "host": options.host,
        "build_packages": options.build_packages,
        "keep_remote_dir": options.keep_remote_dir,
        "preflight": {
            "os": preflight.os,
            "arch": preflight.arch,
            "kernel": preflight.kernel,
            "host_fingerprint": preflight.host_fingerprint,
            "sudo_available": preflight.sudo_available,
            "default_route_device": preflight.default_route_device,
        },
        "ebpf": {
            "status": ebpf_evidence.status,
            "reason": ebpf_evidence.reason,
            "default_route_device": ebpf_evidence.default_route_device,
        },
        "checks": checks,
        "total_seconds": total_seconds,
        "phase_timings": phase_timings_json,
    });

    let (mut lines, rejected) = read_remote_ebpf_history(&history_path)?;
    lines.push(serde_json::to_string(&entry)?);
    if lines.len() > HISTORY_RETENTION {
        lines.drain(0..(lines.len() - HISTORY_RETENTION));
    }
    let rejected_entries = append_rejected_history(&rejected_path, &rejected, observed_at_unix)?;
    atomic_write_evidence(&history_path, &(lines.join("\n") + "\n"))?;
    atomic_write_evidence(&latest_path, &serde_json::to_string_pretty(&entry)?)?;
    atomic_write_evidence(&recent_path, &render_remote_ebpf_recent(&lines))?;
    atomic_write_evidence(
        &summary_path,
        &serde_json::to_string_pretty(&summarize_remote_ebpf_history(
            &lines,
            rejected_entries,
            rejected.len(),
        ))?,
    )?;
    Ok(())
}

struct RemoteEvidenceLock {
    path: PathBuf,
    token: String,
}

impl Drop for RemoteEvidenceLock {
    fn drop(&mut self) {
        if fs::read_to_string(&self.path).ok().as_deref() == Some(self.token.as_str()) {
            let _ = fs::remove_file(&self.path);
        }
    }
}

fn acquire_remote_ebpf_history_lock(out_dir: &Path) -> Result<RemoteEvidenceLock, ValidationError> {
    acquire_remote_evidence_lock(
        out_dir.join("remote-ebpf-history.lock"),
        Duration::from_secs(5),
        Duration::from_secs(30),
    )
}

fn acquire_remote_validation_run_lock(
    out_dir: &Path,
) -> Result<RemoteEvidenceLock, ValidationError> {
    acquire_remote_evidence_lock(
        out_dir.join("remote-validation.lock"),
        Duration::from_secs(120),
        Duration::from_secs(15 * 60),
    )
}

fn acquire_remote_evidence_lock(
    path: PathBuf,
    lock_wait: Duration,
    stale_lock_age: Duration,
) -> Result<RemoteEvidenceLock, ValidationError> {
    const RETRY_DELAY: Duration = Duration::from_millis(25);

    let sequence = EVIDENCE_TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let token = format!("{}:{nonce}:{sequence}\n", std::process::id());
    let started_at = Instant::now();

    loop {
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(mut file) => {
                if let Err(err) = file
                    .write_all(token.as_bytes())
                    .and_then(|_| file.sync_all())
                {
                    drop(file);
                    let _ = fs::remove_file(&path);
                    return Err(ValidationError::new(format!(
                        "failed to initialize remote evidence lock '{}': {err}",
                        path.display()
                    )));
                }
                return Ok(RemoteEvidenceLock { path, token });
            }
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {
                remove_stale_remote_evidence_lock(&path, stale_lock_age)?;
                if started_at.elapsed() >= lock_wait {
                    return Err(ValidationError::new(format!(
                        "timed out waiting for remote evidence lock '{}'",
                        path.display()
                    )));
                }
                std::thread::sleep(RETRY_DELAY);
            }
            Err(err) => {
                return Err(ValidationError::new(format!(
                    "failed to acquire remote evidence lock '{}': {err}",
                    path.display()
                )));
            }
        }
    }
}

fn remove_stale_remote_evidence_lock(
    path: &Path,
    stale_age: Duration,
) -> Result<(), ValidationError> {
    let Ok(observed_token) = fs::read_to_string(path) else {
        return Ok(());
    };
    let Ok(metadata) = fs::metadata(path) else {
        return Ok(());
    };
    let is_stale = metadata
        .modified()
        .ok()
        .and_then(|modified| modified.elapsed().ok())
        .is_some_and(|age| age >= stale_age);
    if is_stale && fs::read_to_string(path).ok().as_deref() == Some(observed_token.as_str()) {
        fs::remove_file(path).map_err(|err| {
            ValidationError::new(format!(
                "failed to remove stale remote evidence lock '{}': {err}",
                path.display()
            ))
        })?;
    }
    Ok(())
}

fn read_remote_ebpf_history(path: &Path) -> Result<(Vec<String>, Vec<String>), ValidationError> {
    const MAX_HISTORY_BYTES: u64 = 1_048_576;

    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return Ok((Vec::new(), Vec::new()));
        }
        Err(err) => {
            return Err(ValidationError::new(format!(
                "failed to read remote eBPF history '{}': {err}",
                path.display()
            )));
        }
    };
    if bytes.len() as u64 > MAX_HISTORY_BYTES {
        return Err(ValidationError::new(format!(
            "remote eBPF history '{}' exceeds the {} byte safety limit",
            path.display(),
            MAX_HISTORY_BYTES
        )));
    }

    let body = String::from_utf8_lossy(&bytes);
    let mut accepted = Vec::new();
    let mut rejected = Vec::new();
    for line in body.lines().map(str::trim).filter(|line| !line.is_empty()) {
        match serde_json::from_str::<serde_json::Value>(line) {
            Ok(value) if valid_remote_ebpf_history_entry(&value) => accepted.push(line.to_string()),
            _ => rejected.push(line.to_string()),
        }
    }
    Ok((accepted, rejected))
}

fn valid_remote_ebpf_history_entry(value: &serde_json::Value) -> bool {
    let nonempty_string = |value: Option<&serde_json::Value>| {
        value
            .and_then(serde_json::Value::as_str)
            .is_some_and(|value| !value.trim().is_empty())
    };
    let preflight = value.get("preflight");
    let ebpf = value.get("ebpf");
    let fingerprint_valid = preflight
        .and_then(|value| value.get("host_fingerprint"))
        .is_none_or(|value| value.is_null() || value.as_str().is_some_and(valid_host_fingerprint));
    let total_seconds_valid = value
        .get("total_seconds")
        .and_then(serde_json::Value::as_f64)
        .is_some_and(|value| value.is_finite() && value >= 0.0);
    let status_valid = ebpf
        .and_then(|value| value.get("status"))
        .and_then(serde_json::Value::as_str)
        .is_some_and(|value| matches!(value, "ok" | "skipped" | "failed"));

    value
        .get("schema_version")
        .and_then(serde_json::Value::as_u64)
        == Some(1)
        && value
            .get("observed_at_unix")
            .and_then(serde_json::Value::as_u64)
            .is_some()
        && nonempty_string(value.get("host"))
        && nonempty_string(preflight.and_then(|value| value.get("os")))
        && nonempty_string(preflight.and_then(|value| value.get("arch")))
        && nonempty_string(preflight.and_then(|value| value.get("kernel")))
        && fingerprint_valid
        && status_valid
        && nonempty_string(ebpf.and_then(|value| value.get("reason")))
        && total_seconds_valid
}

fn append_rejected_history(
    path: &Path,
    rejected: &[String],
    observed_at_unix: u64,
) -> Result<usize, ValidationError> {
    const REJECTED_RETENTION: usize = 32;
    const MAX_REJECTED_HISTORY_BYTES: u64 = 1_048_576;

    let mut audit_lines = match fs::read(path) {
        Ok(bytes) if bytes.len() as u64 <= MAX_REJECTED_HISTORY_BYTES => {
            String::from_utf8_lossy(&bytes)
                .lines()
                .filter(|line| !line.trim().is_empty())
                .map(ToOwned::to_owned)
                .collect::<Vec<_>>()
        }
        Ok(_) => {
            return Err(ValidationError::new(format!(
                "rejected remote eBPF history '{}' exceeds the {} byte safety limit",
                path.display(),
                MAX_REJECTED_HISTORY_BYTES
            )));
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(err) => {
            return Err(ValidationError::new(format!(
                "failed to read rejected remote eBPF history '{}': {err}",
                path.display()
            )));
        }
    };
    for line in rejected {
        audit_lines.push(serde_json::to_string(&json!({
            "rejected_at_unix": observed_at_unix,
            "reason": "invalid_history_entry",
            "line": line,
        }))?);
    }
    if audit_lines.len() > REJECTED_RETENTION {
        audit_lines.drain(0..(audit_lines.len() - REJECTED_RETENTION));
    }
    if !rejected.is_empty() {
        atomic_write_evidence(path, &(audit_lines.join("\n") + "\n"))?;
    }
    Ok(audit_lines.len())
}

fn atomic_write_evidence(path: &Path, contents: &str) -> Result<(), ValidationError> {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            ValidationError::new(format!(
                "evidence path '{}' has no file name",
                path.display()
            ))
        })?;
    let sequence = EVIDENCE_TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp_path = path.with_file_name(format!(
        ".{file_name}.{}.{}.{}.tmp",
        std::process::id(),
        nonce,
        sequence
    ));
    let result = (|| -> Result<(), ValidationError> {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp_path)
            .map_err(|err| {
                ValidationError::new(format!(
                    "failed to create temporary evidence file '{}': {err}",
                    temp_path.display()
                ))
            })?;
        file.write_all(contents.as_bytes())?;
        file.sync_all()?;
        fs::rename(&temp_path, path).map_err(|err| {
            ValidationError::new(format!(
                "failed to atomically replace evidence '{}' with '{}': {err}",
                path.display(),
                temp_path.display()
            ))
        })?;
        if let Some(parent) = path.parent() {
            fs::File::open(parent)?.sync_all()?;
        }
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp_path);
    }
    result
}

fn render_remote_ebpf_recent(lines: &[String]) -> String {
    let mut rendered = Vec::new();
    for line in lines.iter().rev().take(5).rev() {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        let observed_at_unix = value
            .get("observed_at_unix")
            .and_then(|value| value.as_u64())
            .unwrap_or_default();
        let host = value
            .get("host")
            .and_then(|value| value.as_str())
            .unwrap_or("unknown");
        let status = value
            .get("ebpf")
            .and_then(|value| value.get("status"))
            .and_then(|value| value.as_str())
            .unwrap_or("unknown");
        let reason = value
            .get("ebpf")
            .and_then(|value| value.get("reason"))
            .and_then(|value| value.as_str())
            .unwrap_or("unknown");
        let total_seconds = value
            .get("total_seconds")
            .and_then(|value| value.as_f64())
            .unwrap_or_default();
        let kernel = value
            .get("preflight")
            .and_then(|value| value.get("kernel"))
            .and_then(|value| value.as_str())
            .unwrap_or("unknown");
        let route_device = value
            .get("ebpf")
            .and_then(|value| value.get("default_route_device"))
            .and_then(|value| value.as_str())
            .unwrap_or("-");

        rendered.push(format!(
            "{observed_at_unix} host={host} status={status} reason={reason} total={total_seconds:.3}s kernel={kernel} route={route_device}"
        ));
    }

    if rendered.is_empty() {
        "no remote eBPF history yet\n".to_string()
    } else {
        rendered.join("\n") + "\n"
    }
}

fn summarize_remote_ebpf_history(
    lines: &[String],
    rejected_entries: usize,
    rejected_entries_this_run: usize,
) -> serde_json::Value {
    const MINIMUM_MATRIX_HOSTS: usize = 2;
    const MINIMUM_MATRIX_KERNELS: usize = 2;

    let mut status_counts = BTreeMap::<String, usize>::new();
    let mut reason_counts = BTreeMap::<String, usize>::new();
    let mut successful_host_counts = BTreeMap::<String, usize>::new();
    let mut successful_kernel_counts = BTreeMap::<String, usize>::new();
    let mut successful_arch_counts = BTreeMap::<String, usize>::new();
    let mut unidentified_successful_runs = 0usize;
    let mut latest = None;

    for line in lines {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        let status = value
            .get("ebpf")
            .and_then(|value| value.get("status"))
            .and_then(|value| value.as_str())
            .unwrap_or("unknown")
            .to_string();
        let reason = value
            .get("ebpf")
            .and_then(|value| value.get("reason"))
            .and_then(|value| value.as_str())
            .unwrap_or("unknown")
            .to_string();
        *status_counts.entry(status).or_default() += 1;
        *reason_counts.entry(reason).or_default() += 1;
        if value
            .get("ebpf")
            .and_then(|value| value.get("status"))
            .and_then(|value| value.as_str())
            == Some("ok")
        {
            let preflight = value.get("preflight");
            let fingerprint = preflight
                .and_then(|value| value.get("host_fingerprint"))
                .and_then(serde_json::Value::as_str)
                .filter(|value| valid_host_fingerprint(value));
            if let Some(fingerprint) = fingerprint {
                *successful_host_counts
                    .entry(fingerprint.to_string())
                    .or_default() += 1;
                increment_history_dimension(
                    &mut successful_kernel_counts,
                    preflight.and_then(|value| value.get("kernel")),
                );
                increment_history_dimension(
                    &mut successful_arch_counts,
                    preflight.and_then(|value| value.get("arch")),
                );
            } else {
                unidentified_successful_runs += 1;
            }
        }
        latest = Some(value);
    }

    let unique_hosts = successful_host_counts.len();
    let unique_kernels = successful_kernel_counts.len();
    let unique_architectures = successful_arch_counts.len();

    json!({
        "schema_version": 1,
        "entries": lines.len(),
        "integrity": {
            "status": if rejected_entries == 0 { "clean" } else { "repaired" },
            "valid_entries": lines.len(),
            "rejected_entries": rejected_entries,
            "rejected_entries_this_run": rejected_entries_this_run,
        },
        "status_counts": status_counts,
        "reason_counts": reason_counts,
        "matrix": {
            "ready": unique_hosts >= MINIMUM_MATRIX_HOSTS
                && unique_kernels >= MINIMUM_MATRIX_KERNELS,
            "minimum_hosts": MINIMUM_MATRIX_HOSTS,
            "minimum_kernels": MINIMUM_MATRIX_KERNELS,
            "unique_hosts": unique_hosts,
            "unique_kernels": unique_kernels,
            "unique_architectures": unique_architectures,
            "unidentified_successful_runs": unidentified_successful_runs,
            "successful_host_counts": successful_host_counts,
            "successful_kernel_counts": successful_kernel_counts,
            "successful_arch_counts": successful_arch_counts,
        },
        "latest": latest,
    })
}

fn increment_history_dimension(
    counts: &mut BTreeMap<String, usize>,
    value: Option<&serde_json::Value>,
) {
    let Some(value) = value
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty() && *value != "unknown")
    else {
        return;
    };
    *counts.entry(value.to_string()).or_default() += 1;
}

fn default_remote_dir() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let sequence = REMOTE_RUN_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    format!(
        ".kyuubiki-remote-runs/gewyvern-remote-{now}-{}-{sequence}",
        std::process::id()
    )
}

fn remote_workspace_path(remote_dir: &str) -> String {
    if remote_dir.starts_with('/') {
        remote_dir.to_string()
    } else {
        format!("~/{remote_dir}")
    }
}

fn remote_cargo_target_dir(home_dir: &str) -> String {
    format!("{home_dir}/.cache/gewyvern/remote-target")
}

fn remote_source_cache_dir(home_dir: &str) -> String {
    format!("{home_dir}/.cache/gewyvern/remote-source")
}

fn ssh_control_path_template() -> String {
    format!("/tmp/gwy-ssh-{}-%C", std::process::id())
}

fn ssh_batch_mode_args() -> Vec<OsString> {
    vec![
        OsString::from("-o"),
        OsString::from("BatchMode=yes"),
        OsString::from("-o"),
        OsString::from("ControlMaster=auto"),
        OsString::from("-o"),
        OsString::from("ControlPersist=60"),
        OsString::from("-o"),
        OsString::from(format!("ControlPath={}", ssh_control_path_template())),
    ]
}

fn ssh_password_mode_args() -> Vec<OsString> {
    vec![
        OsString::from("-o"),
        OsString::from("StrictHostKeyChecking=accept-new"),
        OsString::from("-o"),
        OsString::from("PreferredAuthentications=password"),
        OsString::from("-o"),
        OsString::from("PubkeyAuthentication=no"),
        OsString::from("-o"),
        OsString::from("ControlMaster=auto"),
        OsString::from("-o"),
        OsString::from("ControlPersist=60"),
        OsString::from("-o"),
        OsString::from(format!("ControlPath={}", ssh_control_path_template())),
    ]
}

fn rsync_ssh_command(auth: Option<&RemoteAdminAuth>) -> String {
    let control_path = ssh_control_path_template();
    match auth {
        Some(_) => format!(
            "sshpass -e ssh -o StrictHostKeyChecking=accept-new -o PreferredAuthentications=password -o PubkeyAuthentication=no -o ControlMaster=auto -o ControlPersist=60 -o ControlPath={control_path}"
        ),
        None => format!(
            "ssh -o BatchMode=yes -o ControlMaster=auto -o ControlPersist=60 -o ControlPath={control_path}"
        ),
    }
}

fn ensure_ssh_control_master(
    host: &str,
    auth: Option<&RemoteAdminAuth>,
) -> Result<(), ValidationError> {
    let control_path = ssh_control_path_template();
    let check_status = start_ssh_command(auth, host, None)
        .arg("-O")
        .arg("check")
        .arg("-o")
        .arg(format!("ControlPath={control_path}"))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    if let Ok(status) = check_status {
        if status.success() {
            return Ok(());
        }
    }

    let status = start_ssh_command(auth, host, None)
        .arg("-fN")
        .arg("-o")
        .arg("ControlMaster=yes")
        .arg("-o")
        .arg("ControlPersist=60")
        .arg("-o")
        .arg(format!("ControlPath={control_path}"))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .status()
        .map_err(|err| {
            ValidationError::new(format!("failed to establish ssh control master: {err}"))
        })?;
    if status.success() {
        Ok(())
    } else {
        Err(ValidationError::new(format!(
            "failed to establish ssh control master: ssh exited with status {status}"
        )))
    }
}

fn close_ssh_control_master(
    host: &str,
    auth: Option<&RemoteAdminAuth>,
) -> Result<(), ValidationError> {
    let control_path = ssh_control_path_template();
    let status = start_ssh_command(auth, host, None)
        .arg("-O")
        .arg("exit")
        .arg("-o")
        .arg(format!("ControlPath={control_path}"))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|err| {
            ValidationError::new(format!("failed to close ssh control master: {err}"))
        })?;
    if status.success() {
        Ok(())
    } else {
        Err(ValidationError::new(format!(
            "failed to close ssh control master: ssh exited with status {status}"
        )))
    }
}

fn sync_workspace(
    auth: Option<&RemoteAdminAuth>,
    host: &str,
    remote_path: &str,
    workspace_sync_key: &str,
) -> Result<(), ValidationError> {
    if remote_workspace_sync_key_matches(auth, host, remote_path, workspace_sync_key)? {
        validation_log("[remote-host] workspace sync cache hit; skipping rsync");
        return Ok(());
    }

    let root = repo_root();
    let mut command = Command::new("rsync");
    if let Some(auth) = auth {
        command.env("SSHPASS", &auth.password);
    }
    command
        .arg("-az")
        .arg("--delete")
        .arg("-e")
        .arg(rsync_ssh_command(auth))
        .arg("--exclude")
        .arg(".git/")
        .arg("--exclude")
        .arg("target/")
        .arg("--exclude")
        .arg(".gewy-workspace-sync-key")
        .arg("--exclude")
        .arg("node_modules/")
        .arg("--exclude")
        .arg("tests/")
        .arg("--exclude")
        .arg("apps/**/obj/")
        .arg("--exclude")
        .arg("apps/**/bin/")
        .arg("--exclude")
        .arg("**/__pycache__/")
        .arg("--exclude")
        .arg(".DS_Store")
        .arg(format!("{}/", root.display()))
        .arg(rsync_remote_target(auth, host, &format!("{remote_path}/")))
        .stdin(Stdio::null())
        .stdout(validation_command_stdout())
        .stderr(Stdio::inherit());

    let status = command
        .status()
        .map_err(|err| ValidationError::new(format!("failed to launch rsync: {err}")))?;
    if status.success() {
        write_remote_workspace_sync_key(auth, host, remote_path, workspace_sync_key)
    } else {
        Err(ValidationError::new(format!(
            "rsync failed with status {status}"
        )))
    }
}

fn remote_workspace_sync_key_matches(
    auth: Option<&RemoteAdminAuth>,
    host: &str,
    remote_path: &str,
    workspace_sync_key: &str,
) -> Result<bool, ValidationError> {
    let remote_path = shell_single_quote(remote_path);
    let workspace_sync_key = shell_single_quote(workspace_sync_key);
    let status = start_ssh_command(
        auth,
        host,
        Some(format!(
            "[ -f {remote_path}/.gewy-workspace-sync-key ] && [ \"$(cat {remote_path}/.gewy-workspace-sync-key)\" = {workspace_sync_key} ]"
        )),
    )
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .status()
        .map_err(|err| {
            ValidationError::new(format!(
                "failed to probe remote workspace sync key: {err}"
            ))
        })?;
    Ok(status.success())
}

fn write_remote_workspace_sync_key(
    auth: Option<&RemoteAdminAuth>,
    host: &str,
    remote_path: &str,
    workspace_sync_key: &str,
) -> Result<(), ValidationError> {
    let remote_path = shell_single_quote(remote_path);
    let workspace_sync_key = shell_single_quote(workspace_sync_key);
    run_ssh_command(
        auth,
        host,
        &format!("printf '%s\\n' {workspace_sync_key} > {remote_path}/.gewy-workspace-sync-key"),
        "failed to write remote workspace sync key",
    )
}

fn compute_local_workspace_sync_key() -> Result<String, ValidationError> {
    let root = repo_root();
    if let Some(git_key) = compute_git_workspace_sync_key(&root)? {
        return Ok(git_key);
    }

    let mut child = Command::new("python3")
        .arg("-")
        .arg(root.as_os_str())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|err| ValidationError::new(format!("failed to launch python3: {err}")))?;

    {
        let stdin = child.stdin.as_mut().ok_or_else(|| {
            ValidationError::new("failed to open python3 stdin for workspace sync key")
        })?;
        stdin
            .write_all(
                br#"from pathlib import Path
import hashlib
import os
import sys

root = Path(sys.argv[1])
hash_obj = hashlib.sha256()
dir_excludes = {".git", "target", "node_modules", "tests", "__pycache__"}
file_excludes = {".DS_Store", ".gewy-workspace-sync-key"}

for current_root, dir_names, file_names in os.walk(root):
    rel_root = Path(current_root).relative_to(root)
    dir_names[:] = sorted(
        name for name in dir_names
        if name not in dir_excludes
        and not (name == "obj" and "apps" in rel_root.parts)
        and not (name == "bin" and "apps" in rel_root.parts)
    )
    for file_name in sorted(file_names):
        if file_name in file_excludes:
            continue
        file_path = Path(current_root) / file_name
        relative = file_path.relative_to(root)
        if "__pycache__" in relative.parts:
            continue
        if len(relative.parts) >= 3 and relative.parts[0] == "apps" and relative.parts[-2] in {"obj", "bin"}:
            continue
        hash_obj.update(str(relative).encode("utf-8"))
        hash_obj.update(b"\0")
        stat = file_path.stat()
        hash_obj.update(oct(stat.st_mode).encode("utf-8"))
        hash_obj.update(b"\0")
        with file_path.open("rb") as handle:
            while True:
                chunk = handle.read(1024 * 1024)
                if not chunk:
                    break
                hash_obj.update(chunk)

print(hash_obj.hexdigest())
"#,
            )
            .map_err(|err| {
                ValidationError::new(format!(
                    "failed to write workspace sync key script to python3: {err}"
                ))
            })?;
    }

    let output = child.wait_with_output().map_err(|err| {
        ValidationError::new(format!("failed to read workspace sync key output: {err}"))
    })?;
    if !output.status.success() {
        return Err(ValidationError::new(format!(
            "workspace sync key computation failed with status {}",
            output.status
        )));
    }
    let key = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if key.is_empty() {
        return Err(ValidationError::new(
            "workspace sync key computation returned an empty key",
        ));
    }
    Ok(key)
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct LocalWorkspaceSyncCache {
    head: String,
    key: String,
    changes: Vec<String>,
    files: BTreeMap<String, LocalWorkspaceSyncCacheFile>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct LocalWorkspaceSyncCacheFile {
    mode: u32,
    size: u64,
    modified_unix_nanos: u128,
}

fn local_workspace_sync_cache_path() -> PathBuf {
    repo_root()
        .join("target")
        .join("validation")
        .join("remote-linux-host-validation")
        .join("local-workspace-sync-key-cache.txt")
}

fn file_metadata_fingerprint(
    path: &Path,
) -> Result<Option<LocalWorkspaceSyncCacheFile>, ValidationError> {
    let metadata = match fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) => {
            return Err(ValidationError::new(format!(
                "failed to read workspace sync cache metadata for {}: {err}",
                path.display()
            )));
        }
    };
    let modified = metadata.modified().map_err(|err| {
        ValidationError::new(format!(
            "failed to read workspace sync cache mtime for {}: {err}",
            path.display()
        ))
    })?;
    let modified_unix_nanos = modified
        .duration_since(UNIX_EPOCH)
        .map_err(|err| {
            ValidationError::new(format!(
                "workspace sync cache mtime is before unix epoch for {}: {err}",
                path.display()
            ))
        })?
        .as_nanos();
    #[cfg(unix)]
    let mode = {
        use std::os::unix::fs::PermissionsExt;
        metadata.permissions().mode()
    };
    #[cfg(not(unix))]
    let mode = 0;
    Ok(Some(LocalWorkspaceSyncCacheFile {
        mode,
        size: metadata.len(),
        modified_unix_nanos,
    }))
}

fn collect_local_workspace_sync_cache_files(
    root: &Path,
    relevant_changes: &[String],
) -> Result<BTreeMap<String, LocalWorkspaceSyncCacheFile>, ValidationError> {
    let mut files = BTreeMap::new();
    for change in relevant_changes {
        let Some(path) = parse_git_status_path(change) else {
            continue;
        };
        let file_path = root.join(path);
        if let Some(metadata) = file_metadata_fingerprint(&file_path)? {
            files.insert(path.to_string(), metadata);
        }
    }
    Ok(files)
}

fn read_local_workspace_sync_cache() -> Result<Option<LocalWorkspaceSyncCache>, ValidationError> {
    let path = local_workspace_sync_cache_path();
    let content = match fs::read_to_string(&path) {
        Ok(content) => content,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) => {
            return Err(ValidationError::new(format!(
                "failed to read local workspace sync cache {}: {err}",
                path.display()
            )));
        }
    };

    let mut head = None;
    let mut key = None;
    let mut changes = Vec::new();
    let mut files = BTreeMap::new();
    for line in content.lines() {
        if let Some(value) = line.strip_prefix("head=") {
            head = Some(value.to_string());
            continue;
        }
        if let Some(value) = line.strip_prefix("key=") {
            key = Some(value.to_string());
            continue;
        }
        if let Some(value) = line.strip_prefix("change=") {
            changes.push(value.to_string());
            continue;
        }
        if let Some(value) = line.strip_prefix("file=") {
            let mut parts = value.splitn(4, '\t');
            let Some(path) = parts.next() else {
                continue;
            };
            let Some(mode) = parts.next() else {
                continue;
            };
            let Some(size) = parts.next() else {
                continue;
            };
            let Some(modified_unix_nanos) = parts.next() else {
                continue;
            };
            let mode = mode.parse::<u32>().map_err(|err| {
                ValidationError::new(format!(
                    "failed to parse workspace sync cache mode for {path}: {err}"
                ))
            })?;
            let size = size.parse::<u64>().map_err(|err| {
                ValidationError::new(format!(
                    "failed to parse workspace sync cache size for {path}: {err}"
                ))
            })?;
            let modified_unix_nanos = modified_unix_nanos.parse::<u128>().map_err(|err| {
                ValidationError::new(format!(
                    "failed to parse workspace sync cache mtime for {path}: {err}"
                ))
            })?;
            files.insert(
                path.to_string(),
                LocalWorkspaceSyncCacheFile {
                    mode,
                    size,
                    modified_unix_nanos,
                },
            );
        }
    }

    match (head, key) {
        (Some(head), Some(key)) => Ok(Some(LocalWorkspaceSyncCache {
            head,
            key,
            changes,
            files,
        })),
        _ => Ok(None),
    }
}

fn write_local_workspace_sync_cache(
    cache: &LocalWorkspaceSyncCache,
) -> Result<(), ValidationError> {
    let path = local_workspace_sync_cache_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut content = String::new();
    content.push_str("head=");
    content.push_str(&cache.head);
    content.push('\n');
    content.push_str("key=");
    content.push_str(&cache.key);
    content.push('\n');
    for change in &cache.changes {
        content.push_str("change=");
        content.push_str(change);
        content.push('\n');
    }
    for (path, metadata) in &cache.files {
        content.push_str("file=");
        content.push_str(path);
        content.push('\t');
        content.push_str(&metadata.mode.to_string());
        content.push('\t');
        content.push_str(&metadata.size.to_string());
        content.push('\t');
        content.push_str(&metadata.modified_unix_nanos.to_string());
        content.push('\n');
    }
    fs::write(&path, content).map_err(|err| {
        ValidationError::new(format!(
            "failed to write local workspace sync cache {}: {err}",
            path.display()
        ))
    })
}

fn try_reuse_dirty_workspace_sync_key_cache(
    root: &Path,
    head: &str,
    relevant_changes: &[String],
) -> Result<Option<String>, ValidationError> {
    let Some(cache) = read_local_workspace_sync_cache()? else {
        return Ok(None);
    };
    if cache.head != head || cache.changes != relevant_changes {
        return Ok(None);
    }
    let current_files = collect_local_workspace_sync_cache_files(root, relevant_changes)?;
    if cache.files != current_files {
        return Ok(None);
    }
    Ok(Some(cache.key))
}

fn compute_git_workspace_sync_key(root: &Path) -> Result<Option<String>, ValidationError> {
    let head = Command::new("git")
        .args(["-C"])
        .arg(root)
        .args(["rev-parse", "HEAD"])
        .output();
    let Ok(head) = head else {
        return Ok(None);
    };
    if !head.status.success() {
        return Ok(None);
    }
    let head = String::from_utf8_lossy(&head.stdout).trim().to_string();
    if head.is_empty() {
        return Ok(None);
    }

    let status = Command::new("git")
        .args(["-C"])
        .arg(root)
        .args(["status", "--porcelain=v1", "--untracked-files=all"])
        .output()
        .map_err(|err| {
            ValidationError::new(format!("failed to read git workspace status: {err}"))
        })?;
    if !status.status.success() {
        return Ok(None);
    }

    let relevant_changes = String::from_utf8_lossy(&status.stdout)
        .lines()
        .filter(|line| parse_git_status_path(line).is_some_and(is_relevant_workspace_path))
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    if relevant_changes.is_empty() {
        Ok(Some(format!("git:{head}")))
    } else {
        if let Some(cache_key) =
            try_reuse_dirty_workspace_sync_key_cache(root, &head, &relevant_changes)?
        {
            return Ok(Some(cache_key));
        }
        let key = compute_dirty_git_workspace_sync_key(root, &head, &relevant_changes)?;
        let files = collect_local_workspace_sync_cache_files(root, &relevant_changes)?;
        write_local_workspace_sync_cache(&LocalWorkspaceSyncCache {
            head,
            key: key.clone(),
            changes: relevant_changes,
            files,
        })?;
        Ok(Some(key))
    }
}

fn compute_dirty_git_workspace_sync_key(
    root: &Path,
    head: &str,
    relevant_changes: &[String],
) -> Result<String, ValidationError> {
    let mut child = Command::new("python3")
        .arg("-c")
        .arg(
            r#"from pathlib import Path
import hashlib
import sys

root = Path(sys.argv[1])
head = sys.argv[2]
hash_obj = hashlib.sha256()
hash_obj.update(f"git:{head}\0".encode("utf-8"))

for raw_line in sys.stdin.read().splitlines():
    if len(raw_line) < 4:
        continue
    status = raw_line[:2]
    path = raw_line[3:]
    if " -> " in path:
        path = path.rsplit(" -> ", 1)[1]
    file_path = root / path
    hash_obj.update(status.encode("utf-8"))
    hash_obj.update(b"\0")
    hash_obj.update(path.encode("utf-8"))
    hash_obj.update(b"\0")
    if file_path.exists():
        stat = file_path.stat()
        hash_obj.update(oct(stat.st_mode).encode("utf-8"))
        hash_obj.update(b"\0")
        with file_path.open("rb") as handle:
            while True:
                chunk = handle.read(1024 * 1024)
                if not chunk:
                    break
                hash_obj.update(chunk)

print("git-dirty:" + hash_obj.hexdigest())
"#,
        )
        .arg(root.as_os_str())
        .arg(head)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|err| {
            ValidationError::new(format!(
                "failed to launch python3 for dirty git workspace sync key: {err}"
            ))
        })?;

    {
        let stdin = child.stdin.as_mut().ok_or_else(|| {
            ValidationError::new("failed to open python3 stdin for dirty git workspace sync key")
        })?;
        stdin
            .write_all(relevant_changes.join("\n").as_bytes())
            .map_err(|err| {
                ValidationError::new(format!(
                    "failed to write dirty git workspace changes to python3: {err}"
                ))
            })?;
    }

    let output = child.wait_with_output().map_err(|err| {
        ValidationError::new(format!(
            "failed to read dirty git workspace sync key output: {err}"
        ))
    })?;
    if !output.status.success() {
        return Err(ValidationError::new(format!(
            "dirty git workspace sync key computation failed with status {}",
            output.status
        )));
    }
    let key = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if key.is_empty() {
        return Err(ValidationError::new(
            "dirty git workspace sync key computation returned an empty key",
        ));
    }
    Ok(key)
}

fn parse_git_status_path(line: &str) -> Option<&str> {
    if line.len() < 4 {
        return None;
    }
    let path = &line[3..];
    Some(path.rsplit(" -> ").next().unwrap_or(path))
}

fn is_relevant_workspace_path(path: &str) -> bool {
    if path.is_empty() {
        return false;
    }
    if path == ".DS_Store" || path == ".gewy-workspace-sync-key" {
        return false;
    }
    if path == "target" || path.starts_with("target/") {
        return false;
    }
    if path == "tests" || path.starts_with("tests/") {
        return false;
    }
    if path == "node_modules" || path.starts_with("node_modules/") {
        return false;
    }
    if path.ends_with("/__pycache__") || path.contains("/__pycache__/") {
        return false;
    }
    if path.starts_with("apps/") && (path.contains("/obj/") || path.contains("/bin/")) {
        return false;
    }
    true
}

fn materialize_remote_workspace(
    auth: Option<&RemoteAdminAuth>,
    host: &str,
    remote_source_cache: &str,
    remote_path: &str,
) -> Result<(), ValidationError> {
    let remote_source_cache = shell_single_quote(remote_source_cache);
    let remote_path = shell_single_quote(remote_path);
    run_ssh_command(
        auth,
        host,
        &format!("ln -sfn {remote_source_cache} {remote_path}"),
        "failed to point remote workspace at source cache",
    )
}

fn sync_remote_ebpf_evidence(
    auth: Option<&RemoteAdminAuth>,
    host: &str,
    remote_path: &str,
    home_dir: &str,
    out_dir: &std::path::Path,
) -> Result<(), ValidationError> {
    let remote_workspace = resolve_remote_execution_path(remote_path, home_dir)?;
    let remote_evidence_root = rsync_remote_target(
        auth,
        host,
        &format!("{remote_workspace}/target/validation/remote-ebpf/"),
    );
    let local_evidence_root = out_dir.join("remote-ebpf");
    fs::create_dir_all(&local_evidence_root)?;

    let status = Command::new("rsync")
        .envs(
            auth.map(|auth| [("SSHPASS", auth.password.as_str())])
                .into_iter()
                .flatten(),
        )
        .arg("-az")
        .arg("--delete")
        .arg("-e")
        .arg(rsync_ssh_command(auth))
        .arg(&remote_evidence_root)
        .arg(format!("{}/", local_evidence_root.display()))
        .stdin(Stdio::null())
        .stdout(validation_command_stdout())
        .stderr(Stdio::inherit())
        .status()
        .map_err(|err| {
            ValidationError::new(format!(
                "failed to launch rsync for remote eBPF evidence: {err}"
            ))
        })?;
    if status.success() {
        Ok(())
    } else {
        Err(ValidationError::new(format!(
            "remote eBPF evidence rsync failed with status {status}"
        )))
    }
}

fn start_ssh_command(
    auth: Option<&RemoteAdminAuth>,
    host: &str,
    remote_command: Option<String>,
) -> Command {
    match auth {
        Some(auth) => {
            let mut command = Command::new("sshpass");
            command
                .env("SSHPASS", &auth.password)
                .arg("-e")
                .arg("ssh")
                .args(ssh_password_mode_args())
                .arg(ssh_auth_target(host, &auth.user));
            if let Some(remote_command) = remote_command {
                command.arg(remote_command);
            }
            command
        }
        None => {
            let mut command = Command::new("ssh");
            command.args(ssh_batch_mode_args()).arg(host);
            if let Some(remote_command) = remote_command {
                command.arg(remote_command);
            }
            command
        }
    }
}

fn run_ssh_command(
    auth: Option<&RemoteAdminAuth>,
    host: &str,
    command: &str,
    context: &str,
) -> Result<(), ValidationError> {
    let status = start_ssh_command(auth, host, Some(command.to_string()))
        .stdin(Stdio::null())
        .stdout(validation_command_stdout())
        .stderr(Stdio::inherit())
        .status()
        .map_err(|err| ValidationError::new(format!("{context}: {err}")))?;
    if status.success() {
        Ok(())
    } else {
        Err(ValidationError::new(format!(
            "{context}: remote command exited with status {status}"
        )))
    }
}

fn run_ssh_script(
    auth: Option<&RemoteAdminAuth>,
    host: &str,
    command: &str,
    script: &str,
    context: &str,
) -> Result<(), ValidationError> {
    let mut child = start_ssh_command(auth, host, Some(command.to_string()))
        .stdin(Stdio::piped())
        .stdout(validation_command_stdout())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|err| ValidationError::new(format!("{context}: {err}")))?;

    {
        let stdin = child
            .stdin
            .as_mut()
            .ok_or_else(|| ValidationError::new(format!("{context}: missing ssh stdin")))?;
        stdin
            .write_all(script.as_bytes())
            .map_err(|err| ValidationError::new(format!("{context}: {err}")))?;
    }

    let status = child
        .wait()
        .map_err(|err| ValidationError::new(format!("{context}: {err}")))?;
    if status.success() {
        Ok(())
    } else {
        Err(ValidationError::new(format!(
            "{context}: remote script exited with status {status}"
        )))
    }
}

fn require_cmd(name: &str) -> Result<(), ValidationError> {
    if has_command(name) {
        Ok(())
    } else {
        Err(ValidationError::new(format!(
            "required command not found: {name}"
        )))
    }
}

fn has_command(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    let candidate = Path::new(name);
    if candidate.components().count() > 1 {
        return is_executable_file(candidate);
    }
    env::var_os("PATH")
        .map(|path| {
            env::split_paths(&path).any(|dir| {
                let base = dir.join(name);
                command_probe_candidates(&base)
                    .into_iter()
                    .any(|candidate| is_executable_file(&candidate))
            })
        })
        .unwrap_or(false)
}

fn command_probe_candidates(base: &Path) -> Vec<PathBuf> {
    #[cfg(windows)]
    {
        let mut candidates = Vec::new();
        let has_extension = base.extension().is_some();
        candidates.push(base.to_path_buf());
        if !has_extension {
            if let Some(path_ext) = env::var_os("PATHEXT") {
                for suffix in path_ext.to_string_lossy().split(';') {
                    let trimmed = suffix.trim();
                    if trimmed.is_empty() {
                        continue;
                    }
                    let suffix = trimmed.trim_start_matches('.');
                    candidates.push(base.with_extension(suffix));
                }
            }
        }
        candidates
    }
    #[cfg(not(windows))]
    {
        vec![base.to_path_buf()]
    }
}

fn is_executable_file(path: &Path) -> bool {
    let Ok(metadata) = fs::metadata(path) else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        metadata.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}

fn remote_ebpf_admin_auth() -> Result<Option<RemoteAdminAuth>, ValidationError> {
    let user = env::var("GEWY_REMOTE_EBPF_ADMIN_USER").ok();
    let password = env::var("GEWY_REMOTE_EBPF_ADMIN_PASSWORD").ok();

    match (user, password) {
        (None, None) => Ok(None),
        (Some(user), Some(password)) => {
            require_cmd("sshpass")?;
            Ok(Some(RemoteAdminAuth { user, password }))
        }
        (Some(_), None) => Err(ValidationError::new(
            "GEWY_REMOTE_EBPF_ADMIN_USER is set but GEWY_REMOTE_EBPF_ADMIN_PASSWORD is missing",
        )),
        (None, Some(_)) => Err(ValidationError::new(
            "GEWY_REMOTE_EBPF_ADMIN_PASSWORD is set but GEWY_REMOTE_EBPF_ADMIN_USER is missing",
        )),
    }
}

fn validate_remote_host(host: &str) -> Result<(), ValidationError> {
    let host = host.trim();
    if host.is_empty()
        || host.starts_with('-')
        || host.chars().any(|character| {
            !(character.is_ascii_alphanumeric()
                || matches!(character, '.' | '-' | '_' | ':' | '@' | '[' | ']' | '%'))
        })
    {
        return Err(ValidationError::new(
            "remote host must be a hostname, IP address, or user@host without whitespace, control characters, or command options",
        ));
    }
    Ok(())
}

fn remove_remote_workspace(
    host: &str,
    remote_path: &str,
    home_dir: &str,
    admin_auth: Option<&RemoteAdminAuth>,
) -> Result<(), ValidationError> {
    let workspace_path = resolve_remote_workspace_path(remote_path, home_dir)?;
    if let Some(admin_auth) = admin_auth {
        let script = format!(
            r#"set -euo pipefail
printf '%s\n' "$GEWY_REMOTE_SUDO_PASSWORD" | sudo -S -p '' -k rm -rf {workspace_path}
"#,
        );
        run_ssh_script_capture_with_auth(
            Some(admin_auth),
            host,
            remote_sudo_script_command(),
            &script,
            "failed to remove remote workspace",
        )
        .map(|_| ())
    } else {
        run_ssh_command(
            None,
            host,
            &format!("rm -rf {workspace_path}"),
            "failed to remove remote workspace",
        )
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct RemotePreflight {
    os: String,
    arch: String,
    kernel: String,
    host_fingerprint: Option<String>,
    home_dir: String,
    required_commands: Vec<String>,
    sudo_available: bool,
    default_route_device: Option<String>,
}

impl RemotePreflight {
    fn render(&self) -> String {
        format!(
            "os={}\narch={}\nkernel={}\nhost_fingerprint={}\nhome_dir={}\ncommands={}\nsudo_available={}\ndefault_route_device={}\n",
            self.os,
            self.arch,
            self.kernel,
            self.host_fingerprint.as_deref().unwrap_or(""),
            self.home_dir,
            self.required_commands.join(","),
            self.sudo_available,
            self.default_route_device.as_deref().unwrap_or("")
        )
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct RemoteArtifactManifest {
    deb: String,
    rpm: String,
}

impl RemoteArtifactManifest {
    fn render(&self) -> String {
        format!("deb={}\nrpm={}\n", self.deb, self.rpm)
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct RemoteEbpfEvidence {
    status: String,
    reason: String,
    default_route_device: Option<String>,
}

impl RemoteEbpfEvidence {
    fn render(&self) -> String {
        format!(
            "status={}\nreason={}\ndefault_route_device={}\n",
            self.status,
            self.reason,
            self.default_route_device.as_deref().unwrap_or("")
        )
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct RemoteAdminAuth {
    user: String,
    password: String,
}

fn collect_remote_preflight(
    auth: Option<&RemoteAdminAuth>,
    host: &str,
    build_packages: bool,
) -> Result<RemotePreflight, ValidationError> {
    let mut required = vec![
        "bash", "curl", "dpkg-deb", "rpm2cpio", "cpio", "find", "grep", "mktemp",
    ];
    if build_packages {
        required.extend(["cargo", "rustc", "python3", "rpmbuild"]);
    }

    let commands = required.join(" ");
    let script = format!(
        r#"set -euo pipefail
printf 'os=%s\n' "$(uname -s)"
printf 'arch=%s\n' "$(uname -m)"
printf 'kernel=%s\n' "$(uname -r)"
if [ -r /etc/machine-id ] && command -v sha256sum >/dev/null 2>&1; then
  MACHINE_HASH=$(sha256sum /etc/machine-id)
  MACHINE_HASH=${{MACHINE_HASH%% *}}
  printf 'host_fingerprint=sha256:%s\n' "$MACHINE_HASH"
else
  printf 'host_fingerprint=\n'
fi
printf 'home_dir=%s\n' "$HOME"
for cmd in {commands}; do
  command -v "$cmd" >/dev/null 2>&1 || {{
    echo "missing command: $cmd" >&2
    exit 19
  }}
done
if sudo -n true >/dev/null 2>&1; then
  printf 'sudo_available=true\n'
else
  printf 'sudo_available=false\n'
fi
DEFAULT_DEV=$(ip route show default 2>/dev/null | awk 'NR==1 {{print $5}}')
printf 'default_route_device=%s\n' "$DEFAULT_DEV"
printf 'commands=%s\n' "{commands}"
"#
    );
    let output = run_ssh_script_capture_with_auth(
        auth,
        host,
        "bash -s",
        &script,
        "remote preflight failed",
    )?;
    let preflight = parse_remote_preflight(&output)?;

    if preflight.os != "Linux" {
        return Err(ValidationError::new(format!(
            "remote host must be Linux, got `{}`",
            preflight.os
        )));
    }

    if preflight.arch != "x86_64" && preflight.arch != "amd64" {
        return Err(ValidationError::new(format!(
            "remote host must be x86_64/amd64 for packaged validation, got `{}`",
            preflight.arch
        )));
    }

    Ok(preflight)
}

fn parse_remote_preflight(output: &str) -> Result<RemotePreflight, ValidationError> {
    let mut os = None;
    let mut arch = None;
    let mut kernel = None;
    let mut host_fingerprint = None;
    let mut home_dir = None;
    let mut commands = None;
    let mut sudo_available = None;
    let mut default_route_device = None;

    for line in output.lines() {
        if let Some(value) = line.strip_prefix("os=") {
            os = Some(value.to_string());
        } else if let Some(value) = line.strip_prefix("arch=") {
            arch = Some(value.to_string());
        } else if let Some(value) = line.strip_prefix("kernel=") {
            kernel = Some(value.to_string());
        } else if let Some(value) = line.strip_prefix("host_fingerprint=") {
            if !value.is_empty() {
                if !valid_host_fingerprint(value) {
                    return Err(ValidationError::new(
                        "remote preflight host fingerprint is invalid",
                    ));
                }
                host_fingerprint = Some(value.to_string());
            }
        } else if let Some(value) = line.strip_prefix("home_dir=") {
            home_dir = Some(value.to_string());
        } else if let Some(value) = line.strip_prefix("commands=") {
            commands = Some(
                value
                    .split_whitespace()
                    .map(ToOwned::to_owned)
                    .collect::<Vec<_>>(),
            );
        } else if let Some(value) = line.strip_prefix("sudo_available=") {
            sudo_available = Some(matches!(value, "true"));
        } else if let Some(value) = line.strip_prefix("default_route_device=") {
            if !value.is_empty() {
                default_route_device = Some(value.to_string());
            }
        }
    }

    Ok(RemotePreflight {
        os: os.ok_or_else(|| ValidationError::new("remote preflight missing os"))?,
        arch: arch.ok_or_else(|| ValidationError::new("remote preflight missing arch"))?,
        kernel: kernel.ok_or_else(|| ValidationError::new("remote preflight missing kernel"))?,
        host_fingerprint,
        home_dir: home_dir
            .ok_or_else(|| ValidationError::new("remote preflight missing home_dir"))?,
        required_commands: commands
            .ok_or_else(|| ValidationError::new("remote preflight missing commands"))?,
        sudo_available: sudo_available
            .ok_or_else(|| ValidationError::new("remote preflight missing sudo_available"))?,
        default_route_device,
    })
}

fn valid_host_fingerprint(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|digest| {
        digest.len() == 64 && digest.bytes().all(|byte| byte.is_ascii_hexdigit())
    })
}

fn collect_remote_ebpf_evidence(
    host: &str,
    remote_path: &str,
    preflight: &RemotePreflight,
    admin_auth: Option<&RemoteAdminAuth>,
    phase_timings: &mut Vec<PhaseTiming>,
) -> Result<RemoteEbpfEvidence, ValidationError> {
    if !preflight.sudo_available && admin_auth.is_none() {
        return Ok(RemoteEbpfEvidence {
            status: "skipped".to_string(),
            reason: "sudo_not_available".to_string(),
            default_route_device: preflight.default_route_device.clone(),
        });
    }

    let Some(default_route_device) = preflight.default_route_device.clone() else {
        return Ok(RemoteEbpfEvidence {
            status: "skipped".to_string(),
            reason: "default_route_device_not_detected".to_string(),
            default_route_device: None,
        });
    };
    let workspace_path = resolve_remote_execution_path(remote_path, &preflight.home_dir)?;
    let target_dir = remote_cargo_target_dir(&preflight.home_dir);
    let validate_bin = format!("{target_dir}/release/gewyvern_validate");

    measure_phase(phase_timings, "remote_ebpf_validator_build", || {
        build_remote_ebpf_validator(
            admin_auth,
            host,
            &workspace_path,
            &preflight.home_dir,
            &target_dir,
        )
    })?;
    let output = measure_phase(phase_timings, "remote_ebpf_attach", || {
        run_remote_ebpf_attach(
            admin_auth,
            host,
            &workspace_path,
            &validate_bin,
            &default_route_device,
            preflight.sudo_available,
        )
    })?;
    parse_remote_ebpf_evidence(&output)
}

fn build_remote_ebpf_validator(
    auth: Option<&RemoteAdminAuth>,
    host: &str,
    workspace_path: &str,
    home_dir: &str,
    target_dir: &str,
) -> Result<(), ValidationError> {
    let script = format!(
        r#"set -euo pipefail
export HOME={home_dir}
export CARGO_HOME={cargo_home}
export RUSTUP_HOME={rustup_home}
cd {workspace_path}
mkdir -p target/validation/remote-ebpf {target_dir}
if command -v ld.lld >/dev/null 2>&1; then
  export RUSTFLAGS="${{RUSTFLAGS:-}} -C link-arg=-fuse-ld=lld"
fi
CARGO_TARGET_DIR={target_dir} cargo build --quiet --release --bin gewyvern_validate
"#,
        home_dir = shell_single_quote(home_dir),
        cargo_home = shell_single_quote(&format!("{home_dir}/.cargo")),
        rustup_home = shell_single_quote(&format!("{home_dir}/.rustup")),
        workspace_path = shell_single_quote(workspace_path),
        target_dir = shell_single_quote(target_dir),
    );
    run_ssh_script_capture_with_auth(
        auth,
        host,
        "bash -s",
        &script,
        "remote eBPF validator build failed",
    )
    .map(|_| ())
}

fn run_remote_ebpf_attach(
    auth: Option<&RemoteAdminAuth>,
    host: &str,
    workspace_path: &str,
    validate_bin: &str,
    default_route_device: &str,
    sudo_available: bool,
) -> Result<String, ValidationError> {
    let workspace_path = shell_single_quote(workspace_path);
    let validate_bin = shell_single_quote(validate_bin);
    let default_route_device_env = shell_single_quote(default_route_device);

    let script = if sudo_available {
        format!(
            r#"set -euo pipefail
cd {workspace_path}
mkdir -p target/validation/remote-ebpf
CURRENT_PATH="$PATH"
CALLER_UID="$(id -u)"
CALLER_GID="$(id -g)"
sudo -n env \
  "PATH=$CURRENT_PATH" \
  "GEWY_EVIDENCE_UID=$CALLER_UID" \
  "GEWY_EVIDENCE_GID=$CALLER_GID" \
  GEWY_WORKSPACE={workspace_env} \
  GEWY_VALIDATE_BIN={validate_bin_env} \
  GEWY_TC_DEVICE={default_route_device_env} \
  bash -c '
    set -euo pipefail
    cd "$GEWY_WORKSPACE"
    restore_evidence_owner() {{
      chown -R "$GEWY_EVIDENCE_UID:$GEWY_EVIDENCE_GID" target/validation/remote-ebpf
    }}
    trap restore_evidence_owner EXIT
    "$GEWY_VALIDATE_BIN" linux-attach-smoke --out-dir target/validation/remote-ebpf/linux-attach-smoke
    "$GEWY_VALIDATE_BIN" linux-kprobe-smoke --out-dir target/validation/remote-ebpf/linux-kprobe-smoke
    "$GEWY_VALIDATE_BIN" linux-tc-smoke --dev "$GEWY_TC_DEVICE" --out-dir target/validation/remote-ebpf/linux-tc-smoke
  '
printf 'status=ok\n'
printf 'reason=all_smokes_passed\n'
printf 'default_route_device=%s\n' "{default_route_device}"
"#,
            workspace_env = workspace_path,
            validate_bin_env = validate_bin,
        )
    } else {
        format!(
            r#"set -euo pipefail
CURRENT_PATH="$PATH"
WORKDIR={workspace_path}
CALLER_UID="$(id -u)"
CALLER_GID="$(id -g)"
cd "$WORKDIR"
mkdir -p target/validation/remote-ebpf
printf '%s\n' "$GEWY_REMOTE_SUDO_PASSWORD" | sudo -S -p '' -k env \
  "PATH=$CURRENT_PATH" \
  "GEWY_EVIDENCE_UID=$CALLER_UID" \
  "GEWY_EVIDENCE_GID=$CALLER_GID" \
  "GEWY_WORKSPACE=$WORKDIR" \
  GEWY_VALIDATE_BIN={validate_bin} \
  GEWY_TC_DEVICE={default_route_device_env} \
  bash -c '
    set -euo pipefail
    cd "$GEWY_WORKSPACE"
    restore_evidence_owner() {{
      chown -R "$GEWY_EVIDENCE_UID:$GEWY_EVIDENCE_GID" target/validation/remote-ebpf
    }}
    trap restore_evidence_owner EXIT
    "$GEWY_VALIDATE_BIN" linux-attach-smoke --out-dir target/validation/remote-ebpf/linux-attach-smoke
    "$GEWY_VALIDATE_BIN" linux-kprobe-smoke --out-dir target/validation/remote-ebpf/linux-kprobe-smoke
    "$GEWY_VALIDATE_BIN" linux-tc-smoke --dev "$GEWY_TC_DEVICE" --out-dir target/validation/remote-ebpf/linux-tc-smoke
  '
printf 'status=ok\n'
printf 'reason=all_smokes_passed_admin_ssh\n'
printf 'default_route_device=%s\n' "{default_route_device}"
"#,
            validate_bin = validate_bin,
        )
    };
    run_ssh_script_capture_with_auth(
        auth,
        host,
        if auth.is_some() {
            remote_sudo_script_command()
        } else {
            "bash -s"
        },
        &script,
        "remote eBPF smoke failed",
    )
}

fn resolve_remote_workspace_path(
    remote_path: &str,
    home_dir: &str,
) -> Result<String, ValidationError> {
    let expanded = expand_remote_path(remote_path, home_dir);
    validate_remote_workspace_path(&expanded, home_dir)
}

fn resolve_remote_execution_path(
    remote_path: &str,
    home_dir: &str,
) -> Result<String, ValidationError> {
    let expanded = expand_remote_path(remote_path, home_dir);
    normalize_remote_workspace_path(&expanded)
}

fn expand_remote_path(remote_path: &str, home_dir: &str) -> String {
    if let Some(rest) = remote_path.strip_prefix("~/") {
        format!("{home_dir}/{rest}")
    } else if remote_path.starts_with('/') {
        remote_path.to_string()
    } else {
        format!("{home_dir}/{remote_path}")
    }
}

fn validate_remote_workspace_path(path: &str, home_dir: &str) -> Result<String, ValidationError> {
    let normalized_home = normalize_remote_workspace_path(home_dir)?;
    let allowed_root =
        normalize_remote_workspace_path(&format!("{normalized_home}/.kyuubiki-remote-runs"))?;
    let normalized_path = normalize_remote_workspace_path(path)?;
    let candidate = Path::new(&normalized_path);
    let root = Path::new(&allowed_root);
    if normalized_path == allowed_root || !candidate.starts_with(root) {
        return Err(ValidationError::new(format!(
            "remote workspace path '{path}' must stay under {allowed_root}"
        )));
    }
    Ok(normalized_path)
}

fn normalize_remote_workspace_path(path: &str) -> Result<String, ValidationError> {
    let candidate = Path::new(path);
    if !candidate.is_absolute() {
        return Err(ValidationError::new(format!(
            "remote workspace path '{path}' must be absolute after expansion"
        )));
    }
    let mut normalized = PathBuf::new();
    for component in candidate.components() {
        match component {
            std::path::Component::RootDir => normalized.push(component.as_os_str()),
            std::path::Component::CurDir => {}
            std::path::Component::Normal(value) => normalized.push(value),
            std::path::Component::ParentDir => {
                if !normalized.pop() {
                    return Err(ValidationError::new(format!(
                        "remote workspace path '{path}' escapes root"
                    )));
                }
            }
            std::path::Component::Prefix(_) => {
                return Err(ValidationError::new(format!(
                    "remote workspace path '{path}' uses unsupported prefix"
                )));
            }
        }
    }
    Ok(normalized.to_string_lossy().into_owned())
}

fn collect_remote_artifact_manifest(
    auth: Option<&RemoteAdminAuth>,
    host: &str,
    remote_path: &str,
) -> Result<RemoteArtifactManifest, ValidationError> {
    let script = format!(
        r#"set -euo pipefail
cd {remote_path}
MANIFEST=target/packages/build-manifest.txt
[ -f "$MANIFEST" ] || {{
  echo "missing remote package manifest under target/packages/build-manifest.txt" >&2
  exit 20
}}
cat "$MANIFEST"
"#
    );
    let output = run_ssh_script_capture_with_auth(
        auth,
        host,
        "bash -s",
        &script,
        "remote artifact verification failed; rerun without --skip-build or reuse a populated --remote-dir",
    )?;
    parse_remote_artifact_manifest(&output)
}

fn collect_remote_package_build_timings(
    auth: Option<&RemoteAdminAuth>,
    host: &str,
    remote_path: &str,
) -> Result<String, ValidationError> {
    let script = format!(
        r#"set -euo pipefail
cd {remote_path}
cat target/packages/build-timings.txt
"#
    );
    run_ssh_script_capture_with_auth(
        auth,
        host,
        "bash -s",
        &script,
        "remote package build timings missing; rerun without --skip-build or inspect target/packages/build-timings.txt on the host",
    )
}

fn collect_remote_package_smoke_timings(
    auth: Option<&RemoteAdminAuth>,
    host: &str,
    remote_path: &str,
) -> Result<String, ValidationError> {
    let script = format!(
        r#"set -euo pipefail
cd {remote_path}
cat target/packages/package-smoke-timings.txt
"#
    );
    run_ssh_script_capture_with_auth(
        auth,
        host,
        "bash -s",
        &script,
        "remote package smoke timings missing; rerun the package smoke or inspect target/packages/package-smoke-timings.txt on the host",
    )
}

fn collect_remote_runtime_smoke_timings(
    auth: Option<&RemoteAdminAuth>,
    host: &str,
    remote_path: &str,
) -> Result<String, ValidationError> {
    let script = format!(
        r#"set -euo pipefail
cd {remote_path}
cat target/packages/runtime-smoke-timings.txt
"#
    );
    run_ssh_script_capture_with_auth(
        auth,
        host,
        "bash -s",
        &script,
        "remote runtime smoke timings missing; rerun the runtime smoke or inspect target/packages/runtime-smoke-timings.txt on the host",
    )
}

fn parse_remote_artifact_manifest(output: &str) -> Result<RemoteArtifactManifest, ValidationError> {
    let mut deb = None;
    let mut rpm = None;

    for line in output.lines() {
        if let Some(value) = line.strip_prefix("deb=") {
            deb = Some(value.to_string());
        } else if let Some(value) = line.strip_prefix("rpm=") {
            rpm = Some(value.to_string());
        }
    }

    Ok(RemoteArtifactManifest {
        deb: deb.ok_or_else(|| ValidationError::new("remote artifact manifest missing deb"))?,
        rpm: rpm.ok_or_else(|| ValidationError::new("remote artifact manifest missing rpm"))?,
    })
}

fn parse_remote_ebpf_evidence(output: &str) -> Result<RemoteEbpfEvidence, ValidationError> {
    let mut status = None;
    let mut reason = None;
    let mut default_route_device = None;

    for line in output.lines() {
        if let Some(value) = line.strip_prefix("status=") {
            status = Some(value.to_string());
        } else if let Some(value) = line.strip_prefix("reason=") {
            reason = Some(value.to_string());
        } else if let Some(value) = line.strip_prefix("default_route_device=") {
            if !value.is_empty() {
                default_route_device = Some(value.to_string());
            }
        }
    }

    Ok(RemoteEbpfEvidence {
        status: status
            .ok_or_else(|| ValidationError::new("remote eBPF evidence missing status"))?,
        reason: reason
            .ok_or_else(|| ValidationError::new("remote eBPF evidence missing reason"))?,
        default_route_device,
    })
}

fn shell_single_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', r#"'\''"#))
}

fn remote_package_smoke_script(release_line: &str) -> String {
    format!(
        r#"set -euo pipefail
MANIFEST=target/packages/build-manifest.txt
DEB=$(awk -F= '/^deb=/{{print $2; exit}}' "$MANIFEST")
RPM=$(awk -F= '/^rpm=/{{print $2; exit}}' "$MANIFEST")
TIMINGS=target/packages/package-smoke-timings.txt
TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT
DEB_ROOT="target/packages/.package-smoke/deb/$(basename "$DEB" .deb)"
DEB_STAMP="$DEB_ROOT/.deb-sha256"
RPM_ROOT="target/packages/.package-smoke/rpm/$(basename "$RPM" .rpm)"
RPM_STAMP="$RPM_ROOT/.rpm-sha256"
now_seconds() {{
  date +%s.%N
}}
duration_seconds() {{
  awk -v start="$1" -v end="$2" 'BEGIN {{ printf "%.3f", (end - start) }}'
}}
record_timing() {{
  printf '%s=%s\n' "$1" "$2" >>"$TIMINGS"
}}
rm -f "$TIMINGS"
DEB_LIST_STARTED=$(now_seconds)
dpkg-deb -c "$DEB" > "$TMP/deb-contents.txt"
record_timing deb_list_contents "$(duration_seconds "$DEB_LIST_STARTED" "$(now_seconds)")"
grep -q './usr/share/doc/gewyvern/LICENSE' "$TMP/deb-contents.txt"
EXPECTED_DEB_SHA=$(sha256sum "$DEB" | awk '{{print $1}}')
CURRENT_DEB_SHA=""
if [ -f "$DEB_STAMP" ]; then
  CURRENT_DEB_SHA=$(cat "$DEB_STAMP")
fi
if [ ! -x "$DEB_ROOT/usr/bin/gewyvern" ] || [ ! -x "$DEB_ROOT/usr/bin/gewyc" ] || [ "$CURRENT_DEB_SHA" != "$EXPECTED_DEB_SHA" ]; then
  rm -rf "$DEB_ROOT"
  mkdir -p "$DEB_ROOT"
  DEB_UNPACK_STARTED=$(now_seconds)
  dpkg-deb -x "$DEB" "$DEB_ROOT"
  printf '%s\n' "$EXPECTED_DEB_SHA" >"$DEB_STAMP"
  record_timing deb_unpack_cache_refresh "$(duration_seconds "$DEB_UNPACK_STARTED" "$(now_seconds)")"
fi
DEB_VERIFY_STARTED=$(now_seconds)
"$DEB_ROOT/usr/bin/gewyvern" --list-protocols >/dev/null
"$DEB_ROOT/usr/bin/gewyc" "$DEB_ROOT/usr/share/gewyvern/dsl/http_request_path.gewy" --json >/dev/null
test -d "$DEB_ROOT/usr/share/gewyvern/dsl"
test -d "$DEB_ROOT/usr/share/gewyvern/protocols"
test -f "$DEB_ROOT/usr/share/gewyvern/package-compat.toml"
grep -q '^schema_version = 1$' "$DEB_ROOT/usr/share/gewyvern/package-compat.toml"
grep -q '^release_line = "{release_line}"$' "$DEB_ROOT/usr/share/gewyvern/package-compat.toml"
test -f "$DEB_ROOT/usr/share/gewyvern/examples/gewyvern.toml.example"
record_timing deb_verify "$(duration_seconds "$DEB_VERIFY_STARTED" "$(now_seconds)")"
RPM_LIST_STARTED=$(now_seconds)
rpm -qpl "$RPM" > "$TMP/rpm-contents.txt"
record_timing rpm_list_contents "$(duration_seconds "$RPM_LIST_STARTED" "$(now_seconds)")"
grep -q '/usr/share/doc/gewyvern/LICENSE' "$TMP/rpm-contents.txt"
EXPECTED_RPM_SHA=$(sha256sum "$RPM" | awk '{{print $1}}')
CURRENT_RPM_SHA=""
if [ -f "$RPM_STAMP" ]; then
  CURRENT_RPM_SHA=$(cat "$RPM_STAMP")
fi
if [ ! -x "$RPM_ROOT/usr/bin/gewyvern" ] || [ ! -x "$RPM_ROOT/usr/bin/gewyc" ] || [ "$CURRENT_RPM_SHA" != "$EXPECTED_RPM_SHA" ]; then
  rm -rf "$RPM_ROOT"
  mkdir -p "$RPM_ROOT"
  RPM_UNPACK_STARTED=$(now_seconds)
  rpm2cpio "$RPM" | (cd "$RPM_ROOT" && cpio -idmu --quiet)
  printf '%s\n' "$EXPECTED_RPM_SHA" >"$RPM_STAMP"
  record_timing rpm_unpack_cache_refresh "$(duration_seconds "$RPM_UNPACK_STARTED" "$(now_seconds)")"
fi
RPM_VERIFY_STARTED=$(now_seconds)
"$RPM_ROOT/usr/bin/gewyvern" --list-protocols >/dev/null
"$RPM_ROOT/usr/bin/gewyc" "$RPM_ROOT/usr/share/gewyvern/dsl/http_request_path.gewy" --json >/dev/null
test -d "$RPM_ROOT/usr/share/gewyvern/dsl"
test -d "$RPM_ROOT/usr/share/gewyvern/protocols"
test -f "$RPM_ROOT/usr/share/gewyvern/package-compat.toml"
grep -q '^schema_version = 1$' "$RPM_ROOT/usr/share/gewyvern/package-compat.toml"
grep -q '^release_line = "{release_line}"$' "$RPM_ROOT/usr/share/gewyvern/package-compat.toml"
test -f "$RPM_ROOT/usr/share/gewyvern/examples/gewyvern.toml.example"
record_timing rpm_verify "$(duration_seconds "$RPM_VERIFY_STARTED" "$(now_seconds)")"
record_timing total "$(duration_seconds "$DEB_LIST_STARTED" "$(now_seconds)")"
echo 'remote package smoke: ok'
"#
    )
}

const REMOTE_RUNTIME_SMOKE_SCRIPT: &str = r#"set -euo pipefail
MANIFEST=target/packages/build-manifest.txt
DEB=$(awk -F= '/^deb=/{{print $2; exit}}' "$MANIFEST")
TIMINGS=target/packages/runtime-smoke-timings.txt
TMP=$(mktemp -d)
trap 'kill ${TCP_PID:-} ${UDP_PID:-} >/dev/null 2>&1 || true; wait ${TCP_PID:-} ${UDP_PID:-} >/dev/null 2>&1 || true; rm -rf "$TMP"' EXIT
RUNTIME_ROOT="target/packages/.runtime-smoke/$(basename "$DEB" .deb)"
RUNTIME_STAMP="$RUNTIME_ROOT/.deb-sha256"
now_seconds() {
  date +%s.%N
}
duration_seconds() {
  awk -v start="$1" -v end="$2" 'BEGIN { printf "%.3f", (end - start) }'
}
record_timing() {
  printf '%s=%s\n' "$1" "$2" >>"$TIMINGS"
}
rm -f "$TIMINGS"
TOTAL_STARTED=$(now_seconds)
EXPECTED_DEB_SHA=$(sha256sum "$DEB" | awk '{print $1}')
CURRENT_DEB_SHA=""
if [ -f "$RUNTIME_STAMP" ]; then
  CURRENT_DEB_SHA=$(cat "$RUNTIME_STAMP")
fi
if [ ! -x "$RUNTIME_ROOT/usr/bin/gewyvern" ] || [ ! -x "$RUNTIME_ROOT/usr/bin/gewyvern_socket_send" ] || [ "$CURRENT_DEB_SHA" != "$EXPECTED_DEB_SHA" ]; then
  rm -rf "$RUNTIME_ROOT"
  mkdir -p "$RUNTIME_ROOT"
  UNPACK_STARTED=$(now_seconds)
  dpkg-deb -x "$DEB" "$RUNTIME_ROOT"
  printf '%s\n' "$EXPECTED_DEB_SHA" >"$RUNTIME_STAMP"
  record_timing unpack_cache_refresh "$(duration_seconds "$UNPACK_STARTED" "$(now_seconds)")"
fi
GEWY="$RUNTIME_ROOT/usr/bin/gewyvern"
SEND="$RUNTIME_ROOT/usr/bin/gewyvern_socket_send"
wait_http() {
  local url="$1" out="$2" frag="${3:-}"
  for _ in $(seq 1 120); do
    if curl -fsS "$url" >"$out" 2>/dev/null; then
      if [ -z "$frag" ] || grep -q "$frag" "$out"; then
        return 0
      fi
    fi
    sleep 0.1
  done
  echo "timed out waiting for $url" >&2
  return 1
}
TCP_SOCKET=127.0.0.1:29090
TCP_API=127.0.0.1:29190
UDP_SOCKET=127.0.0.1:29091
UDP_API=127.0.0.1:29191
TCP_BOOT_STARTED=$(now_seconds)
"$GEWY" --tcp-socket "$TCP_SOCKET" --template tcp --serve --api-socket "$TCP_API" --json --summary-only >"$TMP/tcp.log" 2>&1 &
TCP_PID=$!
UDP_BOOT_STARTED=$(now_seconds)
"$GEWY" --tcp-socket "$UDP_SOCKET" --template udp --serve --api-socket "$UDP_API" --json --summary-only >"$TMP/udp.log" 2>&1 &
UDP_PID=$!
wait_http "http://$TCP_API/health" "$TMP/tcp-health.json"
record_timing tcp_boot_health "$(duration_seconds "$TCP_BOOT_STARTED" "$(now_seconds)")"
wait_http "http://$UDP_API/health" "$TMP/udp-health.json"
record_timing udp_boot_health "$(duration_seconds "$UDP_BOOT_STARTED" "$(now_seconds)")"
"$SEND" --tcp-socket "$TCP_SOCKET" --template tcp >/dev/null
"$SEND" --tcp-socket "$UDP_SOCKET" --template udp >/dev/null
TCP_SUMMARY_STARTED=$(now_seconds)
(
  wait_http "http://$TCP_API/v1/latest/summary.json" "$TMP/tcp-summary.json" '"primary_module_kind":"connection_establishment"'
  record_timing tcp_summary "$(duration_seconds "$TCP_SUMMARY_STARTED" "$(now_seconds)")"
) &
TCP_SUMMARY_WAIT_PID=$!
UDP_SUMMARY_STARTED=$(now_seconds)
(
  wait_http "http://$UDP_API/v1/latest/summary.json" "$TMP/udp-summary.json" '"primary_module_kind":"datagram_exchange"'
  record_timing udp_summary "$(duration_seconds "$UDP_SUMMARY_STARTED" "$(now_seconds)")"
) &
UDP_SUMMARY_WAIT_PID=$!
wait "$TCP_SUMMARY_WAIT_PID"
wait "$UDP_SUMMARY_WAIT_PID"
grep -q '"operator_guidance_action":"avoid_pid_strong_actions"' "$TMP/tcp-summary.json"
"$SEND" --tcp-socket "$TCP_SOCKET" --raw-line '{"broken":true' >/dev/null || true
UDP_ANALYSIS_STARTED=$(now_seconds)
(
  wait_http "http://$UDP_API/v1/latest/analysis.json" "$TMP/udp-analysis.json" '"primary_failure_mode":"none"'
  record_timing udp_analysis "$(duration_seconds "$UDP_ANALYSIS_STARTED" "$(now_seconds)")"
) &
UDP_ANALYSIS_WAIT_PID=$!
TCP_HEALTH_AFTER_BAD_STARTED=$(now_seconds)
wait_http "http://$TCP_API/health" "$TMP/tcp-health-after.json"
record_timing tcp_health_after_bad "$(duration_seconds "$TCP_HEALTH_AFTER_BAD_STARTED" "$(now_seconds)")"
grep -q '"ok":true' "$TMP/tcp-health-after.json"
"$SEND" --tcp-socket "$TCP_SOCKET" --template tcp >/dev/null
TCP_ANALYSIS_STARTED=$(now_seconds)
wait_http "http://$TCP_API/v1/latest/analysis.json" "$TMP/tcp-analysis.json" '"protocol_flows"'
record_timing tcp_analysis "$(duration_seconds "$TCP_ANALYSIS_STARTED" "$(now_seconds)")"
wait "$UDP_ANALYSIS_WAIT_PID"
kill "$TCP_PID" >/dev/null 2>&1 || true
wait "$TCP_PID" >/dev/null 2>&1 || true
kill "$UDP_PID" >/dev/null 2>&1 || true
wait "$UDP_PID" >/dev/null 2>&1 || true
record_timing total "$(duration_seconds "$TOTAL_STARTED" "$(now_seconds)")"
echo 'remote runtime smoke: ok'
"#;

fn run_ssh_script_capture_with_auth(
    auth: Option<&RemoteAdminAuth>,
    host: &str,
    command: &str,
    script: &str,
    context: &str,
) -> Result<String, ValidationError> {
    let mut ssh_command = if let Some(auth) = auth {
        let mut command_builder = Command::new("sshpass");
        command_builder
            .env("SSHPASS", &auth.password)
            .arg("-e")
            .arg("ssh")
            .args(ssh_password_mode_args())
            .arg(ssh_auth_target(host, &auth.user))
            .arg(command);
        command_builder
    } else {
        let mut command_builder = Command::new("ssh");
        command_builder
            .args(ssh_batch_mode_args())
            .arg(host)
            .arg(command);
        command_builder
    };

    let mut child = ssh_command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|err| ValidationError::new(format!("{context}: {err}")))?;

    {
        let stdin = child
            .stdin
            .as_mut()
            .ok_or_else(|| ValidationError::new(format!("{context}: missing ssh stdin")))?;
        if let Some(auth) = auth.filter(|_| command == remote_sudo_script_command()) {
            stdin
                .write_all(auth.password.as_bytes())
                .and_then(|_| stdin.write_all(b"\n"))
                .map_err(|err| ValidationError::new(format!("{context}: {err}")))?;
        }
        stdin
            .write_all(script.as_bytes())
            .map_err(|err| ValidationError::new(format!("{context}: {err}")))?;
    }

    let output = child
        .wait_with_output()
        .map_err(|err| ValidationError::new(format!("{context}: {err}")))?;
    if !output.status.success() {
        return Err(ValidationError::new(format!(
            "{context}: remote script exited with status {}",
            output.status
        )));
    }

    String::from_utf8(output.stdout).map_err(|err| {
        ValidationError::new(format!("{context}: invalid utf-8 from ssh stdout: {err}"))
    })
}

fn remote_sudo_script_command() -> &'static str {
    "bash -lc 'IFS= read -r GEWY_REMOTE_SUDO_PASSWORD; export GEWY_REMOTE_SUDO_PASSWORD; bash -s'"
}

fn ssh_auth_target(host: &str, user: &str) -> String {
    let remote_host = host.rsplit_once('@').map(|(_, host)| host).unwrap_or(host);
    format!("{user}@{remote_host}")
}

fn rsync_remote_target(auth: Option<&RemoteAdminAuth>, host: &str, remote_path: &str) -> String {
    let target = auth
        .map(|auth| ssh_auth_target(host, &auth.user))
        .unwrap_or_else(|| host.to_string());
    format!("{target}:{remote_path}")
}

#[cfg(test)]
mod tests {
    use super::{
        RemoteAdminAuth, acquire_remote_ebpf_history_lock, acquire_remote_validation_run_lock,
        atomic_write_evidence, default_remote_dir, parse_remote_artifact_manifest,
        parse_remote_ebpf_evidence, parse_remote_preflight, read_remote_ebpf_history,
        resolve_remote_execution_path, resolve_remote_workspace_path, rsync_remote_target,
        ssh_auth_target, ssh_password_mode_args, summarize_remote_ebpf_history,
        validate_remote_host,
    };
    use std::fs;
    use std::sync::mpsc;
    use std::thread;
    use std::time::Duration;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn ssh_auth_target_replaces_existing_user_prefix() {
        assert_eq!(
            ssh_auth_target("builder@192.168.1.12", "chiharukiryu"),
            "chiharukiryu@192.168.1.12"
        );
    }

    #[test]
    fn ssh_auth_target_adds_user_when_host_has_no_prefix() {
        assert_eq!(
            ssh_auth_target("kyuubiki-lab", "chiharukiryu"),
            "chiharukiryu@kyuubiki-lab"
        );
    }

    #[test]
    fn rsync_target_uses_the_same_admin_identity_as_ssh() {
        let auth = RemoteAdminAuth {
            user: "chiharukiryu".to_string(),
            password: "not-exposed".to_string(),
        };
        assert_eq!(
            rsync_remote_target(Some(&auth), "kyuubiki-dev@192.168.1.12", "/tmp/evidence/"),
            "chiharukiryu@192.168.1.12:/tmp/evidence/"
        );
        assert_eq!(
            rsync_remote_target(None, "kyuubiki-lab", "/tmp/evidence/"),
            "kyuubiki-lab:/tmp/evidence/"
        );
    }

    #[test]
    fn ebpf_history_matrix_counts_only_successful_distinct_hosts_and_kernels() {
        let fingerprint_a = format!("sha256:{}", "a".repeat(64));
        let fingerprint_b = format!("sha256:{}", "b".repeat(64));
        let lines = vec![
            serde_json::json!({"host":"alias-a","preflight":{"host_fingerprint":fingerprint_a.clone(),"kernel":"6.8.0","arch":"x86_64"},"ebpf":{"status":"ok","reason":"passed"}}).to_string(),
            serde_json::json!({"host":"alias-a-again","preflight":{"host_fingerprint":fingerprint_a.clone(),"kernel":"6.9.0","arch":"x86_64"},"ebpf":{"status":"ok","reason":"passed"}}).to_string(),
            serde_json::json!({"host":"alias-b","preflight":{"host_fingerprint":fingerprint_b,"kernel":"6.9.0","arch":"aarch64"},"ebpf":{"status":"ok","reason":"passed"}}).to_string(),
            serde_json::json!({"host":"unidentified","preflight":{"kernel":"7.0.0","arch":"riscv64"},"ebpf":{"status":"ok","reason":"passed"}}).to_string(),
            serde_json::json!({"host":"failed","preflight":{"host_fingerprint":format!("sha256:{}", "c".repeat(64)),"kernel":"7.1.0","arch":"riscv64"},"ebpf":{"status":"failed","reason":"attach_failed"}}).to_string(),
        ];

        let summary = summarize_remote_ebpf_history(&lines, 3, 1);
        let matrix = &summary["matrix"];
        assert_eq!(summary["integrity"]["status"], "repaired");
        assert_eq!(summary["integrity"]["rejected_entries"], 3);
        assert_eq!(summary["integrity"]["rejected_entries_this_run"], 1);
        assert_eq!(matrix["ready"], true);
        assert_eq!(matrix["unique_hosts"], 2);
        assert_eq!(matrix["unique_kernels"], 2);
        assert_eq!(matrix["unique_architectures"], 2);
        assert_eq!(matrix["unidentified_successful_runs"], 1);
        assert_eq!(
            matrix["successful_host_counts"]
                .get(fingerprint_a.as_str())
                .unwrap(),
            2
        );
        assert!(matrix["successful_kernel_counts"].get("7.0.0").is_none());
        assert!(matrix["successful_kernel_counts"].get("7.1.0").is_none());
    }

    #[test]
    fn remote_ebpf_history_rejects_invalid_entries_and_writes_atomically() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("gewyvern-history-integrity-{unique}"));
        fs::create_dir_all(&root).unwrap();
        let history_path = root.join("history.jsonl");
        let valid = serde_json::json!({
            "schema_version": 1,
            "observed_at_unix": 1,
            "host": "host-a",
            "preflight": {
                "os": "Linux",
                "arch": "x86_64",
                "kernel": "6.8.0",
                "host_fingerprint": null,
            },
            "ebpf": {"status": "ok", "reason": "passed"},
            "total_seconds": 1.0,
        })
        .to_string();
        fs::write(
            &history_path,
            format!("{valid}\nnot-json\n{{\"schema_version\":99}}\n"),
        )
        .unwrap();

        let (accepted, rejected) = read_remote_ebpf_history(&history_path).unwrap();
        assert_eq!(accepted, vec![valid]);
        assert_eq!(rejected.len(), 2);

        atomic_write_evidence(&history_path, "replacement\n").unwrap();
        assert_eq!(fs::read_to_string(&history_path).unwrap(), "replacement\n");
        assert_eq!(fs::read_dir(&root).unwrap().count(), 1);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn remote_ebpf_history_lock_serializes_concurrent_writers() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("gewyvern-history-lock-{unique}"));
        fs::create_dir_all(&root).unwrap();
        let first = acquire_remote_ebpf_history_lock(&root).unwrap();
        let worker_root = root.clone();
        let (sender, receiver) = mpsc::channel();
        let worker = thread::spawn(move || {
            let second = acquire_remote_ebpf_history_lock(&worker_root).unwrap();
            sender.send(()).unwrap();
            drop(second);
        });

        assert!(receiver.recv_timeout(Duration::from_millis(100)).is_err());
        drop(first);
        receiver.recv_timeout(Duration::from_secs(2)).unwrap();
        worker.join().unwrap();
        assert!(!root.join("remote-ebpf-history.lock").exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn remote_validation_lock_serializes_shared_evidence_shelf() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("gewyvern-run-lock-{unique}"));
        fs::create_dir_all(&root).unwrap();
        let first = acquire_remote_validation_run_lock(&root).unwrap();
        let worker_root = root.clone();
        let (sender, receiver) = mpsc::channel();
        let worker = thread::spawn(move || {
            let second = acquire_remote_validation_run_lock(&worker_root).unwrap();
            sender.send(()).unwrap();
            drop(second);
        });

        assert!(receiver.recv_timeout(Duration::from_millis(100)).is_err());
        drop(first);
        receiver.recv_timeout(Duration::from_secs(2)).unwrap();
        worker.join().unwrap();
        assert!(!root.join("remote-validation.lock").exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn default_remote_directories_are_unique_within_one_process() {
        let first = default_remote_dir();
        let second = default_remote_dir();

        assert_ne!(first, second);
        assert!(first.contains(&format!("-{}-", std::process::id())));
    }

    #[test]
    fn password_ssh_keeps_host_verification_enabled() {
        let args = ssh_password_mode_args();
        assert!(
            args.iter()
                .any(|arg| arg == "StrictHostKeyChecking=accept-new")
        );
        assert!(!args.iter().any(|arg| arg == "StrictHostKeyChecking=no"));
    }

    #[test]
    fn remote_host_rejects_ssh_option_and_shell_injection_shapes() {
        assert!(validate_remote_host("builder@192.168.1.12").is_ok());
        assert!(validate_remote_host("[fd00::12]").is_ok());
        assert!(validate_remote_host("-oProxyCommand=sh").is_err());
        assert!(validate_remote_host("host; touch /tmp/pwned").is_err());
    }

    #[test]
    fn parse_remote_preflight_accepts_linux_x86_64_manifest() {
        let preflight = parse_remote_preflight(
            "os=Linux\narch=x86_64\nkernel=6.8.0\nhost_fingerprint=sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\nhome_dir=/home/kyuubiki-dev\nsudo_available=true\ndefault_route_device=eth0\ncommands=bash curl cargo rustc python3 rpmbuild\n",
        )
        .unwrap();

        assert_eq!(preflight.os, "Linux");
        assert_eq!(preflight.arch, "x86_64");
        assert_eq!(preflight.kernel, "6.8.0");
        assert_eq!(
            preflight.host_fingerprint.as_deref(),
            Some("sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
        );
        assert_eq!(preflight.home_dir, "/home/kyuubiki-dev");
        assert!(preflight.sudo_available);
        assert_eq!(preflight.default_route_device.as_deref(), Some("eth0"));
        assert!(preflight.required_commands.contains(&"cargo".to_string()));
    }

    #[test]
    fn parse_remote_preflight_rejects_malformed_host_fingerprints() {
        for fingerprint in [
            "sha256:abcd",
            "sha256:gggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggg",
        ] {
            let manifest = format!(
                "os=Linux\narch=x86_64\nkernel=6.8.0\nhost_fingerprint={fingerprint}\nhome_dir=/home/test\nsudo_available=true\ndefault_route_device=eth0\ncommands=bash curl cargo rustc python3 rpmbuild\n"
            );

            let error = parse_remote_preflight(&manifest).unwrap_err();
            assert!(error.to_string().contains("host fingerprint is invalid"));
        }
    }

    #[test]
    fn parse_remote_artifact_manifest_requires_both_package_formats() {
        let manifest = parse_remote_artifact_manifest(
            "deb=target/packages/gewyvern_1.0.0-1_amd64.deb\nrpm=target/packages/rpm/gewyvern-1.0.0-1.x86_64.rpm\n",
        )
        .unwrap();

        assert!(manifest.deb.ends_with("_amd64.deb"));
        assert!(manifest.rpm.ends_with(".x86_64.rpm"));
    }

    #[test]
    fn parse_remote_ebpf_evidence_reads_status_and_route_device() {
        let evidence = parse_remote_ebpf_evidence(
            "status=ok\nreason=all_smokes_passed\ndefault_route_device=ens5\n",
        )
        .unwrap();

        assert_eq!(evidence.status, "ok");
        assert_eq!(evidence.reason, "all_smokes_passed");
        assert_eq!(evidence.default_route_device.as_deref(), Some("ens5"));
    }

    #[test]
    fn resolve_remote_workspace_path_keeps_default_rooted_workspace() {
        let resolved = resolve_remote_workspace_path(
            "~/.kyuubiki-remote-runs/gewyvern-remote-123",
            "/home/kyuubiki-dev",
        )
        .unwrap();
        assert_eq!(
            resolved,
            "/home/kyuubiki-dev/.kyuubiki-remote-runs/gewyvern-remote-123"
        );
    }

    #[test]
    fn resolve_remote_workspace_path_rejects_escape_outside_allowed_root() {
        let err =
            resolve_remote_workspace_path("/home/kyuubiki-dev/../../etc", "/home/kyuubiki-dev")
                .unwrap_err();
        assert!(err.to_string().contains("must stay under"));
    }

    #[test]
    fn resolve_remote_execution_path_allows_internal_cache_root() {
        let resolved = resolve_remote_execution_path(
            "/home/kyuubiki-dev/.cache/gewyvern/remote-source",
            "/home/kyuubiki-dev",
        )
        .unwrap();
        assert_eq!(resolved, "/home/kyuubiki-dev/.cache/gewyvern/remote-source");
    }
}
