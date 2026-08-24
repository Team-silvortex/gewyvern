use std::env;
use std::ffi::OsString;
use std::fs;
use std::io::Write;
use std::process::{Command, Stdio};
use std::sync::{
    OnceLock,
    atomic::{AtomicU64, Ordering},
};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};

use ring::digest::{SHA256, digest};
use serde_json::json;

use super::command::{
    ValidationError, ValidationReport, default_out_dir, repo_root, validation_command_stdout,
    validation_log,
};
use super::evidence_codec::{
    parse_bounded_unique_key_values, read_bounded_json_file, read_bounded_nonempty_lines,
};

static EVIDENCE_TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);
static REMOTE_RUN_SEQUENCE: AtomicU64 = AtomicU64::new(0);
static DEFAULT_SSH_CONTROL_PATH_TEMPLATE: OnceLock<String> = OnceLock::new();
pub const DEFAULT_REMOTE_LINUX_HOST: &str = "gewyvern-lab";
const REMOTE_WORKSPACE_ROOT: &str = ".gewyvern-remote-runs";
const MAX_SSH_CONTROL_PATH_BYTES: usize = 100;
const SSH_CONTROL_TEMP_SUFFIX_RESERVE: usize = 20;
const REMOTE_EBPF_HELPER: &str = "/usr/libexec/gewyvern-ebpf-helper";
const REMOTE_EBPF_EVIDENCE_ROOT: &str = "/var/lib/gewyvern-ebpf-validation";
const REMOTE_WORKSPACE_SYNC_KEY_MAX_LEN: usize = 256;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum RemoteLinuxTargetKind {
    #[default]
    Physical,
    Vm,
}

impl RemoteLinuxTargetKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Physical => "physical",
            Self::Vm => "vm",
        }
    }

    pub fn parse(value: &str) -> Result<Self, ValidationError> {
        match value {
            "physical" => Ok(Self::Physical),
            "vm" => Ok(Self::Vm),
            _ => Err(ValidationError::new(format!(
                "remote Linux target kind must be `physical` or `vm`, got `{value}`"
            ))),
        }
    }

    pub fn detect(virtualization: &str) -> Result<Self, ValidationError> {
        if !valid_virtualization(virtualization) {
            return Err(ValidationError::new(
                "remote preflight virtualization value is invalid",
            ));
        }
        if let Some(container) = virtualization.strip_prefix("container-") {
            return Err(ValidationError::new(format!(
                "remote container targets are unsupported by physical-host and VM validation ({container})"
            )));
        }
        match virtualization {
            "none" => Ok(Self::Physical),
            "unknown" => Err(ValidationError::new(
                "remote target virtualization could not be determined; install systemd-detect-virt before collecting release evidence",
            )),
            _ => Ok(Self::Vm),
        }
    }

    const fn evidence_dir_name(self) -> &'static str {
        match self {
            Self::Physical => "remote-linux-host-validation",
            Self::Vm => "remote-linux-vm-validation",
        }
    }

    const fn report_label(self) -> &'static str {
        match self {
            Self::Physical => "remote linux physical-host validation",
            Self::Vm => "remote linux VM validation",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RemoteLinuxHostOptions {
    pub host: String,
    pub target_kind: RemoteLinuxTargetKind,
    pub remote_dir: Option<String>,
    pub build_packages: bool,
    pub keep_remote_dir: bool,
}

impl Default for RemoteLinuxHostOptions {
    fn default() -> Self {
        Self {
            host: env::var("GEWY_REMOTE_HOST")
                .unwrap_or_else(|_| DEFAULT_REMOTE_LINUX_HOST.to_string()),
            target_kind: RemoteLinuxTargetKind::Physical,
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
    if let Some(remote_dir) = options.remote_dir.as_ref() {
        validate_remote_dir(remote_dir)?;
    }
    require_cmd("ssh")?;
    require_cmd("rsync")?;
    let admin_auth = remote_ebpf_admin_auth()?;
    ensure_ssh_control_master(&options.host, admin_auth.as_ref())?;

    let out_dir = default_out_dir(options.target_kind.evidence_dir_name());
    fs::create_dir_all(&out_dir)?;
    let _run_lock = acquire_remote_validation_run_lock(&out_dir)?;
    let mut phase_timings = Vec::new();

    let remote_dir = options
        .remote_dir
        .clone()
        .unwrap_or_else(default_remote_dir);
    let remote_path = remote_workspace_path(&remote_dir);
    let release_line = validate_release_line(
        &env::var("GEWY_RELEASE_LINE").unwrap_or_else(|_| "v1.16.0".to_string()),
    )?;

    validation_log(format!("[remote-host] host: {}", options.host));
    validation_log(format!(
        "[remote-host] target kind: {}",
        options.target_kind.as_str()
    ));
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

    let mut remote_workspace_touched = false;
    let result: Result<ValidationReport, ValidationError> = (|| {
        validation_log("[remote-host] ----------------------------------------");
        validation_log("[remote-host] collecting remote preflight");
        let preflight = measure_phase(&mut phase_timings, "remote_preflight", || {
            collect_remote_preflight(admin_auth.as_ref(), &options.host, options.build_packages)
        })?;
        validate_remote_target_kind(options.target_kind, &preflight)?;
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
            let workspace_sync_key = compute_local_workspace_sync_key(options.target_kind)?;
            sync_workspace(
                admin_auth.as_ref(),
                &options.host,
                &remote_source_cache,
                &workspace_sync_key,
            )
        })?;

        validation_log("[remote-host] ----------------------------------------");
        validation_log("[remote-host] materializing remote workspace from source cache");
        remote_workspace_touched = true;
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
        let validation_workspace_quoted = shell_single_quote(&validation_workspace);

        if options.build_packages {
            let target_dir = shell_single_quote(&remote_cargo_target_dir(&preflight.home_dir));
            validation_log("[remote-host] ----------------------------------------");
            validation_log("[remote-host] enforcing locked Rust workspace quality gate");
            measure_phase(&mut phase_timings, "remote_rust_quality", || {
                run_ssh_command(
                        admin_auth.as_ref(),
                        &options.host,
                        &format!(
                        "mkdir -p {target_dir} && cd -- {validation_workspace_quoted} && CARGO_TARGET_DIR={target_dir} cargo clippy --locked --quiet --workspace --all-targets -- -D warnings"
                    ),
                    "remote Rust workspace quality gate failed",
                )
            })?;
            checks.push("remote_rust_quality".to_string());

            validation_log("[remote-host] ----------------------------------------");
            validation_log("[remote-host] checking all Linux workspace targets");
            measure_phase(&mut phase_timings, "remote_linux_target_check", || {
                run_ssh_command(
                    admin_auth.as_ref(),
                    &options.host,
                    &format!(
                        "mkdir -p {target_dir} && cd -- {validation_workspace_quoted} && CARGO_TARGET_DIR={target_dir} cargo check --quiet --workspace --all-targets"
                    ),
                    "remote Linux workspace target check failed",
                )
            })?;
            checks.push("remote_linux_target_check".to_string());

            validation_log("[remote-host] ----------------------------------------");
            validation_log("[remote-host] building x86_64 packages on remote host");
            measure_phase(&mut phase_timings, "remote_package_build", || {
                run_ssh_command(
                    admin_auth.as_ref(),
                    &options.host,
                    &format!(
                        "mkdir -p {target_dir} && cd -- {validation_workspace_quoted} && CARGO_TARGET_DIR={target_dir} ./scripts/packaging/build_packages.sh --format all"
                    ),
                    "remote package build failed",
                )
            })?;
            checks.push("remote_package_build".to_string());

            validation_log("[remote-host] ----------------------------------------");
            validation_log("[remote-host] proving Leserpent control-plane NativeAOT bundle");
            measure_phase(
                &mut phase_timings,
                "remote_leserpent_control_plane_aot",
                || {
                    run_ssh_script(
                        admin_auth.as_ref(),
                        &options.host,
                        &format!("cd -- {validation_workspace_quoted} && bash -s"),
                        REMOTE_LESERPENT_CONTROL_PLANE_AOT_SCRIPT,
                        "remote Leserpent control-plane NativeAOT proof failed",
                    )
                },
            )?;
            sync_remote_validation_evidence(
                admin_auth.as_ref(),
                &options.host,
                &validation_workspace,
                &preflight.home_dir,
                &out_dir,
                "target/packages/leserpent-control-plane-aot-linux-x64",
                "leserpent-control-plane-aot-linux-x64",
            )?;
            validate_leserpent_control_plane_aot_evidence(
                &out_dir.join("leserpent-control-plane-aot-linux-x64"),
            )?;
            checks.push("remote_leserpent_control_plane_aot".to_string());

            validation_log("[remote-host] ----------------------------------------");
            validation_log(
                "[remote-host] proving packaged Leserpent Local Orchestra language packs",
            );
            measure_phase(
                &mut phase_timings,
                "remote_leserpent_language_pack_local_orchestra_aot",
                || {
                    run_ssh_script(
                        admin_auth.as_ref(),
                        &options.host,
                        &format!(
                            "cd -- {validation_workspace_quoted} && CARGO_TARGET_DIR={target_dir} bash -s"
                        ),
                        REMOTE_LESERPENT_LANGUAGE_PACK_LOCAL_ORCHESTRA_AOT_SCRIPT,
                        "remote Leserpent Local Orchestra language-pack NativeAOT proof failed",
                    )
                },
            )?;
            sync_remote_validation_evidence(
                admin_auth.as_ref(),
                &options.host,
                &validation_workspace,
                &preflight.home_dir,
                &out_dir,
                "target/packages/leserpent-language-pack-local-orchestra-native-aot-linux-x64",
                "leserpent-language-pack-local-orchestra-native-aot-linux-x64",
            )?;
            validate_leserpent_language_pack_local_orchestra_aot_evidence(
                &out_dir.join(
                    "leserpent-language-pack-local-orchestra-native-aot-linux-x64",
                ),
            )?;
            checks.push(
                "remote_leserpent_language_pack_local_orchestra_aot".to_string(),
            );
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
            let timings = collect_remote_package_build_timings(
                admin_auth.as_ref(),
                &options.host,
                &validation_workspace,
            )?;
            fs::write(
                out_dir.join("remote-package-build-timings.txt"),
                timings.render(),
            )?;
            checks.push("remote_package_build_timings".to_string());
        }
        checks.push("remote_artifacts_present".to_string());

        validation_log("[remote-host] ----------------------------------------");
        validation_log("[remote-host] running remote package smoke");
        measure_phase(&mut phase_timings, "remote_package_smoke", || {
            run_ssh_script(
                admin_auth.as_ref(),
                &options.host,
                &format!("cd -- {validation_workspace_quoted} && bash -s"),
                &remote_package_smoke_script(&release_line),
                "remote package smoke failed",
            )
        })?;
        checks.push("remote_package_smoke".to_string());
        let timings = collect_remote_package_smoke_timings(
            admin_auth.as_ref(),
            &options.host,
            &validation_workspace,
        )?;
        fs::write(
            out_dir.join("remote-package-smoke-timings.txt"),
            timings.render(),
        )?;
        checks.push("remote_package_smoke_timings".to_string());

        validation_log("[remote-host] ----------------------------------------");
        validation_log("[remote-host] running remote runtime smoke");
        measure_phase(&mut phase_timings, "remote_runtime_smoke", || {
            let runtime_smoke_script = remote_runtime_smoke_script();
            run_ssh_script(
                admin_auth.as_ref(),
                &options.host,
                &format!("cd -- {validation_workspace_quoted} && bash -s"),
                &runtime_smoke_script,
                "remote runtime smoke failed",
            )
        })?;
        checks.push("remote_runtime_smoke".to_string());
        let timings = collect_remote_runtime_smoke_timings(
            admin_auth.as_ref(),
            &options.host,
            &validation_workspace,
        )?;
        fs::write(
            out_dir.join("remote-runtime-smoke-timings.txt"),
            timings.render(),
        )?;
        checks.push("remote_runtime_smoke_timings".to_string());

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
            "host={}\ntarget_kind={}\nremote_dir={}\nbuild_packages={}\nkeep_remote_dir={}\nchecks={}\n",
            options.host,
            options.target_kind.as_str(),
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
            remote_workspace_touched = false;
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
            name: format!("{} ({})", options.target_kind.report_label(), options.host),
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
        if remote_workspace_touched {
            ValidationError::new(format!(
                "{err}\nremote workspace retained at {}:{}",
                options.host, remote_path
            ))
        } else {
            err
        }
    })
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct PhaseTiming {
    name: String,
    elapsed: Duration,
}

#[derive(Debug, Clone, PartialEq)]
struct RemotePhaseTimings(BTreeMap<String, f64>);

impl RemotePhaseTimings {
    fn render(&self) -> String {
        let lines = self
            .0
            .iter()
            .map(|(name, seconds)| format!("{name}={seconds:.3}"))
            .collect::<Vec<_>>();
        format!("{}\n", lines.join("\n"))
    }
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
        "target_kind": options.target_kind.as_str(),
        "build_packages": options.build_packages,
        "keep_remote_dir": options.keep_remote_dir,
        "preflight": {
            "os": preflight.os,
            "arch": preflight.arch,
            "kernel": preflight.kernel,
            "virtualization": preflight.virtualization,
            "host_fingerprint": preflight.host_fingerprint,
            "rustc_version": preflight.rustc_version,
            "cargo_version": preflight.cargo_version,
            "dpkg_deb_version": preflight.dpkg_deb_version,
            "rpm_version": preflight.rpm_version,
            "rpmbuild_version": preflight.rpmbuild_version,
            "sudo_available": preflight.sudo_available,
            "ebpf_helper_available": preflight.ebpf_helper_available,
            "ebpf_helper_state": preflight.ebpf_helper_state,
            "ebpf_helper_version": preflight.ebpf_helper_version,
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

    let (mut lines, rejected) = read_remote_ebpf_history(&history_path, options.target_kind)?;
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
            options.target_kind,
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

fn read_remote_ebpf_history(
    path: &Path,
    expected_target_kind: RemoteLinuxTargetKind,
) -> Result<(Vec<String>, Vec<String>), ValidationError> {
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
            Ok(value) if valid_remote_ebpf_history_entry(&value, expected_target_kind) => {
                accepted.push(line.to_string())
            }
            _ => rejected.push(line.to_string()),
        }
    }
    Ok((accepted, rejected))
}

fn valid_remote_ebpf_history_entry(
    value: &serde_json::Value,
    expected_target_kind: RemoteLinuxTargetKind,
) -> bool {
    let nonempty_string = |value: Option<&serde_json::Value>| {
        value
            .and_then(serde_json::Value::as_str)
            .is_some_and(|value| !value.trim().is_empty())
    };
    let preflight = value.get("preflight");
    let ebpf = value.get("ebpf");
    let target_kind = history_entry_target_kind(value);
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
        && target_kind == Some(expected_target_kind)
        && nonempty_string(value.get("host"))
        && nonempty_string(preflight.and_then(|value| value.get("os")))
        && nonempty_string(preflight.and_then(|value| value.get("arch")))
        && nonempty_string(preflight.and_then(|value| value.get("kernel")))
        && fingerprint_valid
        && status_valid
        && nonempty_string(ebpf.and_then(|value| value.get("reason")))
        && total_seconds_valid
}

fn history_entry_target_kind(value: &serde_json::Value) -> Option<RemoteLinuxTargetKind> {
    match value.get("target_kind") {
        Some(value) => RemoteLinuxTargetKind::parse(value.as_str()?).ok(),
        None => Some(RemoteLinuxTargetKind::Physical),
    }
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
        let target_kind = history_entry_target_kind(&value)
            .map(RemoteLinuxTargetKind::as_str)
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
            "{observed_at_unix} host={host} target={target_kind} status={status} reason={reason} total={total_seconds:.3}s kernel={kernel} route={route_device}"
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
    target_kind: RemoteLinuxTargetKind,
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
    let breadth_ready =
        unique_hosts >= MINIMUM_MATRIX_HOSTS && unique_kernels >= MINIMUM_MATRIX_KERNELS;
    let release_eligible = target_kind == RemoteLinuxTargetKind::Physical;

    json!({
        "schema_version": 1,
        "target_kind": target_kind.as_str(),
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
            "ready": release_eligible && breadth_ready,
            "breadth_ready": breadth_ready,
            "release_eligible": release_eligible,
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
        "{REMOTE_WORKSPACE_ROOT}/gewyvern-remote-{now}-{}-{sequence}",
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

fn validate_remote_dir(remote_dir: &str) -> Result<(), ValidationError> {
    if remote_dir.trim().is_empty() {
        return Err(ValidationError::new("remote directory must not be empty"));
    }
    if remote_dir.len() > 256 {
        return Err(ValidationError::new("remote directory is too long"));
    }
    if remote_dir
        .chars()
        .any(|character| !matches!(character, '/' | '-' | '_' | '.' | '@' | '~' | 'A'..='Z' | 'a'..='z' | '0'..='9'))
    {
        return Err(ValidationError::new(
            "remote directory contains unsupported characters",
        ));
    }
    if remote_dir.contains('\0') || remote_dir.contains('\n') || remote_dir.contains('\r') {
        return Err(ValidationError::new("remote directory must not contain control characters"));
    }
    Ok(())
}

fn remote_cargo_target_dir(home_dir: &str) -> String {
    format!("{home_dir}/.cache/gewyvern/remote-target")
}

fn remote_source_cache_dir(home_dir: &str) -> String {
    format!("{home_dir}/.cache/gewyvern/remote-source")
}

fn ssh_control_path_template() -> String {
    env::var("GEWY_SSH_CONTROL_PATH_TEMPLATE")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| {
            DEFAULT_SSH_CONTROL_PATH_TEMPLATE
                .get_or_init(default_ssh_control_path_template)
                .clone()
        })
}

fn default_ssh_control_path_template() -> String {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    #[cfg(unix)]
    let root = Path::new("/tmp").to_path_buf();
    #[cfg(not(unix))]
    let root = std::env::temp_dir();
    root.join(format!("gwy-{}-{nonce}-%C", std::process::id()))
        .to_string_lossy()
        .into_owned()
}

fn validate_ssh_control_path_template(path: &str) -> Result<(), ValidationError> {
    if path.is_empty() || path.contains(['\0', '\r', '\n']) || !Path::new(path).is_absolute() {
        return Err(ValidationError::new(
            "SSH control path template must be a non-empty absolute path without control characters",
        ));
    }

    let bytes = path.as_bytes();
    let mut index = 0;
    let mut expanded_len = 0usize;
    while index < bytes.len() {
        if bytes[index] != b'%' {
            expanded_len += 1;
            index += 1;
            continue;
        }
        let Some(token) = bytes.get(index + 1) else {
            return Err(ValidationError::new(
                "SSH control path template ends with an incomplete token",
            ));
        };
        match token {
            b'C' => expanded_len += 40,
            b'%' => expanded_len += 1,
            _ => {
                return Err(ValidationError::new(
                    "SSH control path template only permits the bounded %C and %% tokens",
                ));
            }
        }
        index += 2;
    }

    #[cfg(unix)]
    if expanded_len + SSH_CONTROL_TEMP_SUFFIX_RESERVE > MAX_SSH_CONTROL_PATH_BYTES {
        return Err(ValidationError::new(format!(
            "SSH control path template expands beyond the portable Unix socket budget of {} bytes",
            MAX_SSH_CONTROL_PATH_BYTES - SSH_CONTROL_TEMP_SUFFIX_RESERVE
        )));
    }
    Ok(())
}

fn ssh_batch_mode_args(control_path: &str) -> Vec<OsString> {
    vec![
        OsString::from("-o"),
        OsString::from("BatchMode=yes"),
        OsString::from("-o"),
        OsString::from("ControlMaster=auto"),
        OsString::from("-o"),
        OsString::from("ControlPersist=60"),
        OsString::from("-o"),
        OsString::from(format!("ControlPath={}", control_path)),
    ]
}

fn ssh_password_mode_args(control_path: &str) -> Vec<OsString> {
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
        OsString::from(format!("ControlPath={}", control_path)),
    ]
}

fn rsync_ssh_command(auth: Option<&RemoteAdminAuth>) -> Result<String, ValidationError> {
    let control_path = ssh_control_path_template();
    validate_ssh_control_path_template(&control_path)?;
    let control_path = shell_single_quote(&control_path);
    match auth {
        Some(_) => Ok(format!(
            "sshpass -e ssh -o StrictHostKeyChecking=accept-new -o PreferredAuthentications=password -o PubkeyAuthentication=no -o ControlMaster=auto -o ControlPersist=60 -o ControlPath={control_path}"
        )),
        None => Ok(format!(
            "ssh -o BatchMode=yes -o ControlMaster=auto -o ControlPersist=60 -o ControlPath={control_path}"
        )),
    }
}

fn ensure_ssh_control_master(
    host: &str,
    auth: Option<&RemoteAdminAuth>,
) -> Result<(), ValidationError> {
    let control_path = ssh_control_path_template();
    validate_ssh_control_path_template(&control_path)?;
    let check_status = start_ssh_command(auth, host, None)?
        .arg("-O")
        .arg("check")
        .arg("-o")
        .arg(format!("ControlPath={control_path}"))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    if let Ok(status) = check_status
        && status.success()
    {
        return Ok(());
    }

    let status = start_ssh_command(auth, host, None)?
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
    let status = start_ssh_command(auth, host, None)?
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
    let workspace_sync_key = validate_remote_workspace_sync_key(workspace_sync_key)?;
    if remote_workspace_sync_key_matches(auth, host, remote_path, &workspace_sync_key)? {
        validation_log("[remote-host] workspace sync cache hit; skipping rsync");
        return Ok(());
    }

    let rsync_command = rsync_ssh_command(auth)?;
    let remote_target = rsync_remote_target(auth, host, &format!("{remote_path}/"))?;
    let root = repo_root();
    let mut command = Command::new("rsync");
    if let Some(auth) = auth {
        command.env("SSHPASS", &auth.password);
    }
    command
        .arg("-az")
        .arg("--delete")
        .arg("-e")
        .arg(rsync_command)
        .arg("--exclude")
        .arg(".git/")
        .arg("--exclude")
        .arg("target/")
        .arg("--exclude")
        .arg(".gewy-workspace-sync-key")
        .arg("--exclude")
        .arg("node_modules/")
        .arg("--exclude")
        .arg("apps/**/obj/")
        .arg("--exclude")
        .arg("apps/**/bin/")
        .arg("--exclude")
        .arg("**/__pycache__/")
        .arg("--exclude")
        .arg(".DS_Store")
        .arg(format!("{}/", root.display()))
        .arg(remote_target)
        .stdin(Stdio::null())
        .stdout(validation_command_stdout())
        .stderr(Stdio::inherit());

    let status = command
        .status()
        .map_err(|err| ValidationError::new(format!("failed to launch rsync: {err}")))?;
    if status.success() {
        write_remote_workspace_sync_key(auth, host, remote_path, &workspace_sync_key)
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
    validate_remote_rsync_path(remote_path)?;
    let workspace_sync_key = validate_remote_workspace_sync_key(workspace_sync_key)?;
    let remote_path = shell_single_quote(remote_path);
    let workspace_sync_key = shell_single_quote(&workspace_sync_key);
    let status = start_ssh_command(
        auth,
        host,
        Some(format!(
            "[ -f {remote_path}/.gewy-workspace-sync-key ] && [ \"$(cat {remote_path}/.gewy-workspace-sync-key)\" = {workspace_sync_key} ]"
        )),
    )?
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
    validate_remote_rsync_path(remote_path)?;
    let workspace_sync_key = validate_remote_workspace_sync_key(workspace_sync_key)?;
    let remote_path = shell_single_quote(remote_path);
    let workspace_sync_key = shell_single_quote(&workspace_sync_key);
    run_ssh_command(
        auth,
        host,
        &format!("printf '%s\\n' {workspace_sync_key} > {remote_path}/.gewy-workspace-sync-key"),
        "failed to write remote workspace sync key",
    )
}

fn compute_local_workspace_sync_key(
    target_kind: RemoteLinuxTargetKind,
) -> Result<String, ValidationError> {
    let root = repo_root();
    if let Some(git_key) = compute_git_workspace_sync_key(&root, target_kind)? {
        return validate_remote_workspace_sync_key(&git_key);
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
    validate_remote_workspace_sync_key(String::from_utf8_lossy(&output.stdout).trim())
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

fn local_workspace_sync_cache_path(target_kind: RemoteLinuxTargetKind) -> PathBuf {
    repo_root()
        .join("target")
        .join("validation")
        .join("remote-workspace-sync-cache")
        .join(format!("{}.txt", target_kind.as_str()))
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

fn read_local_workspace_sync_cache(
    target_kind: RemoteLinuxTargetKind,
) -> Result<Option<LocalWorkspaceSyncCache>, ValidationError> {
    let path = local_workspace_sync_cache_path(target_kind);
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
    target_kind: RemoteLinuxTargetKind,
    cache: &LocalWorkspaceSyncCache,
) -> Result<(), ValidationError> {
    let path = local_workspace_sync_cache_path(target_kind);
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
    target_kind: RemoteLinuxTargetKind,
    head: &str,
    relevant_changes: &[String],
) -> Result<Option<String>, ValidationError> {
    let Some(cache) = read_local_workspace_sync_cache(target_kind)? else {
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

fn compute_git_workspace_sync_key(
    root: &Path,
    target_kind: RemoteLinuxTargetKind,
) -> Result<Option<String>, ValidationError> {
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
        Ok(Some(validate_remote_workspace_sync_key(&format!(
            "git:{head}"
        ))?))
    } else {
        if let Some(cache_key) =
            try_reuse_dirty_workspace_sync_key_cache(root, target_kind, &head, &relevant_changes)?
        {
            return Ok(Some(validate_remote_workspace_sync_key(&cache_key)?));
        }
        let key = validate_remote_workspace_sync_key(&compute_dirty_git_workspace_sync_key(
            root,
            &head,
            &relevant_changes,
        )?)?;
        let files = collect_local_workspace_sync_cache_files(root, &relevant_changes)?;
        write_local_workspace_sync_cache(
            target_kind,
            &LocalWorkspaceSyncCache {
                head,
                key: key.clone(),
                changes: relevant_changes,
                files,
            },
        )?;
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
    validate_remote_workspace_sync_key(String::from_utf8_lossy(&output.stdout).trim())
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
    validate_remote_rsync_path(remote_source_cache)?;
    validate_remote_rsync_path(remote_path)?;
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
    sync_remote_validation_evidence(
        auth,
        host,
        remote_path,
        home_dir,
        out_dir,
        "target/validation/remote-ebpf",
        "remote-ebpf",
    )
}

fn sync_remote_validation_evidence(
    auth: Option<&RemoteAdminAuth>,
    host: &str,
    remote_path: &str,
    home_dir: &str,
    out_dir: &std::path::Path,
    remote_evidence_path: &str,
    evidence_name: &str,
) -> Result<(), ValidationError> {
    let remote_workspace = resolve_remote_execution_path(remote_path, home_dir)?;
    let remote_evidence_root =
        rsync_remote_target(auth, host, &format!("{remote_workspace}/{remote_evidence_path}/"))?;
    let rsync_command = rsync_ssh_command(auth)?;
    let local_evidence_root = out_dir.join(evidence_name);
    if let Ok(metadata) = fs::symlink_metadata(&local_evidence_root)
        && (metadata.file_type().is_symlink() || !metadata.is_dir())
    {
        return Err(ValidationError::new(format!(
            "local validation evidence destination must be a non-symlink directory: {}",
            local_evidence_root.display()
        )));
    }
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
        .arg(rsync_command)
        .arg(&remote_evidence_root)
        .arg(format!("{}/", local_evidence_root.display()))
        .stdin(Stdio::null())
        .stdout(validation_command_stdout())
        .stderr(Stdio::inherit())
        .status()
        .map_err(|err| {
            ValidationError::new(format!(
                "failed to launch rsync for remote validation evidence '{evidence_name}': {err}"
            ))
        })?;
    if status.success() {
        Ok(())
    } else {
        Err(ValidationError::new(format!(
            "remote validation evidence '{evidence_name}' rsync failed with status {status}"
        )))
    }
}

pub fn validate_leserpent_control_plane_aot_evidence(root: &Path) -> Result<(), ValidationError> {
    const FILES: [&str; 12] = [
        "environment.txt",
        "restore.log",
        "publish.log",
        "payload.sha256",
        "service.log",
        "health.json",
        "registration-plan.json",
        "registration.json",
        "recovery.json",
        "attention.json",
        "runtime-state.json",
        "orchestra.db",
    ];
    const PROOF_SECRET: &str = "native-aot-proof-secret";

    let root_metadata = fs::symlink_metadata(root).map_err(|error| {
        ValidationError::new(format!(
            "failed to inspect Leserpent control-plane NativeAOT evidence '{}': {error}",
            root.display()
        ))
    })?;
    if root_metadata.file_type().is_symlink() || !root_metadata.is_dir() {
        return Err(ValidationError::new(format!(
            "Leserpent control-plane NativeAOT evidence must be a non-symlink directory: {}",
            root.display()
        )));
    }

    let mut observed = BTreeSet::new();
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let name = entry.file_name().into_string().map_err(|_| {
            ValidationError::new("Leserpent control-plane NativeAOT evidence has a non-UTF-8 name")
        })?;
        let metadata = fs::symlink_metadata(entry.path())?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(ValidationError::new(format!(
                "Leserpent control-plane NativeAOT evidence entry must be a regular non-symlink file: {name}"
            )));
        }
        let max_bytes = if name == "orchestra.db" {
            4 * 1024 * 1024
        } else if name.ends_with(".log") {
            2 * 1024 * 1024
        } else {
            64 * 1024
        };
        if metadata.len() > max_bytes {
            return Err(ValidationError::new(format!(
                "Leserpent control-plane NativeAOT evidence entry exceeds {max_bytes} bytes: {name}"
            )));
        }
        let bytes = fs::read(entry.path())?;
        if bytes
            .windows(PROOF_SECRET.len())
            .any(|window| window == PROOF_SECRET.as_bytes())
        {
            return Err(ValidationError::new(format!(
                "Leserpent control-plane NativeAOT evidence contains the proof secret: {name}"
            )));
        }
        observed.insert(name);
    }
    let mut expected = FILES
        .into_iter()
        .map(str::to_string)
        .collect::<BTreeSet<_>>();
    expected.insert("evidence-index.json".to_string());
    if observed != expected {
        return Err(ValidationError::new(
            "Leserpent control-plane NativeAOT evidence inventory is incomplete or contains unexpected files",
        ));
    }

    let index = read_bounded_json_file(
        &root.join("evidence-index.json"),
        "Leserpent control-plane NativeAOT evidence index",
        8 * 1024,
    )?;
    require_exact_json_keys(
        &index,
        &["schema_version", "proof", "result", "files"],
        "Leserpent control-plane NativeAOT evidence index",
    )?;
    if index
        .get("schema_version")
        .and_then(serde_json::Value::as_u64)
        != Some(1)
        || index.get("proof").and_then(serde_json::Value::as_str)
            != Some("leserpent-control-plane-native-aot-linux-x64")
        || index.get("result").and_then(serde_json::Value::as_str) != Some("passed")
        || index
            .get("files")
            .and_then(serde_json::Value::as_array)
            .map(|files| {
                files
                    .iter()
                    .filter_map(serde_json::Value::as_str)
                    .collect::<Vec<_>>()
            })
            != Some(FILES.to_vec())
    {
        return Err(ValidationError::new(
            "Leserpent control-plane NativeAOT evidence index violates its fixed contract",
        ));
    }

    let health = read_aot_json(root, "health.json")?;
    if health.get("ok").and_then(serde_json::Value::as_bool) != Some(true)
        || health
            .pointer("/runtimePosture/coreReady")
            .and_then(serde_json::Value::as_bool)
            != Some(true)
    {
        return Err(ValidationError::new(
            "Leserpent control-plane NativeAOT health proof is not core-ready",
        ));
    }
    let plan = read_aot_json(root, "registration-plan.json")?;
    let plan_token = plan.get("planToken").and_then(serde_json::Value::as_str);
    if plan.get("allowed").and_then(serde_json::Value::as_bool) != Some(true)
        || plan.get("action").and_then(serde_json::Value::as_str) != Some("create")
        || !plan_token.is_some_and(|value| valid_lower_hex(value, 64))
    {
        return Err(ValidationError::new(
            "Leserpent control-plane NativeAOT registration plan proof is invalid",
        ));
    }
    let registration = read_aot_json(root, "registration.json")?;
    let runtime_id = registration
        .get("runtimeId")
        .and_then(serde_json::Value::as_str);
    if !runtime_id.is_some_and(|value| valid_lower_hex(value, 32))
        || registration.to_string().contains(PROOF_SECRET)
    {
        return Err(ValidationError::new(
            "Leserpent control-plane NativeAOT registration proof is invalid or contains a secret",
        ));
    }
    let recovery = read_aot_json(root, "recovery.json")?;
    let steps = recovery.get("steps").and_then(serde_json::Value::as_array);
    let step_kinds = steps.map(|values| {
        values
            .iter()
            .filter(|value| {
                value.get("outcome").and_then(serde_json::Value::as_str) == Some("degraded")
            })
            .filter_map(|value| value.get("kind").and_then(serde_json::Value::as_str))
            .collect::<BTreeSet<_>>()
    });
    if recovery
        .get("runtimeId")
        .and_then(serde_json::Value::as_str)
        != runtime_id
        || recovery.get("kind").and_then(serde_json::Value::as_str) != Some("all")
        || recovery.get("outcome").and_then(serde_json::Value::as_str) != Some("degraded")
        || steps.map(Vec::len) != Some(2)
        || step_kinds != Some(BTreeSet::from(["capabilities", "status"]))
    {
        return Err(ValidationError::new(
            "Leserpent control-plane NativeAOT recovery proof is invalid",
        ));
    }
    let attention = read_aot_json(root, "attention.json")?;
    let refresh_all = attention
        .get("suggestedActions")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|actions| {
            actions.iter().any(|action| {
                action.get("action").and_then(serde_json::Value::as_str) == Some("refresh_all")
                    && action
                        .get("commandKind")
                        .and_then(serde_json::Value::as_str)
                        == Some("all")
            })
        });
    if attention
        .get("runtimeId")
        .and_then(serde_json::Value::as_str)
        != runtime_id
        || !refresh_all
    {
        return Err(ValidationError::new(
            "Leserpent control-plane NativeAOT attention proof is invalid",
        ));
    }

    let state = read_aot_json(root, "runtime-state.json")?;
    if !state.is_object() {
        return Err(ValidationError::new(
            "Leserpent control-plane NativeAOT runtime state must be a JSON object",
        ));
    }
    let database = fs::read(root.join("orchestra.db"))?;
    if !database.starts_with(b"SQLite format 3\0") {
        return Err(ValidationError::new(
            "Leserpent control-plane NativeAOT persistence evidence is not a SQLite 3 database",
        ));
    }

    let hashes = read_bounded_nonempty_lines(
        &root.join("payload.sha256"),
        "Leserpent control-plane NativeAOT payload hashes",
        8 * 1024,
        4,
        512,
    )?;
    let required_payloads = [
        "Leserpent",
        "leserpent-compat-bridge",
        "leserpentd",
        "libe_sqlite3.so",
    ];
    if hashes.len() != required_payloads.len()
        || !required_payloads.iter().all(|name| {
            hashes.iter().any(|line| {
                line.split_whitespace()
                    .next()
                    .is_some_and(|hash| valid_lower_hex(hash, 64))
                    && line.ends_with(&format!("/publish/{name}"))
            })
        })
    {
        return Err(ValidationError::new(
            "Leserpent control-plane NativeAOT payload hash inventory is invalid",
        ));
    }
    Ok(())
}

const LOCAL_ORCHESTRA_VERIFICATION_PREFIX: &str = "local orchestra valid: ";
const LOCAL_ORCHESTRA_VERIFICATION_CHECKS: [&str; 18] = [
    "rust_daemon=true",
    "loopback_tls=true",
    "ephemeral_token=true",
    "owned_authority=true",
    "runtime_topology_query=true",
    "health_topology_composition=true",
    "authority_bound_live_state=true",
    "credential_free_language_pack_download=true",
    "language_pack_digest_binding=true",
    "language_pack_private_roundtrip=true",
    "private_files=true",
    "minimal_child_environment=true",
    "optional_bootstrap_origin=true",
    "optional_gewyvern_provisioning_origin=true",
    "private_bootstrap_trust=true",
    "package_local_daemon=true",
    "symlink_rejection=true",
    "process_cleanup=true",
];

pub fn validate_leserpent_language_pack_local_orchestra_aot_evidence(
    root: &Path,
) -> Result<(), ValidationError> {
    const FILES: [&str; 7] = [
        "environment.txt",
        "restore.log",
        "publish.log",
        "daemon-build.log",
        "payload.sha256",
        "language-pack-assets.sha256",
        "verification.log",
    ];
    const ENVIRONMENT_KEYS: [&str; 9] = [
        "os",
        "arch",
        "rid",
        "kernel",
        "dotnet_sdk",
        "rustc",
        "cargo",
        "avalonia_bytes",
        "leserpentd_bytes",
    ];

    let root_metadata = fs::symlink_metadata(root).map_err(|error| {
        ValidationError::new(format!(
            "failed to inspect Leserpent Local Orchestra language-pack evidence '{}': {error}",
            root.display()
        ))
    })?;
    if root_metadata.file_type().is_symlink() || !root_metadata.is_dir() {
        return Err(ValidationError::new(format!(
            "Leserpent Local Orchestra language-pack evidence must be a non-symlink directory: {}",
            root.display()
        )));
    }

    let mut observed = BTreeSet::new();
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let name = entry.file_name().into_string().map_err(|_| {
            ValidationError::new(
                "Leserpent Local Orchestra language-pack evidence has a non-UTF-8 name",
            )
        })?;
        let metadata = fs::symlink_metadata(entry.path())?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(ValidationError::new(format!(
                "Leserpent Local Orchestra language-pack evidence entry must be a regular non-symlink file: {name}"
            )));
        }
        let max_bytes = if name.ends_with(".log") {
            2 * 1024 * 1024
        } else {
            64 * 1024
        };
        if metadata.len() == 0 || metadata.len() > max_bytes {
            return Err(ValidationError::new(format!(
                "Leserpent Local Orchestra language-pack evidence entry violates its byte budget: {name}"
            )));
        }
        let bytes = fs::read(entry.path())?;
        for forbidden in [
            b"Authorization: Bearer ".as_slice(),
            b"X-Leserpent-Admin-Token:".as_slice(),
            b"BEGIN PRIVATE KEY".as_slice(),
        ] {
            if bytes
                .windows(forbidden.len())
                .any(|window| window == forbidden)
            {
                return Err(ValidationError::new(format!(
                    "Leserpent Local Orchestra language-pack evidence contains forbidden credential material: {name}"
                )));
            }
        }
        observed.insert(name);
    }
    let mut expected = FILES
        .into_iter()
        .map(str::to_string)
        .collect::<BTreeSet<_>>();
    expected.insert("evidence-index.json".to_string());
    if observed != expected {
        return Err(ValidationError::new(
            "Leserpent Local Orchestra language-pack evidence inventory is incomplete or contains unexpected files",
        ));
    }

    let index = read_bounded_json_file(
        &root.join("evidence-index.json"),
        "Leserpent Local Orchestra language-pack evidence index",
        8 * 1024,
    )?;
    require_exact_json_keys(
        &index,
        &["schema_version", "proof", "result", "files"],
        "Leserpent Local Orchestra language-pack evidence index",
    )?;
    if index
        .get("schema_version")
        .and_then(serde_json::Value::as_u64)
        != Some(1)
        || index.get("proof").and_then(serde_json::Value::as_str)
            != Some("leserpent-language-pack-local-orchestra-native-aot-linux-x64")
        || index.get("result").and_then(serde_json::Value::as_str) != Some("passed")
        || index
            .get("files")
            .and_then(serde_json::Value::as_array)
            .map(|files| {
                files
                    .iter()
                    .filter_map(serde_json::Value::as_str)
                    .collect::<Vec<_>>()
            })
            != Some(FILES.to_vec())
    {
        return Err(ValidationError::new(
            "Leserpent Local Orchestra language-pack evidence index violates its fixed contract",
        ));
    }

    let environment_body = fs::read_to_string(root.join("environment.txt"))?;
    let environment = parse_bounded_unique_key_values(
        &environment_body,
        "Leserpent Local Orchestra language-pack environment",
        &ENVIRONMENT_KEYS,
    )?;
    if environment.len() != ENVIRONMENT_KEYS.len()
        || environment.get("os").map(String::as_str) != Some("Linux")
        || environment.get("arch").map(String::as_str) != Some("x86_64")
        || environment.get("rid").map(String::as_str) != Some("linux-x64")
        || ["kernel", "dotnet_sdk", "rustc", "cargo"].iter().any(|key| {
            environment
                .get(*key)
                .is_none_or(|value| value.is_empty() || value.len() > 256)
        })
    {
        return Err(ValidationError::new(
            "Leserpent Local Orchestra language-pack environment violates its fixed contract",
        ));
    }
    for key in ["avalonia_bytes", "leserpentd_bytes"] {
        let bytes = environment
            .get(key)
            .and_then(|value| value.parse::<u64>().ok())
            .filter(|value| (1024 * 1024..=512 * 1024 * 1024).contains(value));
        if bytes.is_none() {
            return Err(ValidationError::new(format!(
                "Leserpent Local Orchestra language-pack {key} is invalid"
            )));
        }
    }

    let verification = read_bounded_nonempty_lines(
        &root.join("verification.log"),
        "Leserpent Local Orchestra language-pack verification",
        64 * 1024,
        1,
        4096,
    )?;
    let Some(checks) = verification
        .first()
        .and_then(|line| line.strip_prefix(LOCAL_ORCHESTRA_VERIFICATION_PREFIX))
    else {
        return Err(ValidationError::new(
            "Leserpent Local Orchestra language-pack verification is missing its fixed prefix",
        ));
    };
    let checks = checks.split(", ").collect::<BTreeSet<_>>();
    if checks
        != LOCAL_ORCHESTRA_VERIFICATION_CHECKS
            .into_iter()
            .collect::<BTreeSet<_>>()
    {
        return Err(ValidationError::new(
            "Leserpent Local Orchestra language-pack verification is incomplete",
        ));
    }

    validate_sha256_manifest(
        root,
        "payload.sha256",
        &["Leserpent.Avalonia", "leserpentd"],
    )?;
    let asset_hashes = validate_sha256_manifest(
        root,
        "language-pack-assets.sha256",
        &["catalog.json", "pt-BR.json"],
    )?;
    let asset_root = repo_root().join("apps/leserpent/src/Leserpent/wwwroot/language-packs");
    for name in ["catalog.json", "pt-BR.json"] {
        let expected_hash = evidence_file_sha256(&asset_root.join(name))?;
        if asset_hashes.get(name) != Some(&expected_hash) {
            return Err(ValidationError::new(format!(
                "remote Leserpent language-pack asset drifted from the synchronized workspace: {name}"
            )));
        }
    }
    Ok(())
}

fn validate_sha256_manifest(
    root: &Path,
    name: &str,
    expected_files: &[&str],
) -> Result<BTreeMap<String, String>, ValidationError> {
    let lines = read_bounded_nonempty_lines(
        &root.join(name),
        &format!("Leserpent Local Orchestra language-pack {name}"),
        8 * 1024,
        expected_files.len(),
        512,
    )?;
    let mut values = BTreeMap::new();
    for line in lines {
        let mut parts = line.split_whitespace();
        let hash = parts.next().unwrap_or_default();
        let file = parts.next().unwrap_or_default();
        if parts.next().is_some()
            || !valid_lower_hex(hash, 64)
            || !expected_files.contains(&file)
            || values.insert(file.to_string(), hash.to_string()).is_some()
        {
            return Err(ValidationError::new(format!(
                "Leserpent Local Orchestra language-pack {name} is invalid"
            )));
        }
    }
    if values.len() != expected_files.len() {
        return Err(ValidationError::new(format!(
            "Leserpent Local Orchestra language-pack {name} is incomplete"
        )));
    }
    Ok(values)
}

fn evidence_file_sha256(path: &Path) -> Result<String, ValidationError> {
    Ok(digest(&SHA256, &fs::read(path)?)
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

fn read_aot_json(root: &Path, name: &str) -> Result<serde_json::Value, ValidationError> {
    read_bounded_json_file(
        &root.join(name),
        &format!("Leserpent control-plane NativeAOT {name}"),
        64 * 1024,
    )
}

fn require_exact_json_keys(
    value: &serde_json::Value,
    expected: &[&str],
    context: &str,
) -> Result<(), ValidationError> {
    let Some(object) = value.as_object() else {
        return Err(ValidationError::new(format!(
            "{context} must be a JSON object"
        )));
    };
    let observed = object.keys().map(String::as_str).collect::<BTreeSet<_>>();
    let expected = expected.iter().copied().collect::<BTreeSet<_>>();
    if observed != expected {
        return Err(ValidationError::new(format!(
            "{context} contains missing or unexpected fields"
        )));
    }
    Ok(())
}

fn valid_lower_hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn start_ssh_command(
    auth: Option<&RemoteAdminAuth>,
    host: &str,
    remote_command: Option<String>,
) -> Result<Command, ValidationError> {
    validate_remote_host(host)?;
    if let Some(auth) = auth {
        validate_remote_admin_user(&auth.user)?;
    }
    if let Some(command) = remote_command.as_ref() {
        validate_remote_command(command)?;
    }
    let control_path = ssh_control_path_template();
    validate_ssh_control_path_template(&control_path)?;
    match auth {
        Some(auth) => {
            let mut command = Command::new("sshpass");
            command
                .env("SSHPASS", &auth.password)
                .arg("-e")
                .arg("ssh")
                .args(ssh_password_mode_args(&control_path))
                .arg(ssh_auth_target(host, &auth.user));
            if let Some(remote_command) = remote_command {
                command.arg(remote_command);
            }
            Ok(command)
        }
        None => {
            let mut command = Command::new("ssh");
            command.args(ssh_batch_mode_args(&control_path)).arg(host);
            if let Some(remote_command) = remote_command {
                command.arg(remote_command);
            }
            Ok(command)
        }
    }
}

fn run_ssh_command(
    auth: Option<&RemoteAdminAuth>,
    host: &str,
    command: &str,
    context: &str,
) -> Result<(), ValidationError> {
    let status = start_ssh_command(auth, host, Some(command.to_string()))?
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
    let mut child = start_ssh_command(auth, host, Some(command.to_string()))?
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
            let user = validate_remote_admin_user(&user)?;
            let password = validate_remote_admin_password(&password)?;
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
    let trimmed = host.trim();
    if trimmed != host {
        return Err(ValidationError::new(
            "remote host must not include leading or trailing whitespace",
        ));
    }
    if trimmed.is_empty()
        || trimmed.starts_with('-')
        || trimmed.chars().any(|character| {
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

fn validate_remote_workspace_sync_key(
    workspace_sync_key: &str,
) -> Result<String, ValidationError> {
    let trimmed = workspace_sync_key.trim();
    if trimmed.is_empty() {
        return Err(ValidationError::new(
            "remote workspace sync key must not be empty",
        ));
    }
    if trimmed != workspace_sync_key {
        return Err(ValidationError::new(
            "remote workspace sync key must not include surrounding whitespace",
        ));
    }
    if trimmed.len() > REMOTE_WORKSPACE_SYNC_KEY_MAX_LEN {
        return Err(ValidationError::new(
            "remote workspace sync key is too long",
        ));
    }
    if trimmed
        .chars()
        .any(|character| !(character.is_ascii_alphanumeric() || matches!(character, '.' | '-' | ':')))
    {
        return Err(ValidationError::new(
            "remote workspace sync key contains unsafe characters",
        ));
    }
    Ok(trimmed.to_string())
}

fn validate_remote_admin_user(user: &str) -> Result<String, ValidationError> {
    let trimmed = user.trim();
    if trimmed.is_empty() || trimmed != user {
        return Err(ValidationError::new(
            "remote admin user must be a non-empty token without surrounding whitespace",
        ));
    }
    if trimmed.chars().any(|character| {
        !character.is_ascii_alphanumeric() && !matches!(character, '.' | '-' | '_')
    }) {
        return Err(ValidationError::new(
            "remote admin user contains unsafe characters",
        ));
    }
    Ok(trimmed.to_string())
}

fn validate_remote_admin_password(password: &str) -> Result<String, ValidationError> {
    if password.is_empty() {
        return Err(ValidationError::new(
            "remote admin password must be a non-empty value",
        ));
    }
    if password.chars().any(char::is_control) {
        return Err(ValidationError::new(
            "remote admin password must not contain control characters",
        ));
    }
    Ok(password.to_string())
}

fn validate_remote_command(command: &str) -> Result<(), ValidationError> {
    let trimmed = command.trim();
    if trimmed != command {
        return Err(ValidationError::new(
            "remote ssh command must not include surrounding whitespace",
        ));
    }
    if command.is_empty() {
        return Err(ValidationError::new("remote ssh command must not be empty"));
    }
    if command.starts_with('-') {
        return Err(ValidationError::new(
            "remote ssh command must not start with '-'",
        ));
    }
    if command.len() > 8192 {
        return Err(ValidationError::new(
            "remote ssh command is longer than 8192 characters",
        ));
    }
    if command.chars().any(char::is_control) {
        return Err(ValidationError::new(
            "remote ssh command must not contain embedded control characters",
        ));
    }
    Ok(())
}

fn validate_remote_route_device(value: &str) -> Result<String, ValidationError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(ValidationError::new(
            "default route device must not be empty or whitespace",
        ));
    }
    if trimmed != value {
        return Err(ValidationError::new(
            "default route device must not include leading or trailing whitespace",
        ));
    }
    if trimmed.len() > 64 {
        return Err(ValidationError::new("default route device name is too long"));
    }
    if trimmed
        .chars()
        .any(|character| character.is_ascii_control() || character.is_ascii_whitespace())
    {
        return Err(ValidationError::new(
            "default route device must not include control or whitespace characters",
        ));
    }
    if trimmed.chars().any(|character| {
        matches!(
            character,
            ';' | '&'
                | '|'
                | '$'
                | '`'
                | '\\'
                | '"'
                | '\''
                | '<'
                | '>'
                | '('
                | ')'
                | '{'
                | '}'
                | '['
                | ']'
                | '*'
                | '?'
                | '!'
        )
    }) {
        return Err(ValidationError::new(
            "default route device contains unsafe shell characters",
        ));
    }
    Ok(trimmed.to_string())
}

fn validate_release_line(release_line: &str) -> Result<String, ValidationError> {
    if release_line.trim().is_empty() {
        return Err(ValidationError::new("release line must not be empty"));
    }
    if release_line.trim() != release_line {
        return Err(ValidationError::new(
            "release line must not include leading or trailing whitespace",
        ));
    }
    if release_line.chars().any(|character| {
        !(character.is_ascii_alphanumeric() || matches!(character, '.' | '-' | '_' | '+' | 'v'))
    }) {
        return Err(ValidationError::new("release line contains unsafe characters"));
    }
    Ok(release_line.to_string())
}

fn validate_remote_rsync_path(path: &str) -> Result<(), ValidationError> {
    if path.is_empty() {
        return Err(ValidationError::new("remote rsync path must not be empty"));
    }
    if !path.starts_with('/') {
        return Err(ValidationError::new("remote rsync path must be absolute"));
    }
    if path
        .chars()
        .any(|character| character.is_ascii_control() || character.is_ascii_whitespace())
    {
        return Err(ValidationError::new(
            "remote rsync path must not contain control or whitespace characters",
        ));
    }
    if path.chars().any(|character| {
        matches!(
            character,
            ';' | '&' | '|' | '$' | '`' | '\\' | '"' | '\'' | '<' | '>' | '(' | ')' | '{' | '}'
                | '[' | ']' | '*' | '?' | '!' | ':' | '\0'
        )
    }) {
        return Err(ValidationError::new(
            "remote rsync path contains unsafe shell characters",
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
    let workspace_path = shell_single_quote(&workspace_path);
    if let Some(admin_auth) = admin_auth {
        let script = format!(
            r#"set -euo pipefail
printf '%s\n' "$GEWY_REMOTE_SUDO_PASSWORD" | sudo -S -p '' -k rm -rf -- {workspace_path}
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
            &format!("rm -rf -- {workspace_path}"),
            "failed to remove remote workspace",
        )
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct RemotePreflight {
    os: String,
    arch: String,
    kernel: String,
    virtualization: String,
    host_fingerprint: Option<String>,
    home_dir: String,
    required_commands: Vec<String>,
    rustc_version: Option<String>,
    cargo_version: Option<String>,
    dpkg_deb_version: Option<String>,
    rpm_version: Option<String>,
    rpmbuild_version: Option<String>,
    sudo_available: bool,
    ebpf_helper_available: bool,
    ebpf_helper_state: String,
    ebpf_helper_version: Option<String>,
    default_route_device: Option<String>,
}

impl RemotePreflight {
    fn render(&self) -> String {
        format!(
            "os={}\narch={}\nkernel={}\nvirtualization={}\nhost_fingerprint={}\nhome_dir={}\ncommands={}\nrustc_version={}\ncargo_version={}\ndpkg_deb_version={}\nrpm_version={}\nrpmbuild_version={}\nsudo_available={}\nebpf_helper_available={}\nebpf_helper_state={}\nebpf_helper_version={}\ndefault_route_device={}\n",
            self.os,
            self.arch,
            self.kernel,
            self.virtualization,
            self.host_fingerprint.as_deref().unwrap_or(""),
            self.home_dir,
            self.required_commands.join(","),
            self.rustc_version.as_deref().unwrap_or(""),
            self.cargo_version.as_deref().unwrap_or(""),
            self.dpkg_deb_version.as_deref().unwrap_or(""),
            self.rpm_version.as_deref().unwrap_or(""),
            self.rpmbuild_version.as_deref().unwrap_or(""),
            self.sudo_available,
            self.ebpf_helper_available,
            self.ebpf_helper_state,
            self.ebpf_helper_version.as_deref().unwrap_or(""),
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
        "bash",
        "awk",
        "curl",
        "dpkg-deb",
        "rpm",
        "rpm2cpio",
        "cpio",
        "find",
        "flock",
        "grep",
        "ip",
        "mktemp",
        "realpath",
        "sha256sum",
        "sudo",
    ];
    if build_packages {
        required.extend(["cargo", "cargo-clippy", "rustc", "python3", "rpmbuild"]);
    }

    let commands = required.join(" ");
    let script = format!(
        r#"set -euo pipefail
printf 'os=%s\n' "$(uname -s)"
printf 'arch=%s\n' "$(uname -m)"
printf 'kernel=%s\n' "$(uname -r)"
if command -v systemd-detect-virt >/dev/null 2>&1; then
  VM_VIRTUALIZATION=$(systemd-detect-virt --vm 2>/dev/null || true)
  CONTAINER_VIRTUALIZATION=$(systemd-detect-virt --container 2>/dev/null || true)
  VM_VIRTUALIZATION=${{VM_VIRTUALIZATION%%$'\n'*}}
  CONTAINER_VIRTUALIZATION=${{CONTAINER_VIRTUALIZATION%%$'\n'*}}
  [ -n "$VM_VIRTUALIZATION" ] || VM_VIRTUALIZATION=unknown
  [ -n "$CONTAINER_VIRTUALIZATION" ] || CONTAINER_VIRTUALIZATION=unknown
  if [ "$CONTAINER_VIRTUALIZATION" != none ] && [ "$CONTAINER_VIRTUALIZATION" != unknown ]; then
    VIRTUALIZATION=container-$CONTAINER_VIRTUALIZATION
  elif [ "$VM_VIRTUALIZATION" != none ] && [ "$VM_VIRTUALIZATION" != unknown ]; then
    VIRTUALIZATION=$VM_VIRTUALIZATION
  elif [ "$VM_VIRTUALIZATION" = none ] && [ "$CONTAINER_VIRTUALIZATION" = none ]; then
    VIRTUALIZATION=none
  else
    VIRTUALIZATION=unknown
  fi
else
  VIRTUALIZATION=unknown
fi
printf 'virtualization=%s\n' "$VIRTUALIZATION"
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
tool_version() {{
  local value
  command -v "$1" >/dev/null 2>&1 || return 0
  value=$("$1" --version 2>/dev/null || true)
  value=${{value%%$'\n'*}}
  printf '%s' "${{value:0:256}}"
}}
printf 'rustc_version=%s\n' "$(tool_version rustc)"
printf 'cargo_version=%s\n' "$(tool_version cargo)"
printf 'dpkg_deb_version=%s\n' "$(tool_version dpkg-deb)"
printf 'rpm_version=%s\n' "$(tool_version rpm)"
printf 'rpmbuild_version=%s\n' "$(tool_version rpmbuild)"
if sudo -n true >/dev/null 2>&1; then
  printf 'sudo_available=true\n'
else
  printf 'sudo_available=false\n'
fi
HELPER_STATE=missing
HELPER_VERSION=
if [ -x {ebpf_helper} ]; then
  set +e
  HELPER_PROBE=$(sudo -n {ebpf_helper} probe 2>/dev/null)
  HELPER_PROBE_STATUS=$?
  set -e
  if [ "$HELPER_PROBE_STATUS" -ne 0 ]; then
    HELPER_STATE=unavailable
  else
    HELPER_VERSION=$(printf '%s\n' "$HELPER_PROBE" | awk -F= '$1 == "version" {{print $2}}')
    if printf '%s\n' "$HELPER_PROBE" | grep -Fxq 'status=ready' \
      && printf '%s\n' "$HELPER_PROBE" | grep -Fxq 'protocol=1' \
      && [ "$HELPER_VERSION" = {helper_version} ]; then
      HELPER_STATE=ready
    else
      HELPER_STATE=incompatible
    fi
  fi
fi
if [ "$HELPER_STATE" = ready ]; then HELPER_AVAILABLE=true; else HELPER_AVAILABLE=false; fi
printf 'ebpf_helper_available=%s\n' "$HELPER_AVAILABLE"
printf 'ebpf_helper_state=%s\n' "$HELPER_STATE"
printf 'ebpf_helper_version=%s\n' "${{HELPER_VERSION:0:64}}"
DEFAULT_DEV=$(ip route show default 2>/dev/null | awk 'NR==1 {{print $5}}')
printf 'default_route_device=%s\n' "$DEFAULT_DEV"
printf 'commands=%s\n' "{commands}"
"#,
        ebpf_helper = shell_single_quote(REMOTE_EBPF_HELPER),
        helper_version = shell_single_quote(env!("CARGO_PKG_VERSION")),
    );
    let output = run_ssh_script_capture_with_auth(
        auth,
        host,
        "bash -s",
        &script,
        "remote preflight failed",
    )?;
    let preflight = parse_remote_preflight(&output)?;

    require_preflight_tool_version("dpkg-deb", preflight.dpkg_deb_version.as_deref())?;
    require_preflight_tool_version("rpm", preflight.rpm_version.as_deref())?;
    if build_packages {
        require_preflight_tool_version("rustc", preflight.rustc_version.as_deref())?;
        require_preflight_tool_version("cargo", preflight.cargo_version.as_deref())?;
        require_preflight_tool_version("rpmbuild", preflight.rpmbuild_version.as_deref())?;
    }

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

fn require_preflight_tool_version(tool: &str, value: Option<&str>) -> Result<(), ValidationError> {
    if value.is_none() {
        return Err(ValidationError::new(format!(
            "remote preflight could not collect {tool} version"
        )));
    }
    Ok(())
}

fn parse_remote_preflight(output: &str) -> Result<RemotePreflight, ValidationError> {
    let mut values = parse_bounded_unique_key_values(
        output,
        "remote preflight",
        &[
            "os",
            "arch",
            "kernel",
            "virtualization",
            "host_fingerprint",
            "home_dir",
            "commands",
            "rustc_version",
            "cargo_version",
            "dpkg_deb_version",
            "rpm_version",
            "rpmbuild_version",
            "sudo_available",
            "ebpf_helper_available",
            "ebpf_helper_state",
            "ebpf_helper_version",
            "default_route_device",
        ],
    )?;
    let host_fingerprint = values
        .remove("host_fingerprint")
        .filter(|value| !value.is_empty());
    if host_fingerprint
        .as_deref()
        .is_some_and(|value| !valid_host_fingerprint(value))
    {
        return Err(ValidationError::new(
            "remote preflight host fingerprint is invalid",
        ));
    }
    let commands = required_remote_value(&mut values, "commands", "remote preflight")?
        .split_whitespace()
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    if commands.is_empty() {
        return Err(ValidationError::new(
            "remote preflight commands must not be empty",
        ));
    }
    let sudo_available =
        match required_remote_value(&mut values, "sudo_available", "remote preflight")?.as_str() {
            "true" => true,
            "false" => false,
            _ => {
                return Err(ValidationError::new(
                    "remote preflight sudo_available must be true or false",
                ));
            }
        };
    let ebpf_helper_available =
        match required_remote_value(&mut values, "ebpf_helper_available", "remote preflight")?
            .as_str()
        {
            "true" => true,
            "false" => false,
            _ => {
                return Err(ValidationError::new(
                    "remote preflight ebpf_helper_available must be true or false",
                ));
            }
        };
    let ebpf_helper_version = parse_preflight_tool_version(
        "eBPF helper",
        values
            .remove("ebpf_helper_version")
            .as_deref()
            .unwrap_or(""),
    )?;
    let ebpf_helper_state =
        required_remote_value(&mut values, "ebpf_helper_state", "remote preflight")?;
    if !matches!(
        ebpf_helper_state.as_str(),
        "missing" | "unavailable" | "incompatible" | "ready"
    ) {
        return Err(ValidationError::new(
            "remote preflight ebpf_helper_state is invalid",
        ));
    }
    if ebpf_helper_available != (ebpf_helper_state == "ready") {
        return Err(ValidationError::new(
            "remote preflight eBPF helper availability and state disagree",
        ));
    }

    let default_route_device = values
        .remove("default_route_device")
        .filter(|value| !value.is_empty())
        .map(|value| validate_remote_route_device(&value))
        .transpose()?;
    let virtualization = required_remote_value(&mut values, "virtualization", "remote preflight")?;
    if !valid_virtualization(&virtualization) {
        return Err(ValidationError::new(
            "remote preflight virtualization value is invalid",
        ));
    }

    Ok(RemotePreflight {
        os: required_remote_value(&mut values, "os", "remote preflight")?,
        arch: required_remote_value(&mut values, "arch", "remote preflight")?,
        kernel: required_remote_value(&mut values, "kernel", "remote preflight")?,
        virtualization,
        host_fingerprint,
        home_dir: required_remote_value(&mut values, "home_dir", "remote preflight")?,
        required_commands: commands,
        rustc_version: parse_preflight_tool_version(
            "rustc",
            values.remove("rustc_version").as_deref().unwrap_or(""),
        )?,
        cargo_version: parse_preflight_tool_version(
            "cargo",
            values.remove("cargo_version").as_deref().unwrap_or(""),
        )?,
        dpkg_deb_version: parse_preflight_tool_version(
            "dpkg-deb",
            values.remove("dpkg_deb_version").as_deref().unwrap_or(""),
        )?,
        rpm_version: parse_preflight_tool_version(
            "rpm",
            values.remove("rpm_version").as_deref().unwrap_or(""),
        )?,
        rpmbuild_version: parse_preflight_tool_version(
            "rpmbuild",
            values.remove("rpmbuild_version").as_deref().unwrap_or(""),
        )?,
        sudo_available,
        ebpf_helper_available,
        ebpf_helper_state,
        ebpf_helper_version,
        default_route_device,
    })
}

fn required_remote_value(
    values: &mut BTreeMap<String, String>,
    key: &str,
    context: &str,
) -> Result<String, ValidationError> {
    let value = values
        .remove(key)
        .ok_or_else(|| ValidationError::new(format!("{context} missing {key}")))?;
    if value.is_empty() {
        return Err(ValidationError::new(format!(
            "{context} {key} must not be empty"
        )));
    }
    Ok(value)
}

fn parse_remote_phase_timings(
    output: &str,
    context: &str,
    allowed_keys: &[&str],
    required_keys: &[&str],
) -> Result<RemotePhaseTimings, ValidationError> {
    const MAX_SECONDS: f64 = 24.0 * 60.0 * 60.0;

    let raw = parse_bounded_unique_key_values(output, context, allowed_keys)?;
    for key in required_keys {
        if !raw.contains_key(*key) {
            return Err(ValidationError::new(format!("{context} missing {key}")));
        }
    }
    let mut timings = BTreeMap::new();
    for (name, value) in raw {
        let seconds = value
            .parse::<f64>()
            .map_err(|_| ValidationError::new(format!("{context} {name} is not a valid number")))?;
        if !seconds.is_finite() || !(0.0..=MAX_SECONDS).contains(&seconds) {
            return Err(ValidationError::new(format!(
                "{context} {name} must be finite and between 0 and {MAX_SECONDS} seconds"
            )));
        }
        timings.insert(name, seconds);
    }
    Ok(RemotePhaseTimings(timings))
}

fn parse_preflight_tool_version(
    tool: &str,
    value: &str,
) -> Result<Option<String>, ValidationError> {
    if value.is_empty() {
        return Ok(None);
    }
    if value.len() > 256 || value.chars().any(char::is_control) {
        return Err(ValidationError::new(format!(
            "remote preflight {tool} version is invalid"
        )));
    }
    Ok(Some(value.to_string()))
}

fn valid_host_fingerprint(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|digest| {
        digest.len() == 64 && digest.bytes().all(|byte| byte.is_ascii_hexdigit())
    })
}

fn valid_virtualization(value: &str) -> bool {
    value.len() <= 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

fn validate_remote_target_kind(
    requested: RemoteLinuxTargetKind,
    preflight: &RemotePreflight,
) -> Result<(), ValidationError> {
    let detected = RemoteLinuxTargetKind::detect(&preflight.virtualization)?;
    if detected != requested {
        return Err(ValidationError::new(format!(
            "remote target kind mismatch: requested `{}`, detected `{}` ({})",
            requested.as_str(),
            detected.as_str(),
            preflight.virtualization
        )));
    }
    Ok(())
}

fn collect_remote_ebpf_evidence(
    host: &str,
    remote_path: &str,
    preflight: &RemotePreflight,
    admin_auth: Option<&RemoteAdminAuth>,
    phase_timings: &mut Vec<PhaseTiming>,
) -> Result<RemoteEbpfEvidence, ValidationError> {
    if !preflight.ebpf_helper_available && admin_auth.is_none() {
        return Ok(RemoteEbpfEvidence {
            status: "skipped".to_string(),
            reason: format!("privileged_helper_{}", preflight.ebpf_helper_state),
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
    if !preflight.ebpf_helper_available {
        measure_phase(phase_timings, "remote_ebpf_validator_build", || {
            build_remote_ebpf_validator(
                admin_auth,
                host,
                &workspace_path,
                &preflight.home_dir,
                &target_dir,
            )
        })?;
    }
    let output = measure_phase(phase_timings, "remote_ebpf_attach", || {
        run_remote_ebpf_attach(
            admin_auth,
            host,
            &workspace_path,
            &validate_bin,
            &default_route_device,
            preflight.ebpf_helper_available,
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
    helper_available: bool,
) -> Result<String, ValidationError> {
    let workspace_path = shell_single_quote(workspace_path);
    let validate_bin = shell_single_quote(validate_bin);
    let default_route_device_env = shell_single_quote(default_route_device);

    let script = if helper_available {
        let sequence = REMOTE_RUN_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let run_id = format!(
            "remote-{}-{}-{sequence}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs()
        );
        format!(
            r#"set -euo pipefail
cd {workspace_path}
mkdir -p target/validation/remote-ebpf
RUN_ID={run_id}
SOURCE={evidence_root}/$RUN_ID
cleanup() {{ sudo -n {helper} cleanup --run-id "$RUN_ID" >/dev/null 2>&1 || true; }}
trap cleanup EXIT
sudo -n {helper} run --run-id "$RUN_ID" --device {default_route_device}
find "$SOURCE" -type l -print -quit | grep -q . && {{ echo 'helper evidence contains symlink' >&2; exit 31; }}
find "$SOURCE" ! -type d ! -type f -print -quit | grep -q . && {{ echo 'helper evidence contains special file' >&2; exit 32; }}
cp -R -- "$SOURCE"/. target/validation/remote-ebpf/
"#,
            run_id = shell_single_quote(&run_id),
            evidence_root = shell_single_quote(REMOTE_EBPF_EVIDENCE_ROOT),
            helper = shell_single_quote(REMOTE_EBPF_HELPER),
            default_route_device = shell_single_quote(default_route_device),
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
    "$GEWY_VALIDATE_BIN" linux-attach-smoke --out-dir target/validation/remote-ebpf/linux-attach-smoke >&2
    "$GEWY_VALIDATE_BIN" linux-kprobe-smoke --out-dir target/validation/remote-ebpf/linux-kprobe-smoke >&2
    "$GEWY_VALIDATE_BIN" linux-tc-smoke --dev "$GEWY_TC_DEVICE" --out-dir target/validation/remote-ebpf/linux-tc-smoke >&2
  '
    printf 'status=ok\n'
    printf 'reason=all_smokes_passed_admin_ssh\n'
    printf 'default_route_device=%s\n' "$GEWY_TC_DEVICE"
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
        normalize_remote_workspace_path(&format!("{normalized_home}/{REMOTE_WORKSPACE_ROOT}"))?;
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
    let remote_path = shell_single_quote(remote_path);
    let script = format!(
        r#"set -euo pipefail
cd -- {remote_path}
MANIFEST=target/packages/build-manifest.txt
{manifest_helper}
printf 'deb=%s\n' "$(package_from_manifest deb deb)"
printf 'rpm=%s\n' "$(package_from_manifest rpm rpm)"
"#,
        manifest_helper = REMOTE_PACKAGE_MANIFEST_HELPER,
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
) -> Result<RemotePhaseTimings, ValidationError> {
    let remote_path = shell_single_quote(remote_path);
    let script = format!(
        r#"set -euo pipefail
cd -- {remote_path}
cat target/packages/build-timings.txt
"#
    );
    let output = run_ssh_script_capture_with_auth(
        auth,
        host,
        "bash -s",
        &script,
        "remote package build timings missing; rerun without --skip-build or inspect target/packages/build-timings.txt on the host",
    )?;
    parse_remote_phase_timings(
        &output,
        "remote package build timings",
        &["release_build", "stage_layout", "package_all", "total"],
        &["release_build", "stage_layout", "package_all", "total"],
    )
}

fn collect_remote_package_smoke_timings(
    auth: Option<&RemoteAdminAuth>,
    host: &str,
    remote_path: &str,
) -> Result<RemotePhaseTimings, ValidationError> {
    let remote_path = shell_single_quote(remote_path);
    let script = format!(
        r#"set -euo pipefail
cd -- {remote_path}
cat target/packages/package-smoke-timings.txt
"#
    );
    let output = run_ssh_script_capture_with_auth(
        auth,
        host,
        "bash -s",
        &script,
        "remote package smoke timings missing; rerun the package smoke or inspect target/packages/package-smoke-timings.txt on the host",
    )?;
    parse_remote_phase_timings(
        &output,
        "remote package smoke timings",
        &[
            "deb_list_contents",
            "deb_unpack_cache_refresh",
            "deb_verify",
            "rpm_list_contents",
            "rpm_unpack_cache_refresh",
            "rpm_verify",
            "total",
        ],
        &[
            "deb_list_contents",
            "deb_verify",
            "rpm_list_contents",
            "rpm_verify",
            "total",
        ],
    )
}

fn collect_remote_runtime_smoke_timings(
    auth: Option<&RemoteAdminAuth>,
    host: &str,
    remote_path: &str,
) -> Result<RemotePhaseTimings, ValidationError> {
    let remote_path = shell_single_quote(remote_path);
    let script = format!(
        r#"set -euo pipefail
cd -- {remote_path}
cat target/packages/runtime-smoke-timings.txt
"#
    );
    let output = run_ssh_script_capture_with_auth(
        auth,
        host,
        "bash -s",
        &script,
        "remote runtime smoke timings missing; rerun the runtime smoke or inspect target/packages/runtime-smoke-timings.txt on the host",
    )?;
    parse_remote_phase_timings(
        &output,
        "remote runtime smoke timings",
        &[
            "unpack_cache_refresh",
            "tcp_boot_health",
            "udp_boot_health",
            "tcp_summary",
            "udp_summary",
            "udp_analysis",
            "tcp_health_after_bad",
            "tcp_analysis",
            "total",
        ],
        &[
            "tcp_boot_health",
            "udp_boot_health",
            "tcp_summary",
            "udp_summary",
            "udp_analysis",
            "tcp_health_after_bad",
            "tcp_analysis",
            "total",
        ],
    )
}

fn parse_remote_artifact_manifest(output: &str) -> Result<RemoteArtifactManifest, ValidationError> {
    let mut values =
        parse_bounded_unique_key_values(output, "remote artifact manifest", &["deb", "rpm"])?;

    Ok(RemoteArtifactManifest {
        deb: required_remote_value(&mut values, "deb", "remote artifact manifest")?,
        rpm: required_remote_value(&mut values, "rpm", "remote artifact manifest")?,
    })
}

fn parse_remote_ebpf_evidence(output: &str) -> Result<RemoteEbpfEvidence, ValidationError> {
    let mut values = parse_bounded_unique_key_values(
        output,
        "remote eBPF evidence",
        &["status", "reason", "default_route_device"],
    )?;

    Ok(RemoteEbpfEvidence {
        status: required_remote_value(&mut values, "status", "remote eBPF evidence")?,
        reason: required_remote_value(&mut values, "reason", "remote eBPF evidence")?,
        default_route_device: values
            .remove("default_route_device")
            .filter(|value| !value.is_empty()),
    })
}

fn shell_single_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', r#"'\''"#))
}

const REMOTE_PACKAGE_MANIFEST_HELPER: &str = r#"package_from_manifest() {
  local key="$1"
  local extension="$2"
  local candidate package_root count value resolved

  [ -f "$MANIFEST" ] && [ ! -L "$MANIFEST" ] || {
    echo "package manifest must be a regular non-symlink file: $MANIFEST" >&2
    return 20
  }
  awk -F= 'index($0, "=") == 0 || $1 == "" || substr($0, length($1) + 2) == "" || ($1 != "deb" && $1 != "rpm") { exit 1 }' "$MANIFEST" || {
    echo "package manifest contains a malformed entry: $MANIFEST" >&2
    return 21
  }
  count=$(awk -F= -v wanted="$key" '$1 == wanted { count++ } END { print count + 0 }' "$MANIFEST")
  [ "$count" -eq 1 ] || {
    echo "package manifest must contain exactly one $key entry: $MANIFEST" >&2
    return 22
  }
  value=$(awk -F= -v wanted="$key" '$1 == wanted { print substr($0, length($1) + 2) }' "$MANIFEST")
  case "$value" in
    /*) candidate="$value" ;;
    *) candidate="$(dirname "$MANIFEST")/$value" ;;
  esac
  [ -f "$candidate" ] && [ ! -L "$candidate" ] || {
    echo "package manifest $key entry must reference a regular non-symlink file: $value" >&2
    return 23
  }
  package_root=$(realpath target/packages)
  resolved=$(realpath "$candidate")
  case "$resolved" in
    "$package_root"/*) ;;
    *)
      echo "package manifest $key entry escapes package root: $value" >&2
      return 24
      ;;
  esac
  case "$resolved" in
    *."$extension") ;;
    *)
      echo "package manifest $key entry has the wrong extension: $value" >&2
      return 25
      ;;
  esac
  printf '%s\n' "$resolved"
}"#;

const REMOTE_LESERPENT_LANGUAGE_PACK_LOCAL_ORCHESTRA_AOT_SCRIPT: &str = r#"set -euo pipefail
EVIDENCE="$(pwd)/target/packages/leserpent-language-pack-local-orchestra-native-aot-linux-x64"
PUBLISH="$EVIDENCE/publish"
DOTNET_ARTIFACTS="$EVIDENCE/dotnet-artifacts"
DAEMON="$CARGO_TARGET_DIR/release/leserpentd"
cleanup() {
  find "$PUBLISH" "$DOTNET_ARTIFACTS" -depth -delete 2>/dev/null || true
}
trap cleanup EXIT
mkdir -p "$EVIDENCE"
find "$EVIDENCE" -mindepth 1 -depth -delete
mkdir -p "$PUBLISH"

cargo build --locked --quiet --release -p leserpentd --bin leserpentd \
  --features leserpentd/native-ssh >"$EVIDENCE/daemon-build.log" 2>&1
printf 'cargo build completed\n' >>"$EVIDENCE/daemon-build.log"
dotnet restore apps/leserpent-avalonia/src/Leserpent.Avalonia/Leserpent.Avalonia.csproj \
  -p:PublishProfile=NativeAot \
  -p:PublishAot=true \
  -p:RuntimeIdentifier=linux-x64 \
  --locked-mode \
  --artifacts-path "$DOTNET_ARTIFACTS" >"$EVIDENCE/restore.log" 2>&1
printf 'dotnet restore completed\n' >>"$EVIDENCE/restore.log"
dotnet publish apps/leserpent-avalonia/src/Leserpent.Avalonia/Leserpent.Avalonia.csproj \
  -p:PublishProfile=NativeAot \
  -p:PublishAot=true \
  -p:RuntimeIdentifier=linux-x64 \
  --no-restore \
  --artifacts-path "$DOTNET_ARTIFACTS" \
  -o "$PUBLISH" >"$EVIDENCE/publish.log" 2>&1
printf 'dotnet publish completed\n' >>"$EVIDENCE/publish.log"

test -f "$PUBLISH/Leserpent.Avalonia" && test ! -L "$PUBLISH/Leserpent.Avalonia"
test -x "$PUBLISH/Leserpent.Avalonia"
test -f "$DAEMON" && test ! -L "$DAEMON" && test -x "$DAEMON"
cp -- "$DAEMON" "$PUBLISH/leserpentd"
chmod 0755 "$PUBLISH/leserpentd"
file "$PUBLISH/Leserpent.Avalonia" | grep -q 'ELF 64-bit.*x86-64'
file "$PUBLISH/leserpentd" | grep -q 'ELF 64-bit.*x86-64'

(
  cd -- "$PUBLISH"
  sha256sum Leserpent.Avalonia leserpentd
) >"$EVIDENCE/payload.sha256"
(
  cd -- apps/leserpent/src/Leserpent/wwwroot/language-packs
  test -f catalog.json && test ! -L catalog.json
  test -f pt-BR.json && test ! -L pt-BR.json
  sha256sum catalog.json pt-BR.json
) >"$EVIDENCE/language-pack-assets.sha256"

"$PUBLISH/Leserpent.Avalonia" \
  --verify-local-orchestra "$PUBLISH/leserpentd" \
  >"$EVIDENCE/verification.log" 2>&1
grep -q 'credential_free_language_pack_download=true' "$EVIDENCE/verification.log"
grep -q 'language_pack_digest_binding=true' "$EVIDENCE/verification.log"
grep -q 'language_pack_private_roundtrip=true' "$EVIDENCE/verification.log"
grep -q 'process_cleanup=true' "$EVIDENCE/verification.log"

printf 'os=Linux\narch=%s\nrid=linux-x64\nkernel=%s\ndotnet_sdk=%s\nrustc=%s\ncargo=%s\navalonia_bytes=%s\nleserpentd_bytes=%s\n' \
  "$(uname -m)" \
  "$(uname -r)" \
  "$(dotnet --version)" \
  "$(rustc --version)" \
  "$(cargo --version)" \
  "$(stat -c %s "$PUBLISH/Leserpent.Avalonia")" \
  "$(stat -c %s "$PUBLISH/leserpentd")" \
  >"$EVIDENCE/environment.txt"
cat >"$EVIDENCE/evidence-index.json" <<'JSON'
{
  "schema_version": 1,
  "proof": "leserpent-language-pack-local-orchestra-native-aot-linux-x64",
  "result": "passed",
  "files": [
    "environment.txt",
    "restore.log",
    "publish.log",
    "daemon-build.log",
    "payload.sha256",
    "language-pack-assets.sha256",
    "verification.log"
  ]
}
JSON
echo 'remote Leserpent Local Orchestra language-pack NativeAOT proof: ok'
"#;

const REMOTE_LESERPENT_CONTROL_PLANE_AOT_SCRIPT: &str = r#"set -euo pipefail
EVIDENCE="$(pwd)/target/packages/leserpent-control-plane-aot-linux-x64"
PUBLISH="$EVIDENCE/publish"
DOTNET_ARTIFACTS="$EVIDENCE/dotnet-artifacts"
STATE="$EVIDENCE/runtime-state.json"
DATABASE="$EVIDENCE/orchestra.db"
PID=""
cleanup() {
  if [ -n "$PID" ]; then
    kill "$PID" >/dev/null 2>&1 || true
    wait "$PID" >/dev/null 2>&1 || true
  fi
  find "$PUBLISH" "$DOTNET_ARTIFACTS" -depth -delete 2>/dev/null || true
  find "$EVIDENCE" -maxdepth 1 -type f \( -name 'runtime-state.json.*' -o -name 'orchestra.db-*' \) -delete
}
trap cleanup EXIT
mkdir -p "$EVIDENCE"
find "$EVIDENCE" -mindepth 1 -depth -delete
mkdir -p "$PUBLISH"

dotnet restore apps/leserpent/src/Leserpent/Leserpent.csproj \
  -p:PublishProfile=native-aot \
  -p:PublishAot=true \
  -p:RuntimeIdentifier=linux-x64 \
  --locked-mode \
  --artifacts-path "$DOTNET_ARTIFACTS" >"$EVIDENCE/restore.log" 2>&1
dotnet publish apps/leserpent/src/Leserpent/Leserpent.csproj \
  -p:PublishProfile=native-aot \
  -p:PublishAot=true \
  -p:RuntimeIdentifier=linux-x64 \
  --no-restore \
  --artifacts-path "$DOTNET_ARTIFACTS" \
  -o "$PUBLISH" >"$EVIDENCE/publish.log" 2>&1

for required in Leserpent leserpent-compat-bridge leserpentd libe_sqlite3.so; do
  test -f "$PUBLISH/$required" && test ! -L "$PUBLISH/$required"
done
test -x "$PUBLISH/Leserpent"
test -x "$PUBLISH/leserpent-compat-bridge"
test -x "$PUBLISH/leserpentd"
file "$PUBLISH/Leserpent" | grep -q 'ELF 64-bit.*x86-64'
file "$PUBLISH/leserpent-compat-bridge" | grep -q 'ELF 64-bit.*x86-64'
file "$PUBLISH/leserpentd" | grep -q 'ELF 64-bit.*x86-64'
sha256sum "$PUBLISH/Leserpent" "$PUBLISH/leserpent-compat-bridge" \
  "$PUBLISH/leserpentd" "$PUBLISH/libe_sqlite3.so" >"$EVIDENCE/payload.sha256"

PORT=$((40000 + ($$ % 20000)))
env ASPNETCORE_URLS="http://127.0.0.1:$PORT" \
  LESERPENT_STATE_PATH="$STATE" \
  LESERPENT_DATABASE_PATH="$DATABASE" \
  "$PUBLISH/Leserpent" >"$EVIDENCE/service.log" 2>&1 &
PID=$!
for _ in $(seq 1 120); do
  if curl -fsS "http://127.0.0.1:$PORT/health" >"$EVIDENCE/health.json"; then
    break
  fi
  sleep 0.1
done
grep -q '"ok":true' "$EVIDENCE/health.json"
grep -q '"coreReady":true' "$EVIDENCE/health.json"

curl -fsS -X POST \
  -H 'Content-Type: application/json' \
  -H 'X-Leserpent-Intent: mutate' \
  --data '{"name":"native-offline","endpoint":"http://127.0.0.1:9"}' \
  "http://127.0.0.1:$PORT/v1/runtimes/registration-plan" >"$EVIDENCE/registration-plan.json"
grep -q '"allowed":true' "$EVIDENCE/registration-plan.json"
grep -q '"action":"create"' "$EVIDENCE/registration-plan.json"
PLAN_TOKEN=$(sed -n 's/.*"planToken":"\([a-f0-9]*\)".*/\1/p' "$EVIDENCE/registration-plan.json")
test "${#PLAN_TOKEN}" -eq 64

curl -fsS -X POST \
  -H 'Content-Type: application/json' \
  -H 'X-Leserpent-Intent: mutate' \
  --data "{\"name\":\"native-offline\",\"endpoint\":\"http://127.0.0.1:9\",\"pairingToken\":\"native-aot-proof-secret\",\"fetchCapabilities\":false,\"registrationPlanToken\":\"$PLAN_TOKEN\"}" \
  "http://127.0.0.1:$PORT/v1/runtimes/register" >"$EVIDENCE/registration.json"
RUNTIME_ID=$(sed -n 's/.*"runtimeId":"\([a-f0-9]*\)".*/\1/p' "$EVIDENCE/registration.json")
test "${#RUNTIME_ID}" -eq 32

curl -fsS -X POST \
  -H 'Content-Type: application/json' \
  -H 'X-Leserpent-Intent: mutate' \
  --data '{"kind":"all"}' \
  "http://127.0.0.1:$PORT/v1/runtimes/$RUNTIME_ID/recovery" >"$EVIDENCE/recovery.json"
grep -q '"kind":"all"' "$EVIDENCE/recovery.json"
grep -q '"outcome":"degraded"' "$EVIDENCE/recovery.json"
test "$(grep -o '"kind":"\(capabilities\|status\)"' "$EVIDENCE/recovery.json" | wc -l)" -eq 2
curl -fsS "http://127.0.0.1:$PORT/v1/runtimes/$RUNTIME_ID/attention" >"$EVIDENCE/attention.json"
grep -q '"action":"refresh_all"' "$EVIDENCE/attention.json"
grep -q '"commandKind":"all"' "$EVIDENCE/attention.json"
kill -0 "$PID"
kill "$PID"
wait "$PID" || true
PID=""
find "$EVIDENCE" -maxdepth 1 -type f \( -name 'runtime-state.json.*' -o -name 'orchestra.db-*' \) -delete
test -f "$STATE" && test ! -L "$STATE"
test -f "$DATABASE" && test ! -L "$DATABASE"
if grep -a -q 'native-aot-proof-secret' "$STATE" "$DATABASE"; then
  echo 'NativeAOT proof secret was persisted' >&2
  exit 31
fi

printf 'os=linux\narch=x86_64\nrid=linux-x64\n' >"$EVIDENCE/environment.txt"
cat >"$EVIDENCE/evidence-index.json" <<'JSON'
{
  "schema_version": 1,
  "proof": "leserpent-control-plane-native-aot-linux-x64",
  "result": "passed",
  "files": [
    "environment.txt",
    "restore.log",
    "publish.log",
    "payload.sha256",
    "service.log",
    "health.json",
    "registration-plan.json",
    "registration.json",
    "recovery.json",
    "attention.json",
    "runtime-state.json",
    "orchestra.db"
  ]
}
JSON
echo 'remote Leserpent control-plane NativeAOT proof: ok'
"#;

fn remote_package_smoke_script(release_line: &str) -> String {
    let release_line = shell_single_quote(release_line);
    format!(
        r#"set -euo pipefail
MANIFEST=target/packages/build-manifest.txt
{manifest_helper}
RELEASE_LINE={release_line}
DEB=$(package_from_manifest deb deb)
RPM=$(package_from_manifest rpm rpm)
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
grep -q "^release_line = \"${{RELEASE_LINE}}\"$" "$DEB_ROOT/usr/share/gewyvern/package-compat.toml"
test -f "$DEB_ROOT/usr/share/gewyvern/examples/gewyvern.toml.example"
test -x "$DEB_ROOT/usr/libexec/gewyvern-ebpf-helper"
test -x "$DEB_ROOT/usr/sbin/gewyvern-ebpf-provision"
test -f "$DEB_ROOT/usr/share/gewyvern/examples/ebpf-helper.conf.example"
test -f "$DEB_ROOT/usr/share/gewyvern/examples/gewyvern-ebpf-validation.sudoers.example"
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
grep -q "^release_line = \"${{RELEASE_LINE}}\"$" "$RPM_ROOT/usr/share/gewyvern/package-compat.toml"
test -f "$RPM_ROOT/usr/share/gewyvern/examples/gewyvern.toml.example"
test -x "$RPM_ROOT/usr/libexec/gewyvern-ebpf-helper"
test -x "$RPM_ROOT/usr/sbin/gewyvern-ebpf-provision"
test -f "$RPM_ROOT/usr/share/gewyvern/examples/ebpf-helper.conf.example"
test -f "$RPM_ROOT/usr/share/gewyvern/examples/gewyvern-ebpf-validation.sudoers.example"
record_timing rpm_verify "$(duration_seconds "$RPM_VERIFY_STARTED" "$(now_seconds)")"
record_timing total "$(duration_seconds "$DEB_LIST_STARTED" "$(now_seconds)")"
echo 'remote package smoke: ok'
"#,
        manifest_helper = REMOTE_PACKAGE_MANIFEST_HELPER,
        release_line = release_line,
    )
}

fn remote_runtime_smoke_script() -> String {
    let mut script = String::from(
        r#"set -euo pipefail
MANIFEST=target/packages/build-manifest.txt
"#,
    );
    script.push_str(REMOTE_PACKAGE_MANIFEST_HELPER);
    script.push_str(
        r#"
DEB=$(package_from_manifest deb deb)
"#,
    );
    script.push_str(REMOTE_RUNTIME_SMOKE_BODY);
    script
}

const REMOTE_RUNTIME_SMOKE_BODY: &str = r#"TIMINGS=target/packages/runtime-smoke-timings.txt
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
    let mut ssh_command = start_ssh_command(auth, host, Some(command.to_string()))?;

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

fn rsync_remote_target(
    auth: Option<&RemoteAdminAuth>,
    host: &str,
    remote_path: &str,
) -> Result<String, ValidationError> {
    validate_remote_host(host)?;
    validate_remote_rsync_path(remote_path)?;
    let target = auth
        .map(|auth| ssh_auth_target(host, &auth.user))
        .unwrap_or_else(|| host.to_string());
    Ok(format!("{target}:{remote_path}"))
}

#[cfg(test)]
mod tests {
    use super::{
        MAX_SSH_CONTROL_PATH_BYTES, REMOTE_LESERPENT_CONTROL_PLANE_AOT_SCRIPT,
        REMOTE_LESERPENT_LANGUAGE_PACK_LOCAL_ORCHESTRA_AOT_SCRIPT,
        REMOTE_PACKAGE_MANIFEST_HELPER, RemoteAdminAuth, SSH_CONTROL_TEMP_SUFFIX_RESERVE,
        RemoteLinuxTargetKind,
        acquire_remote_ebpf_history_lock, acquire_remote_validation_run_lock,
        atomic_write_evidence, default_remote_dir, default_ssh_control_path_template,
        is_relevant_workspace_path, local_workspace_sync_cache_path,
        parse_remote_artifact_manifest, parse_remote_ebpf_evidence, parse_remote_phase_timings,
        parse_remote_preflight, read_remote_ebpf_history, remote_package_smoke_script,
        remote_runtime_smoke_script, resolve_remote_execution_path, resolve_remote_workspace_path,
        rsync_remote_target, ssh_auth_target, ssh_password_mode_args, summarize_remote_ebpf_history,
        validate_leserpent_control_plane_aot_evidence,
        validate_leserpent_language_pack_local_orchestra_aot_evidence,
        validate_remote_admin_user, validate_remote_admin_password, validate_release_line,
        validate_remote_command, validate_remote_dir, validate_remote_host,
        validate_remote_route_device, validate_remote_target_kind, validate_remote_workspace_sync_key,
        validate_ssh_control_path_template,
    };
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::sync::mpsc;
    use std::thread;
    use std::time::Duration;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn default_ssh_control_path_reserves_unix_socket_suffix_budget() {
        let path = default_ssh_control_path_template();
        validate_ssh_control_path_template(&path).unwrap();
        assert!(Path::new(&path).is_absolute());
        assert!(path.contains("%C"));
        #[cfg(unix)]
        assert!(
            path.len() - 2 + 40 + SSH_CONTROL_TEMP_SUFFIX_RESERVE <= MAX_SSH_CONTROL_PATH_BYTES
        );
    }

    #[test]
    fn workspace_sync_caches_are_isolated_from_evidence_shelves() {
        let physical = local_workspace_sync_cache_path(RemoteLinuxTargetKind::Physical);
        let vm = local_workspace_sync_cache_path(RemoteLinuxTargetKind::Vm);

        assert_ne!(physical, vm);
        assert!(
            physical.ends_with("target/validation/remote-workspace-sync-cache/physical.txt")
        );
        assert!(vm.ends_with("target/validation/remote-workspace-sync-cache/vm.txt"));
        for path in [&physical, &vm] {
            let rendered = path.to_string_lossy();
            assert!(!rendered.contains("remote-linux-host-validation"));
            assert!(!rendered.contains("remote-linux-vm-validation"));
        }
    }

    #[test]
    fn ssh_control_path_rejects_unbounded_or_unsafe_templates() {
        assert!(validate_ssh_control_path_template("relative/%C").is_err());
        assert!(validate_ssh_control_path_template("/tmp/gewy-%h").is_err());
        assert!(validate_ssh_control_path_template("/tmp/gewy-%C\n-oProxyCommand=bad").is_err());
        assert!(
            validate_ssh_control_path_template(&format!(
                "/tmp/{}-%C",
                "x".repeat(MAX_SSH_CONTROL_PATH_BYTES)
            ))
            .is_err()
        );
    }

    #[test]
    fn ssh_auth_target_replaces_existing_user_prefix() {
        assert_eq!(
            ssh_auth_target("builder@192.0.2.10", "administrator"),
            "administrator@192.0.2.10"
        );
    }

    #[test]
    fn ssh_auth_target_adds_user_when_host_has_no_prefix() {
        assert_eq!(
            ssh_auth_target("gewyvern-lab", "administrator"),
            "administrator@gewyvern-lab"
        );
    }

    #[test]
    fn rsync_target_uses_the_same_admin_identity_as_ssh() {
        let auth = RemoteAdminAuth {
            user: "administrator".to_string(),
            password: "not-exposed".to_string(),
        };
        assert_eq!(
            rsync_remote_target(Some(&auth), "builder@192.0.2.10", "/tmp/evidence/").unwrap(),
            "administrator@192.0.2.10:/tmp/evidence/"
        );
        assert_eq!(
            rsync_remote_target(None, "gewyvern-lab", "/tmp/evidence/").unwrap(),
            "gewyvern-lab:/tmp/evidence/"
        );
    }

    #[test]
    fn rsync_target_rejects_unsafe_paths_and_hosts() {
        let auth = RemoteAdminAuth {
            user: "administrator".to_string(),
            password: "not-exposed".to_string(),
        };
        assert!(rsync_remote_target(
            Some(&auth),
            "builder@192.0.2.10",
            "../tmp/evidence/",
        )
        .is_err());
        assert!(rsync_remote_target(Some(&auth), "builder@192.0.2.10", "/tmp/evidence;/usr/bin").is_err());
        assert!(rsync_remote_target(Some(&auth), "-oProxyCommand=bad", "/tmp/evidence/").is_err());
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

        let summary = summarize_remote_ebpf_history(&lines, 3, 1, RemoteLinuxTargetKind::Physical);
        let matrix = &summary["matrix"];
        assert_eq!(summary["integrity"]["status"], "repaired");
        assert_eq!(summary["integrity"]["rejected_entries"], 3);
        assert_eq!(summary["integrity"]["rejected_entries_this_run"], 1);
        assert_eq!(matrix["ready"], true);
        assert_eq!(matrix["breadth_ready"], true);
        assert_eq!(matrix["release_eligible"], true);
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
        let mut vm_value: serde_json::Value = serde_json::from_str(&valid).unwrap();
        vm_value["target_kind"] = serde_json::json!("vm");
        let vm_valid = serde_json::to_string(&vm_value).unwrap();
        fs::write(
            &history_path,
            format!("{valid}\n{vm_valid}\nnot-json\n{{\"schema_version\":99}}\n"),
        )
        .unwrap();

        let (accepted, rejected) =
            read_remote_ebpf_history(&history_path, RemoteLinuxTargetKind::Physical).unwrap();
        assert_eq!(accepted, vec![valid]);
        assert_eq!(rejected.len(), 3);
        let (accepted, rejected) =
            read_remote_ebpf_history(&history_path, RemoteLinuxTargetKind::Vm).unwrap();
        assert_eq!(accepted, vec![vm_valid]);
        assert_eq!(rejected.len(), 3);

        atomic_write_evidence(&history_path, "replacement\n").unwrap();
        assert_eq!(fs::read_to_string(&history_path).unwrap(), "replacement\n");
        assert_eq!(fs::read_dir(&root).unwrap().count(), 1);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn vm_history_is_isolated_and_never_release_eligible() {
        let fingerprint_a = format!("sha256:{}", "a".repeat(64));
        let fingerprint_b = format!("sha256:{}", "b".repeat(64));
        let lines = vec![
            serde_json::json!({"target_kind":"vm","host":"vm-a","preflight":{"host_fingerprint":fingerprint_a,"kernel":"6.8.0","arch":"x86_64"},"ebpf":{"status":"ok","reason":"passed"}}).to_string(),
            serde_json::json!({"target_kind":"vm","host":"vm-b","preflight":{"host_fingerprint":fingerprint_b,"kernel":"6.9.0","arch":"x86_64"},"ebpf":{"status":"ok","reason":"passed"}}).to_string(),
        ];

        let summary = summarize_remote_ebpf_history(&lines, 0, 0, RemoteLinuxTargetKind::Vm);
        assert_eq!(summary["target_kind"], "vm");
        assert_eq!(summary["matrix"]["breadth_ready"], true);
        assert_eq!(summary["matrix"]["release_eligible"], false);
        assert_eq!(summary["matrix"]["ready"], false);
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
        let args = ssh_password_mode_args(&default_ssh_control_path_template());
        assert!(
            args.iter()
                .any(|arg| arg == "StrictHostKeyChecking=accept-new")
        );
        assert!(!args.iter().any(|arg| arg == "StrictHostKeyChecking=no"));
    }

    #[test]
    fn remote_host_rejects_ssh_option_and_shell_injection_shapes() {
        assert!(validate_remote_host("builder@192.0.2.10").is_ok());
        assert!(validate_remote_host("[fd00::12]").is_ok());
        assert!(validate_remote_host("-oProxyCommand=sh").is_err());
        assert!(validate_remote_host("host; touch /tmp/pwned").is_err());
        assert!(validate_remote_host(" builder@192.0.2.10").is_err());
        assert!(validate_remote_host("builder@192.0.2.10 ").is_err());
    }

    #[test]
    fn remote_admin_user_rejects_shell_and_whitespace() {
        assert!(validate_remote_admin_user("builder").is_ok());
        assert!(validate_remote_admin_user("  builder ").is_err());
        assert!(validate_remote_admin_user("builder;id").is_err());
    }

    #[test]
    fn remote_admin_password_rejects_control_characters() {
        assert!(validate_remote_admin_password("s3cur3-p4ss!").is_ok());
        assert!(validate_remote_admin_password("line1\nline2").is_err());
        assert!(validate_remote_admin_password("\twith-tab").is_err());
        assert!(validate_remote_admin_password("").is_err());
    }

    #[test]
    fn remote_dir_rejects_unsafe_values() {
        assert!(validate_remote_dir("~/.gewyvern-remote-runs").is_ok());
        assert!(validate_remote_dir("/home/user/gewyvern").is_ok());
        assert!(validate_remote_dir("gewyvern_remote/2026-01").is_ok());
        assert!(validate_remote_dir("").is_err());
        assert!(validate_remote_dir("   ").is_err());
        assert!(validate_remote_dir("name;rm -rf /").is_err());
        assert!(validate_remote_dir(&"a".repeat(300)).is_err());
    }

    #[test]
    fn remote_workspace_sync_key_rejects_unsafe_values() {
        assert!(validate_remote_workspace_sync_key("git:abc123").is_ok());
        assert!(validate_remote_workspace_sync_key(&format!("git-dirty:{}", "a".repeat(64))).is_ok());
        assert!(validate_remote_workspace_sync_key("").is_err());
        assert!(validate_remote_workspace_sync_key("  git:abc").is_err());
        assert!(validate_remote_workspace_sync_key("git:abc;rm -rf /").is_err());
        assert!(validate_remote_workspace_sync_key(&"x".repeat(300)).is_err());
    }

    #[test]
    fn remote_command_rejects_embedded_controls() {
        assert!(validate_remote_command("bash -s").is_ok());
        assert!(validate_remote_command(" bash -s").is_err());
        assert!(validate_remote_command("bash -s ").is_err());
        assert!(validate_remote_command("-s").is_err());
        assert!(validate_remote_command(&"x".repeat(8_000)).is_ok());
        assert!(validate_remote_command(&"x".repeat(8_200)).is_err());
        assert!(validate_remote_command("bash -s\nrm -rf /").is_err());
        assert!(validate_remote_command("").is_err());
    }

    #[test]
    fn remote_route_device_rejects_unsafe_values() {
        assert!(validate_remote_route_device("eth0").is_ok());
        assert!(validate_remote_route_device("  eth0").is_err());
        assert!(validate_remote_route_device("eth0;rm -rf /").is_err());
        assert!(validate_remote_route_device("eth0$(id)").is_err());
    }

    #[test]
    fn remote_release_line_rejects_unsafe_values() {
        assert!(validate_release_line("v1.16.0").is_ok());
        assert!(validate_release_line("  v1.16.0").is_err());
        assert!(validate_release_line("v1.16.0;rm -rf /").is_err());
        assert!(validate_release_line("").is_err());
    }

    #[test]
    fn parse_remote_preflight_accepts_linux_x86_64_manifest() {
        let preflight = parse_remote_preflight(
            "os=Linux\narch=x86_64\nkernel=6.8.0\nvirtualization=none\nhost_fingerprint=sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\nhome_dir=/home/gewyvern-lab\nsudo_available=true\nebpf_helper_available=true\nebpf_helper_state=ready\nebpf_helper_version=1.4.6\ndefault_route_device=eth0\ncommands=bash curl cargo rustc python3 rpmbuild\nrustc_version=rustc 1.95.0\ncargo_version=cargo 1.95.0\ndpkg_deb_version=Debian dpkg-deb 1.22.6\nrpm_version=RPM version 4.19.1\nrpmbuild_version=RPM version 4.19.1\n",
        )
        .unwrap();

        assert_eq!(preflight.os, "Linux");
        assert_eq!(preflight.arch, "x86_64");
        assert_eq!(preflight.kernel, "6.8.0");
        assert_eq!(preflight.virtualization, "none");
        validate_remote_target_kind(RemoteLinuxTargetKind::Physical, &preflight).unwrap();
        assert!(validate_remote_target_kind(RemoteLinuxTargetKind::Vm, &preflight).is_err());
        let mut vm_preflight = preflight.clone();
        vm_preflight.virtualization = "kvm".to_string();
        validate_remote_target_kind(RemoteLinuxTargetKind::Vm, &vm_preflight).unwrap();
        assert!(
            validate_remote_target_kind(RemoteLinuxTargetKind::Physical, &vm_preflight).is_err()
        );
        vm_preflight.virtualization = "container-docker".to_string();
        let container_error =
            validate_remote_target_kind(RemoteLinuxTargetKind::Vm, &vm_preflight).unwrap_err();
        assert!(
            container_error
                .to_string()
                .contains("remote container targets are unsupported")
        );
        assert_eq!(
            preflight.host_fingerprint.as_deref(),
            Some("sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
        );
        assert_eq!(preflight.home_dir, "/home/gewyvern-lab");
        assert!(preflight.sudo_available);
        assert!(preflight.ebpf_helper_available);
        assert_eq!(preflight.ebpf_helper_state, "ready");
        assert_eq!(preflight.ebpf_helper_version.as_deref(), Some("1.4.6"));
        assert_eq!(preflight.default_route_device.as_deref(), Some("eth0"));
        assert!(preflight.required_commands.contains(&"cargo".to_string()));
        assert_eq!(preflight.rustc_version.as_deref(), Some("rustc 1.95.0"));
        assert_eq!(preflight.cargo_version.as_deref(), Some("cargo 1.95.0"));
        assert_eq!(
            preflight.dpkg_deb_version.as_deref(),
            Some("Debian dpkg-deb 1.22.6")
        );
    }

    #[test]
    fn parse_remote_preflight_rejects_unbounded_tool_versions() {
        let manifest = format!(
            "os=Linux\narch=x86_64\nkernel=6.8.0\nvirtualization=none\nhost_fingerprint=\nhome_dir=/home/test\ncommands=bash cargo rustc\nrustc_version={}\nsudo_available=false\nebpf_helper_available=false\nebpf_helper_state=missing\nebpf_helper_version=\ndefault_route_device=\n",
            "x".repeat(257)
        );
        let error = parse_remote_preflight(&manifest).unwrap_err();
        assert!(error.to_string().contains("rustc version is invalid"));
    }

    #[test]
    fn parse_remote_preflight_rejects_ambiguous_or_unknown_entries() {
        let base = "os=Linux\narch=x86_64\nkernel=6.8.0\nvirtualization=none\nhost_fingerprint=\nhome_dir=/home/test\ncommands=bash\nsudo_available=false\nebpf_helper_available=false\nebpf_helper_state=missing\nebpf_helper_version=\ndefault_route_device=\n";
        for suffix in ["os=Linux\n", "unknown=value\n", "malformed\n"] {
            let error = parse_remote_preflight(&format!("{base}{suffix}")).unwrap_err();
            assert!(!error.to_string().is_empty(), "{suffix}");
        }
        let error =
            parse_remote_preflight(&base.replace("sudo_available=false", "sudo_available=maybe"))
                .unwrap_err();
        assert!(error.to_string().contains("must be true or false"));
        let error = parse_remote_preflight(
            &base.replace("ebpf_helper_state=missing", "ebpf_helper_state=ready"),
        )
        .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("availability and state disagree")
        );
        let error = parse_remote_preflight(
            &base.replace("ebpf_helper_state=missing", "ebpf_helper_state=unknown"),
        )
        .unwrap_err();
        assert!(error.to_string().contains("state is invalid"));
    }

    #[test]
    fn parse_remote_preflight_rejects_malformed_host_fingerprints() {
        for fingerprint in [
            "sha256:abcd",
            "sha256:gggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggg",
        ] {
            let manifest = format!(
                "os=Linux\narch=x86_64\nkernel=6.8.0\nvirtualization=none\nhost_fingerprint={fingerprint}\nhome_dir=/home/test\nsudo_available=true\nebpf_helper_available=true\nebpf_helper_state=ready\nebpf_helper_version=1.4.6\ndefault_route_device=eth0\ncommands=bash curl cargo rustc python3 rpmbuild\n"
            );

            let error = parse_remote_preflight(&manifest).unwrap_err();
            assert!(error.to_string().contains("host fingerprint is invalid"));
        }
    }

    #[test]
    fn parse_remote_artifact_manifest_requires_both_package_formats() {
        let manifest = parse_remote_artifact_manifest(
            "deb=target/packages/gewyvern_1.4.6-1_amd64.deb\nrpm=target/packages/rpm/gewyvern-1.4.6-1.x86_64.rpm\n",
        )
        .unwrap();

        assert!(manifest.deb.ends_with("_amd64.deb"));
        assert!(manifest.rpm.ends_with(".x86_64.rpm"));
    }

    #[test]
    fn parse_remote_artifact_manifest_rejects_ambiguous_or_malformed_entries() {
        for body in [
            "deb=one.deb\ndeb=two.deb\nrpm=one.rpm\n",
            "deb=one.deb\nrpm=one.rpm\nrpm=two.rpm\n",
            "deb=one.deb\nrpm=\n",
            "deb=one.deb\nrpm=one.rpm\nunknown=value\n",
            "deb=one.deb\nrpm=one.rpm\nmalformed\n",
        ] {
            assert!(parse_remote_artifact_manifest(body).is_err(), "{body}");
        }
    }

    #[test]
    fn remote_package_manifest_helper_confines_selected_artifacts() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("gewyvern-package-manifest-{unique}"));
        let packages = root.join("target/packages");
        fs::create_dir_all(&packages).unwrap();
        let deb = packages.join("gewyvern.deb");
        fs::write(&deb, b"deb").unwrap();
        let manifest = packages.join("build-manifest.txt");
        fs::write(&manifest, format!("deb={}\n", deb.display())).unwrap();

        let script = format!(
            "set -euo pipefail\nMANIFEST=target/packages/build-manifest.txt\n{REMOTE_PACKAGE_MANIFEST_HELPER}\npackage_from_manifest deb deb\n"
        );
        let valid = Command::new("bash")
            .arg("-c")
            .arg(&script)
            .current_dir(&root)
            .output()
            .unwrap();
        assert!(valid.status.success(), "{:?}", valid.stderr);
        assert_eq!(
            String::from_utf8(valid.stdout).unwrap().trim(),
            fs::canonicalize(&deb).unwrap().to_string_lossy()
        );

        let outside = root.join("outside.deb");
        fs::write(&outside, b"outside").unwrap();
        fs::write(&manifest, format!("deb={}\n", outside.display())).unwrap();
        let escaped = Command::new("bash")
            .arg("-c")
            .arg(&script)
            .current_dir(&root)
            .output()
            .unwrap();
        assert!(!escaped.status.success());
        assert!(
            String::from_utf8(escaped.stderr)
                .unwrap()
                .contains("escapes package root")
        );

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn generated_remote_package_scripts_are_valid_bash() {
        for script in [
            remote_package_smoke_script("v1.4.6"),
            remote_runtime_smoke_script(),
            REMOTE_LESERPENT_CONTROL_PLANE_AOT_SCRIPT.to_string(),
            REMOTE_LESERPENT_LANGUAGE_PACK_LOCAL_ORCHESTRA_AOT_SCRIPT.to_string(),
        ] {
            let output = Command::new("bash")
                .arg("-n")
                .arg("-c")
                .arg(script)
                .output()
                .unwrap();
            assert!(output.status.success(), "{:?}", output.stderr);
        }
    }

    #[test]
    fn control_plane_aot_restore_and_publish_use_the_same_runtime_graph() {
        let (restore, publish) = REMOTE_LESERPENT_CONTROL_PLANE_AOT_SCRIPT
            .split_once("dotnet publish")
            .unwrap();
        assert!(restore.contains("EVIDENCE=\"$(pwd)/target/packages/"));
        assert!(restore.contains("dotnet restore"));
        assert!(restore.contains("--locked-mode"));
        assert!(!restore.contains("-r linux-x64"));
        assert!(restore.contains("-p:PublishAot=true"));
        assert!(restore.contains("-p:RuntimeIdentifier=linux-x64"));
        assert!(!publish.contains("-r linux-x64"));
        assert!(publish.contains("-p:PublishAot=true"));
        assert!(publish.contains("-p:RuntimeIdentifier=linux-x64"));
        assert!(publish.contains("--no-restore"));
        assert!(publish.contains("'\"outcome\":\"degraded\"'"));
    }

    #[test]
    fn local_orchestra_language_pack_aot_uses_locked_matching_runtime_graphs() {
        let (restore, publish) =
            REMOTE_LESERPENT_LANGUAGE_PACK_LOCAL_ORCHESTRA_AOT_SCRIPT
                .split_once("dotnet publish")
                .unwrap();
        assert!(restore.contains("cargo build --locked --quiet --release -p leserpentd"));
        assert!(restore.contains("--features leserpentd/native-ssh"));
        assert!(restore.contains("dotnet restore"));
        assert!(restore.contains("--locked-mode"));
        assert!(restore.contains("-p:PublishAot=true"));
        assert!(restore.contains("-p:RuntimeIdentifier=linux-x64"));
        assert!(publish.contains("-p:PublishAot=true"));
        assert!(publish.contains("-p:RuntimeIdentifier=linux-x64"));
        assert!(publish.contains("--no-restore"));
        assert!(publish.contains("--verify-local-orchestra"));
        assert!(publish.contains("language-pack-assets.sha256"));
    }

    #[test]
    fn leserpent_control_plane_aot_evidence_is_strict_and_non_vacuous() {
        let root = remote_test_root("leserpent-control-plane-aot-evidence");
        write_valid_leserpent_control_plane_aot_evidence(&root);
        assert!(validate_leserpent_control_plane_aot_evidence(&root).is_ok());

        fs::write(root.join("unexpected.txt"), "stale").unwrap();
        assert!(validate_leserpent_control_plane_aot_evidence(&root).is_err());
        fs::remove_file(root.join("unexpected.txt")).unwrap();

        fs::write(
            root.join("orchestra.db"),
            b"SQLite format 3\0native-aot-proof-secret",
        )
        .unwrap();
        assert!(validate_leserpent_control_plane_aot_evidence(&root).is_err());
        fs::write(root.join("orchestra.db"), b"SQLite format 3\0proof").unwrap();

        fs::write(
            root.join("recovery.json"),
            r#"{"runtimeId":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","kind":"all","outcome":"ok","steps":[]}"#,
        )
        .unwrap();
        assert!(validate_leserpent_control_plane_aot_evidence(&root).is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn leserpent_control_plane_aot_evidence_rejects_symlink_entries() {
        use std::os::unix::fs::symlink;

        let root = remote_test_root("leserpent-control-plane-aot-symlink");
        write_valid_leserpent_control_plane_aot_evidence(&root);
        fs::remove_file(root.join("service.log")).unwrap();
        symlink(root.join("publish.log"), root.join("service.log")).unwrap();
        assert!(validate_leserpent_control_plane_aot_evidence(&root).is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn local_orchestra_language_pack_aot_evidence_is_strict_and_non_vacuous() {
        let root = remote_test_root("leserpent-language-pack-local-orchestra-aot-evidence");
        write_valid_leserpent_language_pack_local_orchestra_aot_evidence(&root);
        assert!(validate_leserpent_language_pack_local_orchestra_aot_evidence(&root).is_ok());

        fs::write(root.join("unexpected.txt"), "stale").unwrap();
        assert!(validate_leserpent_language_pack_local_orchestra_aot_evidence(&root).is_err());
        fs::remove_file(root.join("unexpected.txt")).unwrap();

        fs::write(
            root.join("language-pack-assets.sha256"),
            format!(
                "{}  catalog.json\n{}  pt-BR.json\n",
                "0".repeat(64),
                "1".repeat(64)
            ),
        )
        .unwrap();
        assert!(validate_leserpent_language_pack_local_orchestra_aot_evidence(&root).is_err());

        write_valid_leserpent_language_pack_local_orchestra_aot_evidence(&root);
        fs::write(
            root.join("verification.log"),
            "local orchestra valid: rust_daemon=true\n",
        )
        .unwrap();
        assert!(validate_leserpent_language_pack_local_orchestra_aot_evidence(&root).is_err());

        write_valid_leserpent_language_pack_local_orchestra_aot_evidence(&root);
        fs::write(
            root.join("daemon-build.log"),
            "Authorization: Bearer forbidden\n",
        )
        .unwrap();
        assert!(validate_leserpent_language_pack_local_orchestra_aot_evidence(&root).is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn local_orchestra_language_pack_aot_evidence_rejects_symlink_entries() {
        use std::os::unix::fs::symlink;

        let root = remote_test_root("leserpent-language-pack-local-orchestra-aot-symlink");
        write_valid_leserpent_language_pack_local_orchestra_aot_evidence(&root);
        fs::remove_file(root.join("verification.log")).unwrap();
        symlink(
            root.join("environment.txt"),
            root.join("verification.log"),
        )
        .unwrap();
        assert!(validate_leserpent_language_pack_local_orchestra_aot_evidence(&root).is_err());
        fs::remove_dir_all(root).unwrap();
    }

    fn remote_test_root(name: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("gewyvern-{name}-{unique}"))
    }

    fn write_valid_leserpent_control_plane_aot_evidence(root: &Path) {
        const FILES: [&str; 12] = [
            "environment.txt",
            "restore.log",
            "publish.log",
            "payload.sha256",
            "service.log",
            "health.json",
            "registration-plan.json",
            "registration.json",
            "recovery.json",
            "attention.json",
            "runtime-state.json",
            "orchestra.db",
        ];
        fs::create_dir_all(root).unwrap();
        fs::write(
            root.join("environment.txt"),
            "os=linux\narch=x86_64\nrid=linux-x64\n",
        )
        .unwrap();
        for log in ["restore.log", "publish.log", "service.log"] {
            fs::write(root.join(log), "ok\n").unwrap();
        }
        let hash = "a".repeat(64);
        fs::write(
            root.join("payload.sha256"),
            [
                "Leserpent",
                "leserpent-compat-bridge",
                "leserpentd",
                "libe_sqlite3.so",
            ]
            .map(|name| format!("{hash}  proof/publish/{name}"))
            .join("\n")
                + "\n",
        )
        .unwrap();
        fs::write(
            root.join("health.json"),
            r#"{"ok":true,"runtimePosture":{"coreReady":true}}"#,
        )
        .unwrap();
        fs::write(
            root.join("registration-plan.json"),
            format!(
                r#"{{"allowed":true,"action":"create","planToken":"{}"}}"#,
                "b".repeat(64)
            ),
        )
        .unwrap();
        let runtime_id = "c".repeat(32);
        fs::write(
            root.join("registration.json"),
            format!(r#"{{"runtimeId":"{runtime_id}"}}"#),
        )
        .unwrap();
        fs::write(
            root.join("recovery.json"),
            format!(
                r#"{{"runtimeId":"{runtime_id}","kind":"all","outcome":"degraded","steps":[{{"kind":"capabilities","outcome":"degraded"}},{{"kind":"status","outcome":"degraded"}}]}}"#
            ),
        )
        .unwrap();
        fs::write(
            root.join("attention.json"),
            format!(
                r#"{{"runtimeId":"{runtime_id}","suggestedActions":[{{"action":"refresh_all","commandKind":"all"}}]}}"#
            ),
        )
        .unwrap();
        fs::write(root.join("runtime-state.json"), r#"{"schemaVersion":1}"#).unwrap();
        fs::write(root.join("orchestra.db"), b"SQLite format 3\0proof").unwrap();
        fs::write(
            root.join("evidence-index.json"),
            serde_json::to_vec(&serde_json::json!({
                "schema_version": 1,
                "proof": "leserpent-control-plane-native-aot-linux-x64",
                "result": "passed",
                "files": FILES,
            }))
            .unwrap(),
        )
        .unwrap();
    }

    fn write_valid_leserpent_language_pack_local_orchestra_aot_evidence(root: &Path) {
        const FILES: [&str; 7] = [
            "environment.txt",
            "restore.log",
            "publish.log",
            "daemon-build.log",
            "payload.sha256",
            "language-pack-assets.sha256",
            "verification.log",
        ];
        fs::create_dir_all(root).unwrap();
        fs::write(
            root.join("environment.txt"),
            "os=Linux\narch=x86_64\nrid=linux-x64\nkernel=7.0.0-test\ndotnet_sdk=10.0.111\nrustc=rustc 1.95.0\ncargo=cargo 1.95.0\navalonia_bytes=26000000\nleserpentd_bytes=12000000\n",
        )
        .unwrap();
        for log in ["restore.log", "publish.log", "daemon-build.log"] {
            fs::write(root.join(log), "ok\n").unwrap();
        }
        let payload_hash = "a".repeat(64);
        fs::write(
            root.join("payload.sha256"),
            format!(
                "{payload_hash}  Leserpent.Avalonia\n{payload_hash}  leserpentd\n"
            ),
        )
        .unwrap();
        let asset_root =
            super::repo_root().join("apps/leserpent/src/Leserpent/wwwroot/language-packs");
        let catalog_hash = super::evidence_file_sha256(&asset_root.join("catalog.json")).unwrap();
        let pack_hash = super::evidence_file_sha256(&asset_root.join("pt-BR.json")).unwrap();
        fs::write(
            root.join("language-pack-assets.sha256"),
            format!("{catalog_hash}  catalog.json\n{pack_hash}  pt-BR.json\n"),
        )
        .unwrap();
        fs::write(
            root.join("verification.log"),
            format!(
                "{}{}\n",
                super::LOCAL_ORCHESTRA_VERIFICATION_PREFIX,
                super::LOCAL_ORCHESTRA_VERIFICATION_CHECKS.join(", ")
            ),
        )
        .unwrap();
        fs::write(
            root.join("evidence-index.json"),
            serde_json::to_vec(&serde_json::json!({
                "schema_version": 1,
                "proof": "leserpent-language-pack-local-orchestra-native-aot-linux-x64",
                "result": "passed",
                "files": FILES,
            }))
            .unwrap(),
        )
        .unwrap();
    }

    #[test]
    fn remote_sync_includes_root_and_nested_test_shelves() {
        assert!(is_relevant_workspace_path("tests/linux_smoke_tdd.rs"));
        assert!(is_relevant_workspace_path("apps/etragon/src/tests/mod.rs"));
        assert!(is_relevant_workspace_path(
            "crates/leserpent-cli/tests/compat.rs"
        ));
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
    fn parse_remote_ebpf_evidence_rejects_ambiguous_or_unknown_entries() {
        for body in [
            "status=ok\nstatus=skipped\nreason=test\ndefault_route_device=\n",
            "status=ok\nreason=test\ndefault_route_device=\nunknown=value\n",
            "status=ok\nreason=\ndefault_route_device=\n",
        ] {
            assert!(parse_remote_ebpf_evidence(body).is_err(), "{body}");
        }
    }

    #[test]
    fn parse_remote_phase_timings_requires_finite_unique_known_phases() {
        let timings = parse_remote_phase_timings(
            "build=1.250\ntotal=2.500\n",
            "test timings",
            &["build", "total", "optional"],
            &["build", "total"],
        )
        .unwrap();
        assert_eq!(timings.render(), "build=1.250\ntotal=2.500\n");

        for body in [
            "build=1.0\n",
            "build=1.0\ntotal=NaN\n",
            "build=-1.0\ntotal=2.0\n",
            "build=1.0\ntotal=2.0\ntotal=3.0\n",
            "build=1.0\ntotal=2.0\nunknown=3.0\n",
        ] {
            assert!(
                parse_remote_phase_timings(
                    body,
                    "test timings",
                    &["build", "total", "optional"],
                    &["build", "total"],
                )
                .is_err(),
                "{body}"
            );
        }
    }

    #[test]
    fn resolve_remote_workspace_path_keeps_default_rooted_workspace() {
        let resolved = resolve_remote_workspace_path(
            "~/.gewyvern-remote-runs/gewyvern-remote-123",
            "/home/gewyvern-lab",
        )
        .unwrap();
        assert_eq!(
            resolved,
            "/home/gewyvern-lab/.gewyvern-remote-runs/gewyvern-remote-123"
        );
    }

    #[test]
    fn resolve_remote_workspace_path_rejects_escape_outside_allowed_root() {
        let err =
            resolve_remote_workspace_path("/home/gewyvern-lab/../../etc", "/home/gewyvern-lab")
                .unwrap_err();
        assert!(err.to_string().contains("must stay under"));
    }

    #[test]
    fn resolve_remote_execution_path_allows_internal_cache_root() {
        let resolved = resolve_remote_execution_path(
            "/home/gewyvern-lab/.cache/gewyvern/remote-source",
            "/home/gewyvern-lab",
        )
        .unwrap();
        assert_eq!(resolved, "/home/gewyvern-lab/.cache/gewyvern/remote-source");
    }
}
