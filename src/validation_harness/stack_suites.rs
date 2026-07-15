use std::env;
use std::fs::{self, File};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde_json::json;

use super::command::{ValidationError, ValidationReport, default_out_dir, repo_root};
use super::{
    run_linux_attach_smoke, run_linux_kprobe_smoke, run_linux_tc_smoke,
    run_stack_json_file_validation, run_stack_probe_validation, run_stack_register_runtime_json,
    write_stack_resilience_summary,
};

pub fn run_three_module_stack_smoke() -> Result<ValidationReport, ValidationError> {
    let cfg = ThreeModuleStackConfig::from_env()?;
    require_cmd("docker")?;
    require_cmd("curl")?;
    require_dotnet()?;
    ensure_docker_reachable()?;
    ensure_dir_exists(&cfg.etragon_root, "missing etragon app")?;
    ensure_dir_exists(&cfg.leserpent_root, "missing leserpent app")?;

    let work_dir = make_temp_dir("three-module-stack")?;
    let target_cache_dir = work_dir.join("target-cache");
    fs::create_dir_all(&target_cache_dir)?;
    fs::create_dir_all(&cfg.cargo_cache_dir)?;
    let state_path = work_dir.join("leserpent-state.json");
    let lesp_log = work_dir.join("leserpent.log");

    let mut cleanup = StackCleanup::new(
        vec![
            cfg.gw_a_name.clone(),
            cfg.gw_b_name.clone(),
            cfg.et_a_name.clone(),
        ],
        Some(cfg.network_name.clone()),
    );

    build_stack_image(&cfg)?;
    ensure_docker_network(&cfg.network_name)?;
    build_stack_binaries(&cfg, &target_cache_dir)?;
    start_gewyvern_container(
        &cfg,
        &target_cache_dir,
        &cfg.gw_a_name,
        cfg.gw_a_socket_port,
        cfg.gw_a_api_port,
    )?;
    start_gewyvern_container(
        &cfg,
        &target_cache_dir,
        &cfg.gw_b_name,
        cfg.gw_b_socket_port,
        cfg.gw_b_api_port,
    )?;

    let gw_a_health = work_dir.join("gw-a-health.json");
    stack_probe_or_logs(
        "http-ready",
        &format!("http://127.0.0.1:{}/health", cfg.gw_a_api_port),
        None,
        &gw_a_health,
        &cfg.gw_a_name,
        "gw-a did not become healthy",
    )?;
    let gw_b_health = work_dir.join("gw-b-health.json");
    stack_probe_or_logs(
        "http-ready",
        &format!("http://127.0.0.1:{}/health", cfg.gw_b_api_port),
        None,
        &gw_b_health,
        &cfg.gw_b_name,
        "gw-b did not become healthy",
    )?;

    ingest_template(&cfg.gw_a_name, "udp")?;
    ingest_template(&cfg.gw_b_name, "udp")?;

    let gw_a_resilience = work_dir.join("gw-a-resilience.json");
    probe_with_logs(
        "resilience-healthy",
        &format!(
            "http://127.0.0.1:{}/v1/runtime/resilience.json",
            cfg.gw_a_api_port
        ),
        None,
        &gw_a_resilience,
        &cfg.gw_a_name,
        "gw-a never published a healthy resilience surface",
    )?;
    let gw_b_resilience = work_dir.join("gw-b-resilience.json");
    probe_with_logs(
        "resilience-healthy",
        &format!(
            "http://127.0.0.1:{}/v1/runtime/resilience.json",
            cfg.gw_b_api_port
        ),
        None,
        &gw_b_resilience,
        &cfg.gw_b_name,
        "gw-b never published a healthy resilience surface",
    )?;

    inject_socket_bad_json(&cfg.gw_b_name, 5)?;

    let gw_b_health_degraded = work_dir.join("gw-b-health-degraded.json");
    probe_with_logs(
        "health-degraded",
        &format!("http://127.0.0.1:{}/health", cfg.gw_b_api_port),
        None,
        &gw_b_health_degraded,
        &cfg.gw_b_name,
        "gw-b never exposed resilience_degraded=true after repeated socket failures",
    )?;
    let gw_b_resilience_degraded = work_dir.join("gw-b-resilience-degraded.json");
    probe_with_logs(
        "resilience-degraded",
        &format!(
            "http://127.0.0.1:{}/v1/runtime/resilience.json",
            cfg.gw_b_api_port
        ),
        None,
        &gw_b_resilience_degraded,
        &cfg.gw_b_name,
        "gw-b never published a degraded resilience surface after repeated socket failures",
    )?;

    let gw_a_meta = work_dir.join("gw-a-meta.json");
    probe_with_logs(
        "meta-has-analysis",
        &format!("http://127.0.0.1:{}/v1/latest/meta", cfg.gw_a_api_port),
        None,
        &gw_a_meta,
        &cfg.gw_a_name,
        "gw-a never published analysis_json",
    )?;
    let gw_b_meta = work_dir.join("gw-b-meta.json");
    probe_with_logs(
        "meta-has-analysis",
        &format!("http://127.0.0.1:{}/v1/latest/meta", cfg.gw_b_api_port),
        None,
        &gw_b_meta,
        &cfg.gw_b_name,
        "gw-b never published analysis_json",
    )?;

    start_etragon_container(&cfg, &target_cache_dir)?;
    let etragon_health = work_dir.join("etragon-health.json");
    stack_probe_or_logs(
        "http-ready",
        &format!("http://127.0.0.1:{}/health", cfg.et_a_api_port),
        Some(cfg.et_a_admin_token.as_str()),
        &etragon_health,
        &cfg.et_a_name,
        "etragon sidecar did not become healthy",
    )?;

    let etragon_status = work_dir.join("etragon-status.json");
    probe_with_logs(
        "etragon-status",
        &format!("http://127.0.0.1:{}/v1/latest/status", cfg.et_a_api_port),
        Some(cfg.et_a_admin_token.as_str()),
        &etragon_status,
        &cfg.et_a_name,
        "etragon sidecar never reached ready/degraded daemon status",
    )?;
    println!("etragon-status-ok");

    let etragon_output = work_dir.join("etragon-output.json");
    probe_with_logs(
        "etragon-output",
        &format!(
            "http://127.0.0.1:{}/v1/latest/output.json",
            cfg.et_a_api_port
        ),
        Some(cfg.et_a_admin_token.as_str()),
        &etragon_output,
        &cfg.et_a_name,
        "etragon sidecar never published output_json with augmentations",
    )?;
    println!("etragon-output-ok");

    if cfg.leserpent_dotnet_restore_first {
        run_dotnet_restore(&cfg)?;
    }

    let leserpent = start_leserpent(&cfg, &state_path, &lesp_log)?;
    cleanup.leserpent = Some(leserpent);
    wait_http_status_200(
        &format!("http://127.0.0.1:{}/health", cfg.lesp_port),
        Duration::from_secs(60),
    )
    .map_err(|_| {
        let log = fs::read_to_string(&lesp_log).unwrap_or_default();
        ValidationError::new(format!("leserpent did not become healthy\n{log}"))
    })?;

    post_json_status(
        &format!("http://127.0.0.1:{}/v1/runtimes/register", cfg.lesp_port),
        &run_stack_register_runtime_json(
            "gw-stack-a",
            &format!("http://127.0.0.1:{}", cfg.gw_a_api_port),
            "stack",
            "local",
            "with-sidecar",
            Some(&format!("http://127.0.0.1:{}", cfg.et_a_api_port)),
            Some(&cfg.et_a_admin_token),
        )?,
        &[("X-Leserpent-Intent", "mutate")],
    )?;
    post_json_status(
        &format!("http://127.0.0.1:{}/v1/runtimes/register", cfg.lesp_port),
        &run_stack_register_runtime_json(
            "gw-stack-b",
            &format!("http://127.0.0.1:{}", cfg.gw_b_api_port),
            "stack",
            "local",
            "plain",
            None,
            None,
        )?,
        &[("X-Leserpent-Intent", "mutate")],
    )?;
    post_empty_status(
        &format!(
            "http://127.0.0.1:{}/v1/fleet/refresh-all?environment=stack",
            cfg.lesp_port
        ),
        &[("X-Leserpent-Intent", "mutate")],
    )?;

    let runtimes_json = curl_get_http_body(&format!(
        "http://127.0.0.1:{}/v1/runtimes?environment=stack",
        cfg.lesp_port
    ))?;
    let runtimes_path = work_dir.join("leserpent-runtimes.json");
    fs::write(&runtimes_path, runtimes_json)?;
    run_stack_json_file_validation(&runtimes_path, "leserpent-runtimes-sidecar").map_err(|_| {
        let log = fs::read_to_string(&lesp_log).unwrap_or_default();
        ValidationError::new(format!(
            "leserpent never observed a healthy sidecar for gw-stack-a\n{log}"
        ))
    })?;

    let summary_json = curl_get_http_body(&format!(
        "http://127.0.0.1:{}/v1/fleet/summary?environment=stack",
        cfg.lesp_port
    ))?;
    let summary_path = work_dir.join("leserpent-summary.json");
    fs::write(&summary_path, summary_json)?;
    run_stack_json_file_validation(&summary_path, "leserpent-summary").map_err(|_| {
        let log = fs::read_to_string(&lesp_log).unwrap_or_default();
        ValidationError::new(format!(
            "leserpent never published the expected fleet summary\n{log}"
        ))
    })?;
    println!("summary-ok");

    run_stack_json_file_validation(&runtimes_path, "leserpent-runtime-detail")?;
    println!("runtimes-ok");
    println!("gw-a-resilience-ok");
    println!("gw-b-resilience-ok");
    println!("gw-b-health-degraded-ok");
    println!("gw-b-resilience-degraded-ok");

    write_stack_resilience_summary(
        &gw_a_resilience,
        &gw_b_resilience,
        &gw_b_resilience_degraded,
        &cfg.resilience_summary_path,
    )?;

    println!("three-module stack smoke: ok");
    println!("leserpent=http://127.0.0.1:{}", cfg.lesp_port);
    println!("gewyvern_a=http://127.0.0.1:{}", cfg.gw_a_api_port);
    println!("gewyvern_b=http://127.0.0.1:{}", cfg.gw_b_api_port);
    println!("etragon_a=http://127.0.0.1:{}", cfg.et_a_api_port);
    println!(
        "resilience_summary={}",
        cfg.resilience_summary_path.display()
    );

    Ok(ValidationReport {
        name: "three module stack smoke".to_string(),
        out_dir: work_dir,
        checks: vec![
            "gw_a_healthy".to_string(),
            "gw_b_healthy".to_string(),
            "etragon_ready".to_string(),
            "leserpent_summary_ok".to_string(),
            "resilience_summary_written".to_string(),
        ],
    })
}

pub fn run_pathological_container_validation(
    out_dir: Option<PathBuf>,
) -> Result<ValidationReport, ValidationError> {
    let cfg = PathologyConfig::from_env(out_dir)?;
    require_cmd("docker")?;
    ensure_docker_reachable()?;
    if !cfg
        .pathology_fixture_dir
        .join("pathology_client.py")
        .is_file()
    {
        return Err(ValidationError::new(format!(
            "missing pathological container fixture source: {}",
            cfg.pathology_fixture_dir.display()
        )));
    }

    let work_dir = make_temp_dir("gewyvern-pathology")?;
    let target_cache_dir = work_dir.join("target-cache");
    fs::create_dir_all(&cfg.out_dir)?;
    fs::create_dir_all(&target_cache_dir)?;
    fs::create_dir_all(&cfg.cargo_cache_dir)?;

    let cleanup_names = vec![
        cfg.gw_name.clone(),
        format!("{}-truncated", cfg.patho_prefix),
        format!("{}-disconnect", cfg.patho_prefix),
        format!("{}-oversize", cfg.patho_prefix),
        format!("{}-slow-drip", cfg.patho_prefix),
    ];
    let _cleanup = StackCleanup::new(cleanup_names, Some(cfg.network_name.clone()));

    build_stack_image_from_pathology(&cfg)?;
    build_pathology_binaries(&cfg, &target_cache_dir)?;
    ensure_docker_network(&cfg.network_name)?;
    start_pathology_runtime(&cfg, &target_cache_dir)?;

    stack_probe_to_path(
        "http-ready",
        &format!("http://127.0.0.1:{}/health", cfg.api_port),
        None,
        &cfg.out_dir.join("health-ready.json"),
    )?;
    ingest_template(&cfg.gw_name, "udp")?;
    stack_probe_to_path(
        "resilience-healthy",
        &format!(
            "http://127.0.0.1:{}/v1/runtime/resilience.json",
            cfg.api_port
        ),
        None,
        &cfg.out_dir.join("resilience-healthy.json"),
    )?;

    run_pathology_container(&cfg, "truncated", "truncated-json", 4)?;
    run_pathology_container(&cfg, "disconnect", "empty-disconnect", 4)?;
    run_pathology_container(&cfg, "slow-drip", "slow-drip", 3)?;
    run_pathology_container(&cfg, "oversize", "oversize-line", 3)?;

    stack_probe_to_path(
        "health-degraded",
        &format!("http://127.0.0.1:{}/health", cfg.api_port),
        None,
        &cfg.out_dir.join("health-degraded.json"),
    )?;
    stack_probe_to_path(
        "resilience-degraded",
        &format!(
            "http://127.0.0.1:{}/v1/runtime/resilience.json",
            cfg.api_port
        ),
        None,
        &cfg.out_dir.join("resilience-degraded.json"),
    )?;

    ingest_template(&cfg.gw_name, "udp")?;
    stack_probe_to_path(
        "meta-has-analysis",
        &format!("http://127.0.0.1:{}/v1/latest/meta", cfg.api_port),
        None,
        &cfg.out_dir.join("meta-after-pathology.json"),
    )?;

    fs::write(
        cfg.out_dir.join("runtime.log"),
        read_container_file(&cfg.gw_name, "/tmp/pathology-runtime.log")
            .unwrap_or_else(|_| docker_logs(&cfg.gw_name).unwrap_or_default()),
    )?;
    let runtime_log = fs::read_to_string(cfg.out_dir.join("runtime.log"))?;
    if !runtime_log.contains("socket_session_run_failed") {
        return Err(ValidationError::new(
            "runtime log did not preserve expected socket resilience evidence",
        ));
    }
    if !runtime_log.contains("unexpected_token")
        && !runtime_log.contains("fact_line_exceeded_65536_bytes")
    {
        return Err(ValidationError::new(
            "runtime log did not preserve expected pathological input class evidence",
        ));
    }

    fs::write(
        cfg.out_dir.join("summary.txt"),
        format!(
            "pathological container validation: ok\nhost_api=http://127.0.0.1:{}\nhost_socket=127.0.0.1:{}\nchecked=healthy_baseline,truncated_json,empty_disconnect,slow_drip,oversize_line,degraded_health,degraded_resilience,post_fault_analysis,log_evidence\nevidence={}\n",
            cfg.api_port,
            cfg.socket_port,
            cfg.out_dir.display()
        ),
    )?;
    write_container_validation_evidence_index(
        &cfg.out_dir,
        "pathological-container-validation",
        &[
            "health-ready.json",
            "health-degraded.json",
            "resilience-degraded.json",
            "meta-after-pathology.json",
            "runtime.log",
            "summary.txt",
        ],
    )?;
    print!(
        "{}",
        fs::read_to_string(cfg.out_dir.join("summary.txt")).unwrap_or_default()
    );

    Ok(ValidationReport {
        name: "pathological container validation".to_string(),
        out_dir: cfg.out_dir,
        checks: vec![
            "healthy_baseline".to_string(),
            "pathology_faults_exercised".to_string(),
            "degraded_health".to_string(),
            "degraded_resilience".to_string(),
            "log_evidence".to_string(),
        ],
    })
}

pub fn run_juice_shop_container_validation(
    out_dir: Option<PathBuf>,
) -> Result<ValidationReport, ValidationError> {
    if !cfg!(target_os = "linux") {
        return Err(ValidationError::new(
            "juice-shop container validation requires a Linux host because it bundles same-host eBPF attach proof",
        ));
    }

    let cfg = JuiceShopValidationConfig::from_env(out_dir)?;
    require_cmd("docker")?;
    require_cmd("curl")?;
    ensure_docker_reachable()?;
    ensure_juice_shop_image(&cfg.image)?;

    fs::create_dir_all(&cfg.out_dir)?;
    let container_name = cfg.container_name.clone();
    let _cleanup = StackCleanup::new(vec![container_name.clone()], None);

    let _ = Command::new("docker")
        .args(["rm", "-f", &container_name])
        .output();
    run_command(
        Command::new("docker")
            .arg("run")
            .arg("-d")
            .arg("--name")
            .arg(&container_name)
            .arg("-p")
            .arg(format!("127.0.0.1:{}:3000", cfg.host_port))
            .arg(&cfg.image),
        "failed to start Juice Shop container",
    )?;

    wait_http_status_200(
        &format!("http://127.0.0.1:{}/", cfg.host_port),
        Duration::from_secs(90),
    )
    .map_err(|err| {
        let logs = docker_logs(&container_name).unwrap_or_default();
        ValidationError::new(format!("juice shop never became ready: {err}\n{logs}"))
    })?;

    let root_body = curl_get_http_body(&format!("http://127.0.0.1:{}/", cfg.host_port))?;
    fs::write(cfg.out_dir.join("root.html"), root_body)?;

    let file_guard_status = curl_capture_http_exchange(
        &format!("http://127.0.0.1:{}/ftp/acquisitions.md.bak", cfg.host_port),
        &cfg.out_dir.join("file-guard.headers"),
        &cfg.out_dir.join("file-guard.body"),
    )?;
    let sqli_status = curl_capture_http_exchange(
        &format!(
            "http://127.0.0.1:{}/rest/products/search?q=%27%20OR%201%3D1--",
            cfg.host_port
        ),
        &cfg.out_dir.join("sqli.headers"),
        &cfg.out_dir.join("sqli.body"),
    )?;

    let target_logs = docker_logs(&container_name)?;
    fs::write(cfg.out_dir.join("juice-shop.log"), &target_logs)?;
    let file_guard_body = fs::read_to_string(cfg.out_dir.join("file-guard.body"))?;
    let sqli_body = fs::read_to_string(cfg.out_dir.join("sqli.body"))?;

    if !target_logs.contains("Only .md and .pdf files are allowed!")
        && !file_guard_body.contains("Only .md and .pdf files are allowed!")
    {
        return Err(ValidationError::new(
            "juice shop evidence did not preserve the expected file-guard anomaly",
        ));
    }
    if !target_logs.contains("SQLITE_ERROR") && !sqli_body.contains("SQLITE_ERROR") {
        return Err(ValidationError::new(
            "juice shop evidence did not preserve the expected SQL anomaly",
        ));
    }

    let attach = run_linux_attach_smoke(
        "syscalls/sys_enter_nanosleep",
        Some(cfg.out_dir.join("linux-attach-smoke")),
    )?;
    let kprobe = run_linux_kprobe_smoke(
        "ip_route_output_flow",
        Some(cfg.out_dir.join("linux-kprobe-smoke")),
    )?;
    let netdev = detect_default_route_device()?;
    let tc = run_linux_tc_smoke(&netdev, Some(cfg.out_dir.join("linux-tc-smoke")))?;

    fs::write(
        cfg.out_dir.join("summary.txt"),
        format!(
            "juice shop container validation: ok\nhost_url=http://127.0.0.1:{}\nfile_guard_status={}\nsqli_status={}\ndefault_route_device={}\nchecked=juice_shop_ready,file_guard_evidence,sqli_evidence,linux_attach_smoke,linux_kprobe_smoke,linux_tc_smoke\nattach_evidence={}\nkprobe_evidence={}\ntc_evidence={}\nevidence={}\n",
            cfg.host_port,
            file_guard_status,
            sqli_status,
            netdev,
            attach.out_dir.display(),
            kprobe.out_dir.display(),
            tc.out_dir.display(),
            cfg.out_dir.display(),
        ),
    )?;
    write_container_validation_evidence_index(
        &cfg.out_dir,
        "juice-shop-container-validation",
        &[
            "root.html",
            "file-guard.headers",
            "file-guard.body",
            "sqli.headers",
            "sqli.body",
            "juice-shop.log",
            "summary.txt",
            "linux-attach-smoke/evidence-index.json",
            "linux-kprobe-smoke/evidence-index.json",
            "linux-tc-smoke/evidence-index.json",
        ],
    )?;
    print!(
        "{}",
        fs::read_to_string(cfg.out_dir.join("summary.txt")).unwrap_or_default()
    );

    Ok(ValidationReport {
        name: "juice shop container validation".to_string(),
        out_dir: cfg.out_dir,
        checks: vec![
            "juice_shop_ready".to_string(),
            "file_guard_evidence".to_string(),
            "sqli_evidence".to_string(),
            "linux_attach_smoke".to_string(),
            "linux_kprobe_smoke".to_string(),
            "linux_tc_smoke".to_string(),
        ],
    })
}

pub fn run_ftp_denied_container_validation(
    out_dir: Option<PathBuf>,
) -> Result<ValidationReport, ValidationError> {
    if !cfg!(target_os = "linux") {
        return Err(ValidationError::new(
            "ftp-denied container validation requires a Linux host because it bundles same-host eBPF attach proof",
        ));
    }

    let cfg = FtpDeniedValidationConfig::from_env(out_dir)?;
    require_cmd("docker")?;
    require_cmd("curl")?;
    ensure_docker_reachable()?;
    ensure_ftp_denied_image(&cfg.image)?;

    fs::create_dir_all(&cfg.out_dir)?;
    let container_name = cfg.container_name.clone();
    let _cleanup = StackCleanup::new(vec![container_name.clone()], None);

    let _ = Command::new("docker")
        .args(["rm", "-f", &container_name])
        .output();
    run_command(
        Command::new("docker")
            .arg("run")
            .arg("-d")
            .arg("--name")
            .arg(&container_name)
            .arg("-p")
            .arg(format!("127.0.0.1:{}:21", cfg.host_port))
            .arg("-e")
            .arg(format!("FTP_USER={}", cfg.username))
            .arg("-e")
            .arg(format!("FTP_PASS={}", cfg.password))
            .arg("-e")
            .arg("PASV_ADDRESS=127.0.0.1")
            .arg(&cfg.image),
        "failed to start FTP denied container",
    )?;

    wait_for_ftp_banner(
        &format!("127.0.0.1:{}", cfg.host_port),
        Duration::from_secs(45),
    )
    .map_err(|err| {
        let logs = docker_logs(&container_name).unwrap_or_default();
        ValidationError::new(format!("ftp target never became ready: {err}\n{logs}"))
    })?;

    let denied_exit = curl_capture_ftp_denied_exchange(
        &format!("ftp://127.0.0.1:{}/", cfg.host_port),
        &cfg.out_dir.join("ftp-denied.stderr"),
        &cfg.out_dir.join("ftp-denied.stdout"),
    )?;
    let target_logs = docker_logs(&container_name)?;
    fs::write(cfg.out_dir.join("ftp-target.log"), &target_logs)?;
    let vsftpd_log =
        read_container_file(&container_name, "/var/log/vsftpd.log").unwrap_or_default();
    fs::write(cfg.out_dir.join("ftp-target-vsftpd.log"), &vsftpd_log)?;
    let denied_stderr = fs::read_to_string(cfg.out_dir.join("ftp-denied.stderr"))?;

    if !denied_stderr.contains("530 Login incorrect.")
        && !denied_stderr.contains("Access denied: 530")
    {
        return Err(ValidationError::new(
            "ftp denied evidence did not preserve the expected 530 authentication rejection",
        ));
    }
    if !vsftpd_log.contains("FAIL LOGIN") && !vsftpd_log.contains("[bad]") {
        return Err(ValidationError::new(
            "ftp target-side evidence did not preserve the expected FAIL LOGIN record",
        ));
    }

    let attach = run_linux_attach_smoke(
        "syscalls/sys_enter_nanosleep",
        Some(cfg.out_dir.join("linux-attach-smoke")),
    )?;
    let kprobe = run_linux_kprobe_smoke(
        "ip_route_output_flow",
        Some(cfg.out_dir.join("linux-kprobe-smoke")),
    )?;
    let netdev = detect_default_route_device()?;
    let tc = run_linux_tc_smoke(&netdev, Some(cfg.out_dir.join("linux-tc-smoke")))?;

    fs::write(
        cfg.out_dir.join("summary.txt"),
        format!(
            "ftp denied container validation: ok\nhost_url=ftp://127.0.0.1:{}\ncurl_exit_status={}\ndefault_route_device={}\nchecked=ftp_ready,ftp_denied_evidence,ftp_target_log_evidence,linux_attach_smoke,linux_kprobe_smoke,linux_tc_smoke\nattach_evidence={}\nkprobe_evidence={}\ntc_evidence={}\nevidence={}\n",
            cfg.host_port,
            denied_exit,
            netdev,
            attach.out_dir.display(),
            kprobe.out_dir.display(),
            tc.out_dir.display(),
            cfg.out_dir.display(),
        ),
    )?;
    write_container_validation_evidence_index(
        &cfg.out_dir,
        "ftp-denied-container-validation",
        &[
            "ftp-denied.stderr",
            "ftp-denied.stdout",
            "ftp-target.log",
            "ftp-target-vsftpd.log",
            "summary.txt",
            "linux-attach-smoke/evidence-index.json",
            "linux-kprobe-smoke/evidence-index.json",
            "linux-tc-smoke/evidence-index.json",
        ],
    )?;
    print!(
        "{}",
        fs::read_to_string(cfg.out_dir.join("summary.txt")).unwrap_or_default()
    );

    Ok(ValidationReport {
        name: "ftp denied container validation".to_string(),
        out_dir: cfg.out_dir,
        checks: vec![
            "ftp_ready".to_string(),
            "ftp_denied_evidence".to_string(),
            "ftp_target_log_evidence".to_string(),
            "linux_attach_smoke".to_string(),
            "linux_kprobe_smoke".to_string(),
            "linux_tc_smoke".to_string(),
        ],
    })
}

pub fn run_ldap_bind_denied_container_validation(
    out_dir: Option<PathBuf>,
) -> Result<ValidationReport, ValidationError> {
    if !cfg!(target_os = "linux") {
        return Err(ValidationError::new(
            "ldap-bind-denied container validation requires a Linux host because it bundles same-host eBPF attach proof",
        ));
    }

    let cfg = LdapBindDeniedValidationConfig::from_env(out_dir)?;
    require_cmd("docker")?;
    require_cmd("ldapsearch")?;
    ensure_docker_reachable()?;
    ensure_ldap_bind_denied_image(&cfg.image)?;

    fs::create_dir_all(&cfg.out_dir)?;
    let container_name = cfg.container_name.clone();
    let _cleanup = StackCleanup::new(vec![container_name.clone()], None);

    let _ = Command::new("docker")
        .args(["rm", "-f", &container_name])
        .output();
    run_command(
        Command::new("docker")
            .arg("run")
            .arg("-d")
            .arg("--name")
            .arg(&container_name)
            .arg("-p")
            .arg(format!("127.0.0.1:{}:389", cfg.host_port))
            .arg("-e")
            .arg("LDAP_ORGANISATION=Example Inc")
            .arg("-e")
            .arg("LDAP_DOMAIN=example.org")
            .arg("-e")
            .arg("LDAP_LOG_LEVEL=256")
            .arg("-e")
            .arg(format!("LDAP_ADMIN_PASSWORD={}", cfg.admin_password))
            .arg(&cfg.image),
        "failed to start LDAP bind denied container",
    )?;

    wait_for_ldap_bind_ready(
        cfg.host_port,
        &cfg.admin_dn,
        &cfg.admin_password,
        &cfg.search_base,
        Duration::from_secs(60),
    )
    .map_err(|err| {
        let logs = docker_logs(&container_name).unwrap_or_default();
        ValidationError::new(format!("ldap target never became ready: {err}\n{logs}"))
    })?;

    let live_target_log_path = cfg.out_dir.join("ldap-target.live.log");
    let live_target_log = File::create(&live_target_log_path)?;
    let live_target_log_err = live_target_log.try_clone()?;
    let mut live_target_logs = Command::new("docker")
        .args(["logs", "-f", &container_name])
        .stdout(Stdio::from(live_target_log))
        .stderr(Stdio::from(live_target_log_err))
        .spawn()
        .map_err(|err| ValidationError::new(format!("failed to follow LDAP target logs: {err}")))?;
    thread::sleep(Duration::from_millis(500));

    let denied_exit = ldap_capture_bind_denied_exchange(
        cfg.host_port,
        &cfg.admin_dn,
        &cfg.search_base,
        &cfg.out_dir.join("ldap-bind-denied.stderr"),
        &cfg.out_dir.join("ldap-bind-denied.stdout"),
    )?;
    thread::sleep(Duration::from_secs(1));
    let _ = live_target_logs.kill();
    let _ = live_target_logs.wait();

    let mut target_logs = fs::read_to_string(&live_target_log_path).unwrap_or_default();
    let snapshot_logs = docker_logs(&container_name).unwrap_or_default();
    if target_logs.trim().is_empty() {
        target_logs = snapshot_logs;
    } else if !snapshot_logs.trim().is_empty() {
        if !target_logs.ends_with('\n') {
            target_logs.push('\n');
        }
        target_logs.push_str(&snapshot_logs);
    }
    fs::write(cfg.out_dir.join("ldap-target.log"), &target_logs)?;
    let denied_stderr = fs::read_to_string(cfg.out_dir.join("ldap-bind-denied.stderr"))?;

    if !denied_stderr.contains("Invalid credentials (49)") {
        return Err(ValidationError::new(
            "ldap denied evidence did not preserve the expected invalid-credentials rejection",
        ));
    }
    if !target_logs.contains("BIND dn=") || !target_logs.contains("err=49") {
        return Err(ValidationError::new(
            "ldap target-side evidence did not preserve the expected bind err=49 record",
        ));
    }

    let attach = run_linux_attach_smoke(
        "syscalls/sys_enter_nanosleep",
        Some(cfg.out_dir.join("linux-attach-smoke")),
    )?;
    let kprobe = run_linux_kprobe_smoke(
        "ip_route_output_flow",
        Some(cfg.out_dir.join("linux-kprobe-smoke")),
    )?;
    let netdev = detect_default_route_device()?;
    let tc = run_linux_tc_smoke(&netdev, Some(cfg.out_dir.join("linux-tc-smoke")))?;

    fs::write(
        cfg.out_dir.join("summary.txt"),
        format!(
            "ldap bind denied container validation: ok\nhost_url=ldap://127.0.0.1:{}\nldap_exit_status={}\ndefault_route_device={}\nchecked=ldap_ready,ldap_bind_denied_evidence,ldap_target_log_evidence,linux_attach_smoke,linux_kprobe_smoke,linux_tc_smoke\nattach_evidence={}\nkprobe_evidence={}\ntc_evidence={}\nevidence={}\n",
            cfg.host_port,
            denied_exit,
            netdev,
            attach.out_dir.display(),
            kprobe.out_dir.display(),
            tc.out_dir.display(),
            cfg.out_dir.display(),
        ),
    )?;
    write_container_validation_evidence_index(
        &cfg.out_dir,
        "ldap-bind-denied-container-validation",
        &[
            "ldap-bind-denied.stderr",
            "ldap-bind-denied.stdout",
            "ldap-target.live.log",
            "ldap-target.log",
            "summary.txt",
            "linux-attach-smoke/evidence-index.json",
            "linux-kprobe-smoke/evidence-index.json",
            "linux-tc-smoke/evidence-index.json",
        ],
    )?;
    print!(
        "{}",
        fs::read_to_string(cfg.out_dir.join("summary.txt")).unwrap_or_default()
    );

    Ok(ValidationReport {
        name: "ldap bind denied container validation".to_string(),
        out_dir: cfg.out_dir,
        checks: vec![
            "ldap_ready".to_string(),
            "ldap_bind_denied_evidence".to_string(),
            "ldap_target_log_evidence".to_string(),
            "linux_attach_smoke".to_string(),
            "linux_kprobe_smoke".to_string(),
            "linux_tc_smoke".to_string(),
        ],
    })
}

fn write_container_validation_evidence_index(
    out_dir: &Path,
    command: &str,
    files: &[&str],
) -> Result<(), ValidationError> {
    let index = json!({
        "schema_version": 1,
        "command": command,
        "files": files,
    });
    fs::write(
        out_dir.join("evidence-index.json"),
        serde_json::to_string_pretty(&index)?,
    )?;
    Ok(())
}

#[derive(Debug)]
struct ThreeModuleStackConfig {
    repo_root: PathBuf,
    etragon_root: PathBuf,
    leserpent_root: PathBuf,
    image_tag: String,
    skip_docker_build: bool,
    docker_base_image: String,
    docker_apt_mirror: String,
    docker_rustup_init_url: String,
    docker_rustup_init_fallback_url: String,
    docker_rustup_dist_server: String,
    docker_rustup_update_root: String,
    docker_rustup_install_timeout_seconds: String,
    network_name: String,
    gw_a_name: String,
    gw_b_name: String,
    et_a_name: String,
    lesp_port: u16,
    gw_a_socket_port: u16,
    gw_a_api_port: u16,
    gw_b_socket_port: u16,
    gw_b_api_port: u16,
    et_a_api_port: u16,
    et_a_admin_token: String,
    leserpent_dotnet_restore_first: bool,
    leserpent_dotnet_ignore_failed_sources: bool,
    leserpent_dotnet_no_restore: bool,
    cargo_cache_dir: PathBuf,
    cargo_net_offline: bool,
    resilience_summary_path: PathBuf,
}

impl ThreeModuleStackConfig {
    fn from_env() -> Result<Self, ValidationError> {
        let repo = repo_root();
        let work_dir = temp_dir_preview("three-module-stack");
        let unique = std::process::id();
        Ok(Self {
            etragon_root: repo.join("apps/etragon"),
            leserpent_root: repo.join("apps/leserpent"),
            repo_root: repo.clone(),
            image_tag: env_string("IMAGE_TAG", "gewyvern-stack-dev"),
            skip_docker_build: env_bool("SKIP_DOCKER_BUILD", false),
            docker_base_image: env_string("DOCKER_BASE_IMAGE", "ubuntu:24.04"),
            docker_apt_mirror: env_string("DOCKER_APT_MIRROR", ""),
            docker_rustup_init_url: env_string("DOCKER_RUSTUP_INIT_URL", "https://sh.rustup.rs"),
            docker_rustup_init_fallback_url: env_string(
                "DOCKER_RUSTUP_INIT_FALLBACK_URL",
                "https://sh.rustup.rs",
            ),
            docker_rustup_dist_server: env_string(
                "DOCKER_RUSTUP_DIST_SERVER",
                "https://static.rust-lang.org",
            ),
            docker_rustup_update_root: env_string(
                "DOCKER_RUSTUP_UPDATE_ROOT",
                "https://static.rust-lang.org/rustup",
            ),
            docker_rustup_install_timeout_seconds: env_string(
                "DOCKER_RUSTUP_INSTALL_TIMEOUT_SECONDS",
                "600",
            ),
            network_name: env_string("NETWORK_NAME", &format!("gewyvern-stack-net-{unique}")),
            gw_a_name: env_string("GW_A_NAME", &format!("gewyvern-stack-a-{unique}")),
            gw_b_name: env_string("GW_B_NAME", &format!("gewyvern-stack-b-{unique}")),
            et_a_name: env_string("ET_A_NAME", &format!("etragon-stack-a-{unique}")),
            lesp_port: env_u16("LESP_PORT", 5118)?,
            gw_a_socket_port: env_u16("GW_A_SOCKET_PORT", 19001)?,
            gw_a_api_port: env_u16("GW_A_API_PORT", 19101)?,
            gw_b_socket_port: env_u16("GW_B_SOCKET_PORT", 19002)?,
            gw_b_api_port: env_u16("GW_B_API_PORT", 19102)?,
            et_a_api_port: env_u16("ET_A_API_PORT", 19431)?,
            et_a_admin_token: env_string("ET_A_ADMIN_TOKEN", "stack-smoke-admin-token"),
            leserpent_dotnet_restore_first: env_bool("LESERPENT_DOTNET_RESTORE_FIRST", false),
            leserpent_dotnet_ignore_failed_sources: env_bool(
                "LESERPENT_DOTNET_IGNORE_FAILED_SOURCES",
                false,
            ),
            leserpent_dotnet_no_restore: env_bool("LESERPENT_DOTNET_NO_RESTORE", false),
            cargo_cache_dir: env_path(
                "CARGO_CACHE_DIR",
                &env::var("CARGO_HOME").unwrap_or_else(|_| {
                    env::var("HOME")
                        .map(|home| format!("{home}/.cargo"))
                        .unwrap_or_else(|_| format!("{}/.cargo", work_dir.display()))
                }),
            ),
            cargo_net_offline: env_bool("CARGO_NET_OFFLINE", false),
            resilience_summary_path: env_path(
                "RESILIENCE_SUMMARY_PATH",
                &work_dir
                    .join("resilience-summary.txt")
                    .display()
                    .to_string(),
            ),
        })
    }
}

#[derive(Debug)]
struct PathologyConfig {
    repo_root: PathBuf,
    image_tag: String,
    skip_docker_build: bool,
    docker_base_image: String,
    docker_apt_mirror: String,
    docker_rustup_init_url: String,
    docker_rustup_init_fallback_url: String,
    docker_rustup_dist_server: String,
    docker_rustup_update_root: String,
    docker_rustup_install_timeout_seconds: String,
    network_name: String,
    gw_name: String,
    patho_prefix: String,
    socket_port: u16,
    api_port: u16,
    out_dir: PathBuf,
    pathology_fixture_dir: PathBuf,
    cargo_cache_dir: PathBuf,
    cargo_net_offline: bool,
}

#[derive(Debug)]
struct JuiceShopValidationConfig {
    image: String,
    container_name: String,
    host_port: u16,
    out_dir: PathBuf,
}

struct FtpDeniedValidationConfig {
    image: String,
    container_name: String,
    host_port: u16,
    username: String,
    password: String,
    out_dir: PathBuf,
}

struct LdapBindDeniedValidationConfig {
    image: String,
    container_name: String,
    host_port: u16,
    admin_dn: String,
    admin_password: String,
    search_base: String,
    out_dir: PathBuf,
}

impl PathologyConfig {
    fn from_env(out_dir: Option<PathBuf>) -> Result<Self, ValidationError> {
        let repo = repo_root();
        let unique = std::process::id();
        Ok(Self {
            repo_root: repo.clone(),
            image_tag: env_string("IMAGE_TAG", "gewyvern-stack-dev"),
            skip_docker_build: env_bool("SKIP_DOCKER_BUILD", false),
            docker_base_image: env_string("DOCKER_BASE_IMAGE", "ubuntu:24.04"),
            docker_apt_mirror: env_string("DOCKER_APT_MIRROR", ""),
            docker_rustup_init_url: env_string("DOCKER_RUSTUP_INIT_URL", "https://sh.rustup.rs"),
            docker_rustup_init_fallback_url: env_string(
                "DOCKER_RUSTUP_INIT_FALLBACK_URL",
                "https://sh.rustup.rs",
            ),
            docker_rustup_dist_server: env_string(
                "DOCKER_RUSTUP_DIST_SERVER",
                "https://static.rust-lang.org",
            ),
            docker_rustup_update_root: env_string(
                "DOCKER_RUSTUP_UPDATE_ROOT",
                "https://static.rust-lang.org/rustup",
            ),
            docker_rustup_install_timeout_seconds: env_string(
                "DOCKER_RUSTUP_INSTALL_TIMEOUT_SECONDS",
                "600",
            ),
            network_name: env_string("NETWORK_NAME", &format!("gewyvern-pathology-net-{unique}")),
            gw_name: env_string("GW_NAME", &format!("gewyvern-pathology-runtime-{unique}")),
            patho_prefix: env_string("PATHO_PREFIX", &format!("gewyvern-pathology-{unique}")),
            socket_port: env_u16("SOCKET_PORT", 19201)?,
            api_port: env_u16("API_PORT", 19301)?,
            out_dir: out_dir.unwrap_or_else(|| default_out_dir("pathological-container")),
            pathology_fixture_dir: repo.join("tests/pathological-containers"),
            cargo_cache_dir: env_path(
                "CARGO_CACHE_DIR",
                &env::var("CARGO_HOME").unwrap_or_else(|_| {
                    env::var("HOME")
                        .map(|home| format!("{home}/.cargo"))
                        .unwrap_or_else(|_| String::from(".cargo"))
                }),
            ),
            cargo_net_offline: env_bool("CARGO_NET_OFFLINE", false),
        })
    }
}

impl JuiceShopValidationConfig {
    fn from_env(out_dir: Option<PathBuf>) -> Result<Self, ValidationError> {
        let unique = std::process::id();
        Ok(Self {
            image: env_string("JUICE_SHOP_IMAGE", "bkimminich/juice-shop:latest"),
            container_name: env_string("JUICE_SHOP_NAME", &format!("gewyvern-juice-shop-{unique}")),
            host_port: match env::var("JUICE_SHOP_PORT") {
                Ok(value) => value.parse::<u16>().map_err(|err| {
                    ValidationError::new(format!("invalid JUICE_SHOP_PORT value `{value}`: {err}"))
                })?,
                Err(_) => find_free_loopback_port()?,
            },
            out_dir: out_dir.unwrap_or_else(|| default_out_dir("juice-shop-container")),
        })
    }
}

impl FtpDeniedValidationConfig {
    fn from_env(out_dir: Option<PathBuf>) -> Result<Self, ValidationError> {
        let unique = std::process::id();
        Ok(Self {
            image: env_string("FTP_DENIED_IMAGE", "fauria/vsftpd:latest"),
            container_name: env_string("FTP_DENIED_NAME", &format!("gewyvern-ftp-denied-{unique}")),
            host_port: match env::var("FTP_DENIED_PORT") {
                Ok(value) => value.parse::<u16>().map_err(|err| {
                    ValidationError::new(format!("invalid FTP_DENIED_PORT value `{value}`: {err}"))
                })?,
                Err(_) => find_free_loopback_port()?,
            },
            username: env_string("FTP_DENIED_USER", "demo"),
            password: env_string("FTP_DENIED_PASS", "demo"),
            out_dir: out_dir.unwrap_or_else(|| default_out_dir("ftp-denied-container")),
        })
    }
}

impl LdapBindDeniedValidationConfig {
    fn from_env(out_dir: Option<PathBuf>) -> Result<Self, ValidationError> {
        let unique = std::process::id();
        Ok(Self {
            image: env_string("LDAP_BIND_DENIED_IMAGE", "osixia/openldap:1.5.0"),
            container_name: env_string(
                "LDAP_BIND_DENIED_NAME",
                &format!("gewyvern-ldap-bind-denied-{unique}"),
            ),
            host_port: match env::var("LDAP_BIND_DENIED_PORT") {
                Ok(value) => value.parse::<u16>().map_err(|err| {
                    ValidationError::new(format!(
                        "invalid LDAP_BIND_DENIED_PORT value `{value}`: {err}"
                    ))
                })?,
                Err(_) => find_free_loopback_port()?,
            },
            admin_dn: env_string("LDAP_BIND_DENIED_ADMIN_DN", "cn=admin,dc=example,dc=org"),
            admin_password: env_string("LDAP_BIND_DENIED_ADMIN_PASSWORD", "admin"),
            search_base: env_string("LDAP_BIND_DENIED_BASE", "dc=example,dc=org"),
            out_dir: out_dir.unwrap_or_else(|| default_out_dir("ldap-bind-denied-container")),
        })
    }
}

struct StackCleanup {
    container_names: Vec<String>,
    network_name: Option<String>,
    leserpent: Option<Child>,
}

impl StackCleanup {
    fn new(container_names: Vec<String>, network_name: Option<String>) -> Self {
        Self {
            container_names,
            network_name,
            leserpent: None,
        }
    }
}

impl Drop for StackCleanup {
    fn drop(&mut self) {
        if let Some(child) = self.leserpent.as_mut() {
            let _ = child.kill();
            let _ = child.wait();
        }
        for name in &self.container_names {
            let _ = Command::new("docker").args(["rm", "-f", name]).output();
        }
        if let Some(network_name) = &self.network_name {
            let _ = Command::new("docker")
                .args(["network", "rm", network_name])
                .output();
        }
    }
}

fn build_stack_image(cfg: &ThreeModuleStackConfig) -> Result<(), ValidationError> {
    if cfg.skip_docker_build {
        ensure_docker_image(&cfg.image_tag)?;
        return Ok(());
    }
    run_command(
        Command::new("docker")
            .arg("build")
            .arg("--build-arg")
            .arg(format!("BASE_IMAGE={}", cfg.docker_base_image))
            .arg("--build-arg")
            .arg(format!("APT_MIRROR={}", cfg.docker_apt_mirror))
            .arg("--build-arg")
            .arg(format!("RUSTUP_INIT_URL={}", cfg.docker_rustup_init_url))
            .arg("--build-arg")
            .arg(format!(
                "RUSTUP_INIT_FALLBACK_URL={}",
                cfg.docker_rustup_init_fallback_url
            ))
            .arg("--build-arg")
            .arg(format!(
                "RUSTUP_DIST_SERVER={}",
                cfg.docker_rustup_dist_server
            ))
            .arg("--build-arg")
            .arg(format!(
                "RUSTUP_UPDATE_ROOT={}",
                cfg.docker_rustup_update_root
            ))
            .arg("--build-arg")
            .arg(format!(
                "RUSTUP_INSTALL_TIMEOUT_SECONDS={}",
                cfg.docker_rustup_install_timeout_seconds
            ))
            .arg("-t")
            .arg(&cfg.image_tag)
            .arg("-f")
            .arg(cfg.repo_root.join("docker/linux-dev/Dockerfile"))
            .arg(&cfg.repo_root),
        "docker build failed",
    )
}

fn build_stack_image_from_pathology(cfg: &PathologyConfig) -> Result<(), ValidationError> {
    if cfg.skip_docker_build {
        ensure_docker_image(&cfg.image_tag)?;
        return Ok(());
    }
    run_command(
        Command::new("docker")
            .arg("build")
            .arg("--build-arg")
            .arg(format!("BASE_IMAGE={}", cfg.docker_base_image))
            .arg("--build-arg")
            .arg(format!("APT_MIRROR={}", cfg.docker_apt_mirror))
            .arg("--build-arg")
            .arg(format!("RUSTUP_INIT_URL={}", cfg.docker_rustup_init_url))
            .arg("--build-arg")
            .arg(format!(
                "RUSTUP_INIT_FALLBACK_URL={}",
                cfg.docker_rustup_init_fallback_url
            ))
            .arg("--build-arg")
            .arg(format!(
                "RUSTUP_DIST_SERVER={}",
                cfg.docker_rustup_dist_server
            ))
            .arg("--build-arg")
            .arg(format!(
                "RUSTUP_UPDATE_ROOT={}",
                cfg.docker_rustup_update_root
            ))
            .arg("--build-arg")
            .arg(format!(
                "RUSTUP_INSTALL_TIMEOUT_SECONDS={}",
                cfg.docker_rustup_install_timeout_seconds
            ))
            .arg("-t")
            .arg(&cfg.image_tag)
            .arg("-f")
            .arg(cfg.repo_root.join("docker/linux-dev/Dockerfile"))
            .arg(&cfg.repo_root),
        "docker build failed",
    )
}

fn build_stack_binaries(
    cfg: &ThreeModuleStackConfig,
    target_cache_dir: &Path,
) -> Result<(), ValidationError> {
    run_command(
        Command::new("docker")
            .arg("run")
            .arg("--rm")
            .arg("-v")
            .arg(format!(
                "{}:/workspace/dev/gewyvern",
                cfg.repo_root.display()
            ))
            .arg("-v")
            .arg(format!("{}:/stack-target", target_cache_dir.display()))
            .arg("-v")
            .arg(format!("{}:/cargo-cache", cfg.cargo_cache_dir.display()))
            .arg("-e")
            .arg("CARGO_HOME=/cargo-cache")
            .arg("-e")
            .arg(format!("CARGO_NET_OFFLINE={}", cfg.cargo_net_offline))
            .arg(&cfg.image_tag)
            .arg("bash")
            .arg("-lc")
            .arg(
                "set -euo pipefail\n\
                 export CARGO_TARGET_DIR=/stack-target/etragon\n\
                 cd /workspace/dev/gewyvern/apps/etragon\n\
                 cargo build --quiet\n\
                 export CARGO_TARGET_DIR=/stack-target/gewyvern\n\
                 cd /workspace/dev/gewyvern\n\
                 cargo build --quiet --bin gewyvern --bin gewyvern_socket_send\n",
            ),
        "stack binary build failed",
    )
}

fn build_pathology_binaries(
    cfg: &PathologyConfig,
    target_cache_dir: &Path,
) -> Result<(), ValidationError> {
    run_command(
        Command::new("docker")
            .arg("run")
            .arg("--rm")
            .arg("-v")
            .arg(format!(
                "{}:/workspace/dev/gewyvern",
                cfg.repo_root.display()
            ))
            .arg("-v")
            .arg(format!("{}:/stack-target", target_cache_dir.display()))
            .arg("-v")
            .arg(format!("{}:/cargo-cache", cfg.cargo_cache_dir.display()))
            .arg("-e")
            .arg("CARGO_HOME=/cargo-cache")
            .arg("-e")
            .arg(format!("CARGO_NET_OFFLINE={}", cfg.cargo_net_offline))
            .arg(&cfg.image_tag)
            .arg("bash")
            .arg("-lc")
            .arg(
                "set -euo pipefail\n\
                 export CARGO_TARGET_DIR=/stack-target/gewyvern\n\
                 cd /workspace/dev/gewyvern\n\
                 cargo build --quiet --bin gewyvern --bin gewyvern_socket_send\n",
            ),
        "pathology binary build failed",
    )
}

fn start_gewyvern_container(
    cfg: &ThreeModuleStackConfig,
    target_cache_dir: &Path,
    name: &str,
    socket_port: u16,
    api_port: u16,
) -> Result<(), ValidationError> {
    let _ = Command::new("docker").args(["rm", "-f", name]).output();
    run_command(
        Command::new("docker")
            .arg("run")
            .arg("-d")
            .arg("--name")
            .arg(name)
            .arg("--network")
            .arg(&cfg.network_name)
            .arg("-p")
            .arg(format!("127.0.0.1:{socket_port}:9000"))
            .arg("-p")
            .arg(format!("127.0.0.1:{api_port}:9100"))
            .arg("-v")
            .arg(format!(
                "{}:/workspace/dev/gewyvern",
                cfg.repo_root.display()
            ))
            .arg("-v")
            .arg(format!("{}:/stack-target", target_cache_dir.display()))
            .arg(&cfg.image_tag)
            .arg("bash")
            .arg("-lc")
            .arg(
                "/stack-target/gewyvern/debug/gewyvern \
                 --tcp-socket 0.0.0.0:9000 \
                 --template udp \
                 --ingest-mode remote-advisory \
                 --serve \
                 --allow-remote-api \
                 --api-socket 0.0.0.0:9100 \
                 --json \
                 --summary-only",
            ),
        "failed to start gewyvern container",
    )
}

fn start_etragon_container(
    cfg: &ThreeModuleStackConfig,
    target_cache_dir: &Path,
) -> Result<(), ValidationError> {
    let _ = Command::new("docker")
        .args(["rm", "-f", &cfg.et_a_name])
        .output();
    run_command(
        Command::new("docker")
            .arg("run")
            .arg("-d")
            .arg("--name")
            .arg(&cfg.et_a_name)
            .arg("--network")
            .arg(&cfg.network_name)
            .arg("-p")
            .arg(format!("127.0.0.1:{}:4321", cfg.et_a_api_port))
            .arg("-e")
            .arg(format!("ETRAGON_ADMIN_TOKEN={}", cfg.et_a_admin_token))
            .arg("-v")
            .arg(format!("{}:/workspace/dev/gewyvern", cfg.repo_root.display()))
            .arg("-v")
            .arg(format!("{}:/stack-target", target_cache_dir.display()))
            .arg(&cfg.image_tag)
            .arg("bash")
            .arg("-lc")
            .arg(format!(
                "/stack-target/etragon/debug/etragon \
                 serve-python-url \
                 http://{}:9100/v1/latest/analysis.json \
                 --bind 0.0.0.0:4321 \
                 --interval-ms 500 \
                 --python-worker /workspace/dev/gewyvern/apps/etragon/scripts/python_baseline_worker.py \
                 --python-state /tmp/etragon-online-state.json \
                 --daemon-state /tmp/etragon-daemon-state.json",
                cfg.gw_a_name
            )),
        "failed to start etragon container",
    )
}

fn start_pathology_runtime(
    cfg: &PathologyConfig,
    target_cache_dir: &Path,
) -> Result<(), ValidationError> {
    let _ = Command::new("docker")
        .args(["rm", "-f", &cfg.gw_name])
        .output();
    run_command(
        Command::new("docker")
            .arg("run")
            .arg("-d")
            .arg("--name")
            .arg(&cfg.gw_name)
            .arg("--network")
            .arg(&cfg.network_name)
            .arg("-p")
            .arg(format!("127.0.0.1:{}:9000", cfg.socket_port))
            .arg("-p")
            .arg(format!("127.0.0.1:{}:9100", cfg.api_port))
            .arg("-v")
            .arg(format!(
                "{}:/workspace/dev/gewyvern",
                cfg.repo_root.display()
            ))
            .arg("-v")
            .arg(format!("{}:/stack-target", target_cache_dir.display()))
            .arg(&cfg.image_tag)
            .arg("bash")
            .arg("-lc")
            .arg(
                "/stack-target/gewyvern/debug/gewyvern \
                 --tcp-socket 0.0.0.0:9000 \
                 --template udp \
                 --ingest-mode remote-advisory \
                 --serve \
                 --allow-remote-api \
                 --api-socket 0.0.0.0:9100 \
                 --log-level debug \
                 --log-file /tmp/pathology-runtime.log \
                 --no-log-stderr \
                 --json \
                 --summary-only",
            ),
        "failed to start pathological runtime",
    )
}

fn start_leserpent(
    cfg: &ThreeModuleStackConfig,
    state_path: &Path,
    log_path: &Path,
) -> Result<Child, ValidationError> {
    let stdout = File::create(log_path)?;
    let mut cmd = Command::new(dotnet_binary());
    if dotnet_home_bin().is_some() {
        cmd.env(
            "DOTNET_ROOT",
            env::var("DOTNET_ROOT").unwrap_or_else(|_| {
                dotnet_home_bin()
                    .and_then(|path| path.parent().map(Path::to_path_buf))
                    .unwrap_or_else(|| PathBuf::from(""))
                    .display()
                    .to_string()
            }),
        );
    }
    cmd.env("LESERPENT_STATE_PATH", state_path)
        .env("DOTNET_CLI_TELEMETRY_OPTOUT", "1")
        .arg("run")
        .arg("--project")
        .arg(cfg.leserpent_root.join("src/Leserpent/Leserpent.csproj"));
    if cfg.leserpent_dotnet_no_restore {
        cmd.arg("--no-restore");
    }
    cmd.arg("--no-launch-profile")
        .arg("--urls")
        .arg(format!("http://127.0.0.1:{}", cfg.lesp_port))
        .stdout(Stdio::from(stdout.try_clone()?))
        .stderr(Stdio::from(stdout));
    cmd.spawn()
        .map_err(|err| ValidationError::new(format!("failed to start leserpent: {err}")))
}

fn run_dotnet_restore(cfg: &ThreeModuleStackConfig) -> Result<(), ValidationError> {
    let mut cmd = Command::new(dotnet_binary());
    if dotnet_home_bin().is_some() {
        cmd.env(
            "DOTNET_ROOT",
            env::var("DOTNET_ROOT").unwrap_or_else(|_| {
                dotnet_home_bin()
                    .and_then(|path| path.parent().map(Path::to_path_buf))
                    .unwrap_or_else(|| PathBuf::from(""))
                    .display()
                    .to_string()
            }),
        );
    }
    cmd.env("DOTNET_CLI_TELEMETRY_OPTOUT", "1")
        .arg("restore")
        .arg(cfg.leserpent_root.join("src/Leserpent/Leserpent.csproj"));
    if cfg.leserpent_dotnet_ignore_failed_sources {
        cmd.arg("--ignore-failed-sources");
    }
    run_command(&mut cmd, "dotnet restore failed")
}

fn run_pathology_container(
    cfg: &PathologyConfig,
    suffix: &str,
    scenario: &str,
    count: usize,
) -> Result<(), ValidationError> {
    let name = format!("{}-{suffix}", cfg.patho_prefix);
    let _ = Command::new("docker").args(["rm", "-f", &name]).output();
    let output = Command::new("docker")
        .arg("run")
        .arg("--rm")
        .arg("--name")
        .arg(&name)
        .arg("--network")
        .arg(&cfg.network_name)
        .arg("-v")
        .arg(format!(
            "{}:/pathology:ro",
            cfg.pathology_fixture_dir.display()
        ))
        .arg(&cfg.image_tag)
        .arg("python3")
        .arg("/pathology/pathology_client.py")
        .arg("--scenario")
        .arg(scenario)
        .arg("--host")
        .arg(&cfg.gw_name)
        .arg("--port")
        .arg("9000")
        .arg("--count")
        .arg(count.to_string())
        .output()
        .map_err(|err| ValidationError::new(format!("failed to run pathology container: {err}")))?;
    fs::write(cfg.out_dir.join(format!("{suffix}.log")), &output.stdout)?;
    if !output.status.success() {
        return Err(ValidationError::new(format!(
            "pathology container `{suffix}` failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    Ok(())
}

fn ingest_template(container_name: &str, template: &str) -> Result<(), ValidationError> {
    run_command(
        Command::new("docker")
            .arg("exec")
            .arg(container_name)
            .arg("/stack-target/gewyvern/debug/gewyvern_socket_send")
            .arg("--tcp-socket")
            .arg("127.0.0.1:9000")
            .arg("--template")
            .arg(template),
        "failed to ingest template",
    )
}

fn inject_socket_bad_json(container_name: &str, count: usize) -> Result<(), ValidationError> {
    run_command(
        Command::new("docker")
            .arg("exec")
            .arg(container_name)
            .arg("bash")
            .arg("-lc")
            .arg(format!(
                "set -euo pipefail\nfor _ in $(seq 1 {count}); do exec 3<>/dev/tcp/127.0.0.1/9000; printf '{{\"bad\":\"json\"\\n' >&3 || true; exec 3>&-; exec 3<&-; done"
            )),
        "failed to inject malformed socket payloads",
    )
}

fn ensure_docker_network(network_name: &str) -> Result<(), ValidationError> {
    let _ = Command::new("docker")
        .args(["network", "rm", network_name])
        .output();
    run_command(
        Command::new("docker").args(["network", "create", network_name]),
        "failed to create docker network",
    )?;
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        let status = Command::new("docker")
            .args(["network", "inspect", network_name])
            .status()
            .map_err(|err| {
                ValidationError::new(format!("failed to inspect docker network: {err}"))
            })?;
        if status.success() {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(250));
    }
    Err(ValidationError::new(format!(
        "docker network did not become ready: {network_name}"
    )))
}

fn stack_probe_or_logs(
    profile: &str,
    url: &str,
    token: Option<&str>,
    output: &Path,
    container_name: &str,
    context: &str,
) -> Result<(), ValidationError> {
    probe_with_logs(profile, url, token, output, container_name, context).map(|_| ())
}

fn probe_with_logs(
    profile: &str,
    url: &str,
    token: Option<&str>,
    output: &Path,
    container_name: &str,
    context: &str,
) -> Result<String, ValidationError> {
    match stack_probe_to_path(profile, url, token, output) {
        Ok(body) => Ok(body),
        Err(err) => {
            let logs = docker_logs(container_name).unwrap_or_default();
            Err(ValidationError::new(format!("{context}: {err}\n{logs}")))
        }
    }
}

fn stack_probe_to_path(
    profile: &str,
    url: &str,
    token: Option<&str>,
    output: &Path,
) -> Result<String, ValidationError> {
    run_stack_probe_validation(url, profile, token, Some(output.to_path_buf()))?;
    fs::read_to_string(output).map_err(ValidationError::from)
}

fn wait_http_status_200(url: &str, timeout: Duration) -> Result<(), ValidationError> {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if curl_http_ok(url) {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(250));
    }
    Err(ValidationError::new(format!(
        "timed out waiting for HTTP 200 from {url}"
    )))
}

fn wait_for_ftp_banner(target: &str, timeout: Duration) -> Result<(), ValidationError> {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if let Ok(output) = Command::new("curl")
            .arg("-v")
            .arg("--max-time")
            .arg("3")
            .arg(format!("ftp://{target}/"))
            .output()
        {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("220 (vsFTPd") || stderr.contains("< 220") {
                return Ok(());
            }
        }
        thread::sleep(Duration::from_millis(500));
    }
    Err(ValidationError::new(format!(
        "timed out waiting for FTP banner from {target}"
    )))
}

fn wait_for_ldap_bind_ready(
    host_port: u16,
    admin_dn: &str,
    admin_password: &str,
    search_base: &str,
    timeout: Duration,
) -> Result<(), ValidationError> {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if let Ok(output) = Command::new("ldapsearch")
            .arg("-x")
            .arg("-H")
            .arg(format!("ldap://127.0.0.1:{host_port}"))
            .arg("-D")
            .arg(admin_dn)
            .arg("-w")
            .arg(admin_password)
            .arg("-b")
            .arg(search_base)
            .output()
        {
            if output.status.success() {
                return Ok(());
            }
        }
        thread::sleep(Duration::from_millis(500));
    }
    Err(ValidationError::new(format!(
        "timed out waiting for LDAP bind readiness on 127.0.0.1:{host_port}"
    )))
}

fn curl_http_ok(url: &str) -> bool {
    Command::new("curl")
        .arg("-fsS")
        .arg("--max-time")
        .arg("2")
        .arg(url)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn post_json_status(
    url: &str,
    body: &str,
    headers: &[(&str, &str)],
) -> Result<(), ValidationError> {
    let mut command = Command::new("curl");
    command.arg("-fsS").arg("-X").arg("POST").arg(url);
    for (name, value) in headers {
        command.arg("-H").arg(format!("{name}: {value}"));
    }
    command.arg("-H").arg("content-type: application/json");
    command.arg("--data").arg(body);
    run_command(&mut command, "curl JSON POST failed")
}

fn post_empty_status(url: &str, headers: &[(&str, &str)]) -> Result<(), ValidationError> {
    let mut command = Command::new("curl");
    command.arg("-fsS").arg("-X").arg("POST").arg(url);
    for (name, value) in headers {
        command.arg("-H").arg(format!("{name}: {value}"));
    }
    run_command(&mut command, "curl POST failed")
}

fn curl_get_http_body(url: &str) -> Result<String, ValidationError> {
    let output = Command::new("curl")
        .arg("-fsS")
        .arg(url)
        .output()
        .map_err(|err| ValidationError::new(format!("failed to run curl: {err}")))?;
    if !output.status.success() {
        return Err(ValidationError::new(format!(
            "curl GET failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    String::from_utf8(output.stdout)
        .map_err(|err| ValidationError::new(format!("curl output was not UTF-8: {err}")))
}

fn curl_capture_http_exchange(
    url: &str,
    headers_path: &Path,
    body_path: &Path,
) -> Result<u16, ValidationError> {
    let output = Command::new("curl")
        .arg("-sS")
        .arg("-D")
        .arg(headers_path)
        .arg("-o")
        .arg(body_path)
        .arg("-w")
        .arg("%{http_code}")
        .arg(url)
        .output()
        .map_err(|err| ValidationError::new(format!("failed to run curl capture: {err}")))?;
    if !output.status.success() {
        return Err(ValidationError::new(format!(
            "curl capture failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    let rendered = String::from_utf8_lossy(&output.stdout);
    rendered
        .trim()
        .parse::<u16>()
        .map_err(|err| ValidationError::new(format!("invalid curl status `{rendered}`: {err}")))
}

fn curl_capture_ftp_denied_exchange(
    url: &str,
    stderr_path: &Path,
    stdout_path: &Path,
) -> Result<i32, ValidationError> {
    let output = Command::new("curl")
        .arg("-v")
        .arg("--max-time")
        .arg("10")
        .arg("--user")
        .arg("bad:bad")
        .arg(url)
        .output()
        .map_err(|err| ValidationError::new(format!("failed to run FTP curl capture: {err}")))?;
    fs::write(stderr_path, &output.stderr)?;
    fs::write(stdout_path, &output.stdout)?;
    Ok(output.status.code().unwrap_or(-1))
}

fn ldap_capture_bind_denied_exchange(
    host_port: u16,
    admin_dn: &str,
    search_base: &str,
    stderr_path: &Path,
    stdout_path: &Path,
) -> Result<i32, ValidationError> {
    let output = Command::new("ldapsearch")
        .arg("-x")
        .arg("-H")
        .arg(format!("ldap://127.0.0.1:{host_port}"))
        .arg("-D")
        .arg(admin_dn)
        .arg("-w")
        .arg("wrong")
        .arg("-b")
        .arg(search_base)
        .output()
        .map_err(|err| ValidationError::new(format!("failed to run LDAP denied capture: {err}")))?;
    fs::write(stderr_path, &output.stderr)?;
    fs::write(stdout_path, &output.stdout)?;
    Ok(output.status.code().unwrap_or(-1))
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

fn require_dotnet() -> Result<(), ValidationError> {
    if dotnet_home_bin().is_some() {
        return Ok(());
    }
    require_cmd("dotnet")
}

fn ensure_docker_reachable() -> Result<(), ValidationError> {
    let status = Command::new("docker")
        .arg("info")
        .status()
        .map_err(|err| ValidationError::new(format!("failed to query docker: {err}")))?;
    if status.success() {
        Ok(())
    } else {
        Err(ValidationError::new(
            "docker daemon is not reachable; start Docker Desktop or another local daemon and retry",
        ))
    }
}

fn ensure_juice_shop_image(name: &str) -> Result<(), ValidationError> {
    let status = Command::new("docker")
        .args(["image", "inspect", name])
        .status()
        .map_err(|err| ValidationError::new(format!("failed to inspect image `{name}`: {err}")))?;
    if status.success() {
        return Ok(());
    }
    run_command(
        Command::new("docker").args(["pull", name]),
        "failed to pull Juice Shop image",
    )
}

fn ensure_ftp_denied_image(name: &str) -> Result<(), ValidationError> {
    let status = Command::new("docker")
        .args(["image", "inspect", name])
        .status()
        .map_err(|err| ValidationError::new(format!("failed to inspect image `{name}`: {err}")))?;
    if status.success() {
        return Ok(());
    }
    run_command(
        Command::new("docker").args(["pull", name]),
        "failed to pull FTP denied image",
    )
}

fn ensure_ldap_bind_denied_image(name: &str) -> Result<(), ValidationError> {
    let status = Command::new("docker")
        .args(["image", "inspect", name])
        .status()
        .map_err(|err| ValidationError::new(format!("failed to inspect image `{name}`: {err}")))?;
    if status.success() {
        return Ok(());
    }
    run_command(
        Command::new("docker").args(["pull", name]),
        "failed to pull LDAP bind denied image",
    )
}

fn ensure_docker_image(name: &str) -> Result<(), ValidationError> {
    let status = Command::new("docker")
        .args(["image", "inspect", name])
        .status()
        .map_err(|err| ValidationError::new(format!("failed to inspect image `{name}`: {err}")))?;
    if status.success() {
        Ok(())
    } else {
        Err(ValidationError::new(format!(
            "SKIP_DOCKER_BUILD=true but image is missing: {name}"
        )))
    }
}

fn ensure_dir_exists(path: &Path, label: &str) -> Result<(), ValidationError> {
    if path.is_dir() {
        Ok(())
    } else {
        Err(ValidationError::new(format!(
            "{label} at {}",
            path.display()
        )))
    }
}

fn docker_logs(container_name: &str) -> Result<String, ValidationError> {
    let output = Command::new("docker")
        .args(["logs", container_name])
        .output()
        .map_err(|err| ValidationError::new(format!("failed to read docker logs: {err}")))?;
    let mut text = String::from_utf8_lossy(&output.stdout).to_string();
    if text.is_empty() {
        text = String::from_utf8_lossy(&output.stderr).to_string();
    }
    Ok(text)
}

fn detect_default_route_device() -> Result<String, ValidationError> {
    let output = Command::new("sh")
        .arg("-lc")
        .arg("ip route show default | awk 'NR==1 {print $5}'")
        .output()
        .map_err(|err| {
            ValidationError::new(format!("failed to detect default route device: {err}"))
        })?;
    if !output.status.success() {
        return Err(ValidationError::new(format!(
            "failed to detect default route device: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    let device = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if device.is_empty() {
        return Err(ValidationError::new(
            "failed to detect default route device: empty result",
        ));
    }
    Ok(device)
}

fn read_container_file(container_name: &str, path: &str) -> Result<String, ValidationError> {
    let output = Command::new("docker")
        .args(["exec", container_name, "cat", path])
        .output()
        .map_err(|err| ValidationError::new(format!("failed to read container file: {err}")))?;
    if !output.status.success() {
        return Err(ValidationError::new(format!(
            "failed to read `{path}` from `{container_name}`: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn run_command(command: &mut Command, context: &str) -> Result<(), ValidationError> {
    let Output {
        status,
        stdout: _,
        stderr,
    } = command
        .output()
        .map_err(|err| ValidationError::new(format!("{context}: {err}")))?;
    if status.success() {
        return Ok(());
    }
    Err(ValidationError::new(format!(
        "{context}: {}\n{}",
        status,
        String::from_utf8_lossy(&stderr).trim()
    )))
}

fn dotnet_home_bin() -> Option<PathBuf> {
    let home = env::var("HOME").ok()?;
    let path = PathBuf::from(home).join(".dotnet/dotnet");
    path.is_file().then_some(path)
}

fn dotnet_binary() -> PathBuf {
    dotnet_home_bin().unwrap_or_else(|| PathBuf::from("dotnet"))
}

fn make_temp_dir(prefix: &str) -> Result<PathBuf, ValidationError> {
    let dir = temp_dir_preview(prefix);
    if dir.exists() {
        fs::remove_dir_all(&dir)?;
    }
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

fn temp_dir_preview(prefix: &str) -> PathBuf {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    env::temp_dir().join(format!("{prefix}.{now}"))
}

fn find_free_loopback_port() -> Result<u16, ValidationError> {
    let listener = TcpListener::bind("127.0.0.1:0")
        .map_err(|err| ValidationError::new(format!("failed to reserve loopback port: {err}")))?;
    listener
        .local_addr()
        .map(|addr| addr.port())
        .map_err(|err| ValidationError::new(format!("failed to inspect loopback port: {err}")))
}

fn env_string(name: &str, default: &str) -> String {
    env::var(name).unwrap_or_else(|_| default.to_string())
}

fn env_bool(name: &str, default: bool) -> bool {
    env::var(name)
        .ok()
        .map(|value| matches!(value.as_str(), "1" | "true" | "yes"))
        .unwrap_or(default)
}

fn env_u16(name: &str, default: u16) -> Result<u16, ValidationError> {
    match env::var(name) {
        Ok(value) => value
            .parse::<u16>()
            .map_err(|err| ValidationError::new(format!("invalid {name} value `{value}`: {err}"))),
        Err(_) => Ok(default),
    }
}

fn env_path(name: &str, default: &str) -> PathBuf {
    PathBuf::from(env::var(name).unwrap_or_else(|_| default.to_string()))
}
