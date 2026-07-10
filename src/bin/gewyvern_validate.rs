use std::env;
use std::path::PathBuf;
use std::process;

#[path = "../validation_harness_cli_stack.rs"]
mod gewyvern_validate_stack;

use gewyvern::validation_harness::{
    ReleaseCheckMode, ReleaseGateOptions, RemoteLinuxHostOptions, ValidationError,
    run_container_operator_path_validation, run_container_protocol_validation,
    run_container_runtime_validation, run_container_validation_summary,
    run_debugger_cross_validation, run_external_engine_roundtrip_demo, run_field_smoke_validation,
    run_high_frequency_validation, run_linux_attach_smoke, run_linux_kprobe_smoke,
    run_linux_tc_smoke, run_package_install_smoke, run_pathological_container_validation,
    run_registry_validation, run_release_container_check, run_release_gate,
    run_remote_linux_host_validation, run_resilience_bundle_validation,
    run_resilience_drive_bad_json_validation, run_resilience_emit_helper_validation,
    run_resilience_log_evidence_validation, run_resilience_roundtrip_validation,
    run_runtime_lifecycle_validation, run_runtime_operator_validation, run_socket_roundtrip_demo,
    run_three_module_stack_smoke, run_training_dataset_roundtrip_demo,
};

fn main() {
    if let Err(err) = run() {
        eprintln!("validation failed: {err}");
        process::exit(1);
    }
}

fn run() -> Result<(), ValidationError> {
    let mut args = env::args().skip(1);
    let command = args.next().unwrap_or_else(|| "help".to_string());
    let rest = args.collect::<Vec<_>>();

    if gewyvern_validate_stack::run_stack_command(&command, rest.clone())? {
        return Ok(());
    }

    match command.as_str() {
        "debugger-cross" => {
            let options = parse_options(rest)?;
            let report = run_debugger_cross_validation(options.out_dir)?;

            println!("{}: ok", report.name);
            println!("checks: {}", report.checks.join(", "));
            println!("evidence: {}", report.out_dir.display());
            println!(
                "index: {}",
                report.out_dir.join("evidence-index.json").display()
            );
            Ok(())
        }
        "registry" => {
            let options = parse_options(rest)?;
            let report = run_registry_validation(options.out_dir, options.limit)?;

            println!("{}: ok", report.name);
            println!("checks: {}", report.checks.len());
            println!("evidence: {}", report.out_dir.display());
            Ok(())
        }
        "high-frequency" => {
            let options = parse_options(rest)?;
            let report = run_high_frequency_validation(options.out_dir)?;

            println!("{}: ok", report.name);
            println!("checks: {}", report.checks.join(", "));
            println!("evidence: {}", report.out_dir.display());
            Ok(())
        }
        "field-smoke" => {
            let options = parse_options(rest)?;
            let report =
                run_field_smoke_validation(options.out_dir, options.socket, options.scan_all)?;

            println!("{}: ok", report.name);
            println!("checks: {}", report.checks.join(", "));
            println!("evidence: {}", report.out_dir.display());
            Ok(())
        }
        "socket-roundtrip" => {
            let options = parse_options(rest)?;
            let report = run_socket_roundtrip_demo(
                options.socket_target.as_deref(),
                options.template.as_deref(),
                options.output,
                options.socket_kind.as_deref(),
            )?;

            println!("{}: ok", report.name);
            println!("checks: {}", report.checks.join(", "));
            println!("evidence: {}", report.out_dir.display());
            Ok(())
        }
        "training-roundtrip" => {
            let options = parse_options(rest)?;
            let report = run_training_dataset_roundtrip_demo(
                options.api_addr.as_deref(),
                options.out_dir,
                options.target_path_segment.as_deref(),
                options.limit,
            )?;

            println!("{}: ok", report.name);
            println!("checks: {}", report.checks.join(", "));
            println!("evidence: {}", report.out_dir.display());
            Ok(())
        }
        "external-engine-roundtrip" => {
            let options = parse_options(rest)?;
            let report = run_external_engine_roundtrip_demo(
                options.ingest_addr.as_deref(),
                options.api_addr.as_deref(),
                options.template.as_deref(),
                options.analysis_out,
                options.engine_out,
                options.target_path_segment.as_deref(),
                options.engine_root,
                options.engine_cmd.as_deref(),
            )?;

            println!("{}: ok", report.name);
            println!("checks: {}", report.checks.join(", "));
            println!("evidence: {}", report.out_dir.display());
            Ok(())
        }
        "runtime-lifecycle" => {
            let options = parse_options(rest)?;
            let report = run_runtime_lifecycle_validation(options.out_dir)?;

            println!("{}: ok", report.name);
            println!("checks: {}", report.checks.join(", "));
            println!("evidence: {}", report.out_dir.display());
            Ok(())
        }
        "resilience-log-evidence" => {
            let options = parse_options(rest)?;
            let log_source = require_path_option(options.log_source, "--log-source")?;
            let report = run_resilience_log_evidence_validation(log_source, options.out_dir)?;

            println!("{}: ok", report.name);
            println!("checks: {}", report.checks.join(", "));
            println!("evidence: {}", report.out_dir.display());
            Ok(())
        }
        "resilience-roundtrip" => {
            let options = parse_options(rest)?;
            let report =
                run_resilience_roundtrip_validation(options.api_addr.as_deref(), options.out_dir)?;

            println!("{}: ok", report.name);
            println!("checks: {}", report.checks.join(", "));
            println!("evidence: {}", report.out_dir.display());
            Ok(())
        }
        "resilience-bundle" => {
            let options = parse_options(rest)?;
            let log_source = require_path_option(options.log_source, "--log-source")?;
            let report = run_resilience_bundle_validation(
                options.api_addr.as_deref(),
                log_source,
                options.out_dir,
            )?;

            println!("{}: ok", report.name);
            println!("checks: {}", report.checks.join(", "));
            println!("evidence: {}", report.out_dir.display());
            Ok(())
        }
        "resilience-emit-helper" => {
            let options = parse_options(rest)?;
            let mode = require_string_option(options.mode, "--mode")?;
            let output_path = require_path_option(options.output, "--output")?;
            let report = run_resilience_emit_helper_validation(&mode, output_path)?;

            println!("{}: ok", report.name);
            println!("checks: {}", report.checks.join(", "));
            println!("evidence: {}", report.out_dir.display());
            Ok(())
        }
        "resilience-drive-bad-json" => {
            let options = parse_options(rest)?;
            let host = require_string_option(options.host, "--host")?;
            let port = require_u16_option(options.port, "--port")?;
            let report = run_resilience_drive_bad_json_validation(
                &host,
                port,
                options.count.unwrap_or(5),
                options.out_dir,
            )?;

            println!("{}: ok", report.name);
            println!("checks: {}", report.checks.join(", "));
            println!("evidence: {}", report.out_dir.display());
            Ok(())
        }
        "runtime-operator" => {
            let options = parse_options(rest)?;
            let report = run_runtime_operator_validation(options.out_dir, options.json_out)?;

            println!("{}: ok", report.name);
            println!("checks: {}", report.checks.join(", "));
            println!("evidence: {}", report.out_dir.display());
            Ok(())
        }
        "linux-attach-smoke" => {
            if wants_subcommand_help(&rest) {
                print_linux_attach_smoke_help();
                return Ok(());
            }
            let options = parse_linux_ebpf_smoke_options(
                rest,
                "syscalls/sys_enter_nanosleep",
                "--hookpoint",
            )?;
            let report = run_linux_attach_smoke(&options.target, options.out_dir)?;

            println!("{}: ok", report.name);
            println!("checks: {}", report.checks.join(", "));
            println!("evidence: {}", report.out_dir.display());
            Ok(())
        }
        "linux-kprobe-smoke" => {
            if wants_subcommand_help(&rest) {
                print_linux_kprobe_smoke_help();
                return Ok(());
            }
            let options = parse_linux_ebpf_smoke_options(rest, "ip_route_output_flow", "--symbol")?;
            let report = run_linux_kprobe_smoke(&options.target, options.out_dir)?;

            println!("{}: ok", report.name);
            println!("checks: {}", report.checks.join(", "));
            println!("evidence: {}", report.out_dir.display());
            Ok(())
        }
        "linux-tc-smoke" => {
            if wants_subcommand_help(&rest) {
                print_linux_tc_smoke_help();
                return Ok(());
            }
            let options = parse_linux_ebpf_smoke_options(rest, "eth0", "--dev")?;
            let report = run_linux_tc_smoke(&options.target, options.out_dir)?;

            println!("{}: ok", report.name);
            println!("checks: {}", report.checks.join(", "));
            println!("evidence: {}", report.out_dir.display());
            Ok(())
        }
        "release-container-check" => {
            if wants_subcommand_help(&rest) {
                print_release_container_check_help();
                return Ok(());
            }
            let mode = parse_release_check_mode("release-container-check", rest)?;
            let report = run_release_container_check(mode)?;

            println!("{}: ok", report.name);
            println!("checks: {}", report.checks.join(", "));
            println!("evidence: {}", report.out_dir.display());
            Ok(())
        }
        "package-install-smoke" => {
            if wants_subcommand_help(&rest) {
                print_package_install_smoke_help();
                return Ok(());
            }
            let mode = parse_release_check_mode("package-install-smoke", rest)?;
            let report = run_package_install_smoke(mode)?;

            println!("{}: ok", report.name);
            println!("checks: {}", report.checks.join(", "));
            println!("evidence: {}", report.out_dir.display());
            Ok(())
        }
        "container-protocol-validation" => {
            if wants_subcommand_help(&rest) {
                print_container_protocol_validation_help();
                return Ok(());
            }
            let mode = parse_release_check_mode("container-protocol-validation", rest)?;
            let report = run_container_protocol_validation(mode)?;

            println!("{}: ok", report.name);
            println!("checks: {}", report.checks.join(", "));
            println!("evidence: {}", report.out_dir.display());
            Ok(())
        }
        "container-operator-path-validation" => {
            if wants_subcommand_help(&rest) {
                print_container_operator_path_validation_help();
                return Ok(());
            }
            let mode = parse_release_check_mode("container-operator-path-validation", rest)?;
            let report = run_container_operator_path_validation(mode)?;

            println!("{}: ok", report.name);
            println!("checks: {}", report.checks.join(", "));
            println!("evidence: {}", report.out_dir.display());
            Ok(())
        }
        "container-validation-summary" => {
            if wants_subcommand_help(&rest) {
                print_container_validation_summary_help();
                return Ok(());
            }
            let mode = parse_release_check_mode("container-validation-summary", rest)?;
            let report = run_container_validation_summary(mode)?;

            println!("{}: ok", report.name);
            println!("checks: {}", report.checks.join(", "));
            println!("evidence: {}", report.out_dir.display());
            Ok(())
        }
        "container-runtime-validation" => {
            if wants_subcommand_help(&rest) {
                print_container_runtime_validation_help();
                return Ok(());
            }
            let mode = parse_release_check_mode("container-runtime-validation", rest)?;
            let report = run_container_runtime_validation(mode)?;

            println!("{}: ok", report.name);
            println!("checks: {}", report.checks.join(", "));
            println!("evidence: {}", report.out_dir.display());
            Ok(())
        }
        "remote-linux-host-validation" => {
            if wants_subcommand_help(&rest) {
                print_remote_linux_host_validation_help();
                return Ok(());
            }
            let options = parse_remote_linux_host_options(rest)?;
            let report = run_remote_linux_host_validation(options)?;

            println!("{}: ok", report.name);
            println!("checks: {}", report.checks.join(", "));
            println!("evidence: {}", report.out_dir.display());
            Ok(())
        }
        "release-gate" => {
            if wants_subcommand_help(&rest) {
                print_release_gate_help();
                return Ok(());
            }
            let options = parse_release_gate_options(rest)?;
            let report = run_release_gate(options)?;

            println!("{}: ok", report.name);
            println!("checks: {}", report.checks.join(", "));
            println!("evidence: {}", report.out_dir.display());
            Ok(())
        }
        "three-module-stack-smoke" => {
            if wants_subcommand_help(&rest) {
                print_three_module_stack_smoke_help();
                return Ok(());
            }
            if !rest.is_empty() {
                return Err(ValidationError::new(
                    "three-module-stack-smoke does not accept positional arguments",
                ));
            }
            let report = run_three_module_stack_smoke()?;

            println!("{}: ok", report.name);
            println!("checks: {}", report.checks.join(", "));
            println!("evidence: {}", report.out_dir.display());
            Ok(())
        }
        "pathological-container-validation" => {
            if wants_subcommand_help(&rest) {
                print_pathological_container_validation_help();
                return Ok(());
            }
            let out_dir = parse_optional_out_dir(rest)?;
            let report = run_pathological_container_validation(out_dir)?;

            println!("{}: ok", report.name);
            println!("checks: {}", report.checks.join(", "));
            println!("evidence: {}", report.out_dir.display());
            Ok(())
        }
        "list" => {
            println!("container-operator-path-validation");
            println!("container-protocol-validation");
            println!("container-runtime-validation");
            println!("container-validation-summary");
            println!("debugger-cross");
            println!("external-engine-roundtrip");
            println!("field-smoke");
            println!("high-frequency");
            println!("package-install-smoke");
            println!("pathological-container-validation");
            println!("remote-linux-host-validation");
            println!("release-container-check");
            println!("release-gate");
            println!("registry");
            println!("resilience-bundle");
            println!("resilience-drive-bad-json");
            println!("resilience-emit-helper");
            println!("resilience-log-evidence");
            println!("resilience-roundtrip");
            println!("runtime-lifecycle");
            println!("runtime-operator");
            println!("socket-roundtrip");
            println!("three-module-stack-smoke");
            gewyvern_validate_stack::print_stack_list();
            println!("training-roundtrip");
            Ok(())
        }
        "help" | "--help" | "-h" => {
            print_help();
            Ok(())
        }
        other => Err(ValidationError::new(format!(
            "unknown validation command `{other}`; try `gewyvern_validate list`"
        ))),
    }
}

struct Options {
    out_dir: Option<PathBuf>,
    limit: Option<usize>,
    json_out: Option<PathBuf>,
    log_source: Option<PathBuf>,
    api_addr: Option<String>,
    ingest_addr: Option<String>,
    output: Option<PathBuf>,
    analysis_out: Option<PathBuf>,
    engine_out: Option<PathBuf>,
    engine_root: Option<PathBuf>,
    engine_cmd: Option<String>,
    mode: Option<String>,
    host: Option<String>,
    port: Option<u16>,
    count: Option<usize>,
    socket_target: Option<String>,
    socket_kind: Option<String>,
    template: Option<String>,
    target_path_segment: Option<String>,
    socket: bool,
    scan_all: bool,
}

struct RemoteLinuxHostCliOptions {
    host: String,
    remote_dir: Option<String>,
    build_packages: bool,
    keep_remote_dir: bool,
}

struct LinuxEbpfSmokeCliOptions {
    target: String,
    out_dir: Option<PathBuf>,
}

fn parse_options(args: Vec<String>) -> Result<Options, ValidationError> {
    let mut out_dir = None;
    let mut limit = None;
    let mut json_out = None;
    let mut log_source = None;
    let mut api_addr = None;
    let mut ingest_addr = None;
    let mut output = None;
    let mut analysis_out = None;
    let mut engine_out = None;
    let mut engine_root = None;
    let mut engine_cmd = None;
    let mut mode = None;
    let mut host = None;
    let mut port = None;
    let mut count = None;
    let mut socket_target = None;
    let mut socket_kind = None;
    let mut template = None;
    let mut target_path_segment = None;
    let mut socket = env_flag("GEWY_FIELD_VALIDATE_SOCKET");
    let mut scan_all = env_flag("GEWY_FIELD_VALIDATE_SCAN_ALL");
    let mut iter = args.into_iter();

    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--out-dir" => {
                let value = iter
                    .next()
                    .ok_or_else(|| ValidationError::new("--out-dir requires a path"))?;
                out_dir = Some(PathBuf::from(value));
            }
            "--limit" => {
                let value = iter
                    .next()
                    .ok_or_else(|| ValidationError::new("--limit requires a number"))?;
                let parsed = value.parse::<usize>().map_err(|err| {
                    ValidationError::new(format!("invalid --limit value `{value}`: {err}"))
                })?;
                limit = Some(parsed);
            }
            "--json-out" => {
                let value = iter
                    .next()
                    .ok_or_else(|| ValidationError::new("--json-out requires a path"))?;
                json_out = Some(PathBuf::from(value));
            }
            "--log-source" => {
                let value = iter
                    .next()
                    .ok_or_else(|| ValidationError::new("--log-source requires a path"))?;
                log_source = Some(PathBuf::from(value));
            }
            "--api-addr" => {
                let value = iter
                    .next()
                    .ok_or_else(|| ValidationError::new("--api-addr requires an address"))?;
                api_addr = Some(value);
            }
            "--ingest-addr" => {
                let value = iter
                    .next()
                    .ok_or_else(|| ValidationError::new("--ingest-addr requires an address"))?;
                ingest_addr = Some(value);
            }
            "--output" => {
                let value = iter
                    .next()
                    .ok_or_else(|| ValidationError::new("--output requires a path"))?;
                output = Some(PathBuf::from(value));
            }
            "--analysis-out" => {
                let value = iter
                    .next()
                    .ok_or_else(|| ValidationError::new("--analysis-out requires a path"))?;
                analysis_out = Some(PathBuf::from(value));
            }
            "--engine-out" => {
                let value = iter
                    .next()
                    .ok_or_else(|| ValidationError::new("--engine-out requires a path"))?;
                engine_out = Some(PathBuf::from(value));
            }
            "--engine-root" => {
                let value = iter
                    .next()
                    .ok_or_else(|| ValidationError::new("--engine-root requires a path"))?;
                engine_root = Some(PathBuf::from(value));
            }
            "--engine-cmd" => {
                let value = iter
                    .next()
                    .ok_or_else(|| ValidationError::new("--engine-cmd requires a value"))?;
                engine_cmd = Some(value);
            }
            "--mode" => {
                let value = iter
                    .next()
                    .ok_or_else(|| ValidationError::new("--mode requires a value"))?;
                mode = Some(value);
            }
            "--host" => {
                let value = iter
                    .next()
                    .ok_or_else(|| ValidationError::new("--host requires a value"))?;
                host = Some(value);
            }
            "--port" => {
                let value = iter
                    .next()
                    .ok_or_else(|| ValidationError::new("--port requires a number"))?;
                let parsed = value.parse::<u16>().map_err(|err| {
                    ValidationError::new(format!("invalid --port value `{value}`: {err}"))
                })?;
                port = Some(parsed);
            }
            "--count" => {
                let value = iter
                    .next()
                    .ok_or_else(|| ValidationError::new("--count requires a number"))?;
                let parsed = value.parse::<usize>().map_err(|err| {
                    ValidationError::new(format!("invalid --count value `{value}`: {err}"))
                })?;
                count = Some(parsed);
            }
            "--socket-target" => {
                let value = iter
                    .next()
                    .ok_or_else(|| ValidationError::new("--socket-target requires a value"))?;
                socket_target = Some(value);
            }
            "--socket-kind" => {
                let value = iter
                    .next()
                    .ok_or_else(|| ValidationError::new("--socket-kind requires a value"))?;
                socket_kind = Some(value);
            }
            "--template" => {
                let value = iter
                    .next()
                    .ok_or_else(|| ValidationError::new("--template requires a value"))?;
                template = Some(value);
            }
            "--target-path-segment" => {
                let value = iter.next().ok_or_else(|| {
                    ValidationError::new("--target-path-segment requires a value")
                })?;
                target_path_segment = Some(value);
            }
            "--socket" => {
                socket = true;
            }
            "--scan-all" => {
                scan_all = true;
            }
            other => {
                return Err(ValidationError::new(format!(
                    "unknown validation option `{other}`"
                )));
            }
        }
    }

    Ok(Options {
        out_dir,
        limit,
        json_out,
        log_source,
        api_addr,
        ingest_addr,
        output,
        analysis_out,
        engine_out,
        engine_root,
        engine_cmd,
        mode,
        host,
        port,
        count,
        socket_target,
        socket_kind,
        template,
        target_path_segment,
        socket,
        scan_all,
    })
}

fn require_path_option(value: Option<PathBuf>, name: &str) -> Result<PathBuf, ValidationError> {
    value.ok_or_else(|| ValidationError::new(format!("{name} is required")))
}

fn require_string_option(value: Option<String>, name: &str) -> Result<String, ValidationError> {
    value.ok_or_else(|| ValidationError::new(format!("{name} is required")))
}

fn require_u16_option(value: Option<u16>, name: &str) -> Result<u16, ValidationError> {
    value.ok_or_else(|| ValidationError::new(format!("{name} is required")))
}

fn parse_release_check_mode(
    command_name: &str,
    args: Vec<String>,
) -> Result<ReleaseCheckMode, ValidationError> {
    let mut mode = ReleaseCheckMode::DebAndRpm;
    let mut iter = args.into_iter();

    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--deb" => mode = ReleaseCheckMode::Deb,
            "--rpm" => mode = ReleaseCheckMode::Rpm,
            other => {
                return Err(ValidationError::new(format!(
                    "unknown {command_name} option `{other}`"
                )));
            }
        }
    }

    Ok(mode)
}

fn parse_release_gate_options(args: Vec<String>) -> Result<ReleaseGateOptions, ValidationError> {
    let mut options = ReleaseGateOptions::default();
    let mut iter = args.into_iter();

    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--skip-build" => options.run_build = false,
            "--skip-release-check" => options.run_release_check = false,
            "--skip-stack" => options.run_stack = false,
            "--skip-pathology" => options.run_pathology = false,
            "--remote-host-validation" => options.run_remote_host = true,
            "--keep-remote-dir" => options.keep_remote_dir = true,
            "--skip-remote-build" => options.remote_build_packages = false,
            "--remote-host" => {
                options.remote_host = iter
                    .next()
                    .ok_or_else(|| ValidationError::new("--remote-host requires a value"))?;
                options.run_remote_host = true;
            }
            "--remote-dir" => {
                options.remote_dir = Some(
                    iter.next()
                        .ok_or_else(|| ValidationError::new("--remote-dir requires a value"))?,
                );
                options.run_remote_host = true;
            }
            "--deb" => options.release_mode = ReleaseCheckMode::Deb,
            "--rpm" => options.release_mode = ReleaseCheckMode::Rpm,
            other => {
                return Err(ValidationError::new(format!(
                    "unknown release-gate option `{other}`"
                )));
            }
        }
    }

    Ok(options)
}

fn parse_remote_linux_host_options(
    args: Vec<String>,
) -> Result<RemoteLinuxHostOptions, ValidationError> {
    let mut options = RemoteLinuxHostCliOptions {
        host: env::var("GEWY_REMOTE_HOST").unwrap_or_else(|_| "kyuubiki-lab".to_string()),
        remote_dir: None,
        build_packages: true,
        keep_remote_dir: false,
    };
    let mut iter = args.into_iter();

    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--host" => {
                options.host = iter
                    .next()
                    .ok_or_else(|| ValidationError::new("--host requires a value"))?;
            }
            "--remote-dir" => {
                options.remote_dir = Some(
                    iter.next()
                        .ok_or_else(|| ValidationError::new("--remote-dir requires a value"))?,
                );
            }
            "--skip-build" => options.build_packages = false,
            "--keep-remote-dir" => options.keep_remote_dir = true,
            other if other.starts_with('-') => {
                return Err(ValidationError::new(format!(
                    "unknown remote-linux-host-validation option `{other}`"
                )));
            }
            other => options.host = other.to_string(),
        }
    }

    Ok(RemoteLinuxHostOptions {
        host: options.host,
        remote_dir: options.remote_dir,
        build_packages: options.build_packages,
        keep_remote_dir: options.keep_remote_dir,
    })
}

fn parse_linux_ebpf_smoke_options(
    args: Vec<String>,
    default_target: &str,
    expected_flag: &str,
) -> Result<LinuxEbpfSmokeCliOptions, ValidationError> {
    let mut target = None;
    let mut out_dir = None;
    let mut iter = args.into_iter();

    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--hookpoint" | "--symbol" | "--dev" => {
                if arg != expected_flag {
                    return Err(ValidationError::new(format!(
                        "{expected_flag} is required for this command; got `{arg}`"
                    )));
                }
                target = Some(
                    iter.next()
                        .ok_or_else(|| ValidationError::new(format!("{arg} requires a value")))?,
                );
            }
            "--out-dir" => {
                let value = iter
                    .next()
                    .ok_or_else(|| ValidationError::new("--out-dir requires a path"))?;
                out_dir = Some(PathBuf::from(value));
            }
            other if other.starts_with('-') => {
                return Err(ValidationError::new(format!(
                    "unknown linux smoke option `{other}`"
                )));
            }
            other => target = Some(other.to_string()),
        }
    }

    Ok(LinuxEbpfSmokeCliOptions {
        target: target.unwrap_or_else(|| default_target.to_string()),
        out_dir,
    })
}

fn parse_optional_out_dir(args: Vec<String>) -> Result<Option<PathBuf>, ValidationError> {
    let mut out_dir = None;
    let mut iter = args.into_iter();

    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--out-dir" => {
                let value = iter
                    .next()
                    .ok_or_else(|| ValidationError::new("--out-dir requires a path"))?;
                out_dir = Some(PathBuf::from(value));
            }
            other if other.starts_with('-') => {
                return Err(ValidationError::new(format!(
                    "unknown pathological-container-validation option `{other}`"
                )));
            }
            other => {
                if out_dir.is_some() {
                    return Err(ValidationError::new(
                        "pathological-container-validation accepts at most one output path",
                    ));
                }
                out_dir = Some(PathBuf::from(other));
            }
        }
    }

    Ok(out_dir)
}

fn env_flag(name: &str) -> bool {
    matches!(env::var(name).as_deref(), Ok("1") | Ok("true") | Ok("yes"))
}

fn wants_subcommand_help(args: &[String]) -> bool {
    args.iter()
        .any(|arg| matches!(arg.as_str(), "-h" | "--help"))
}

fn print_help() {
    println!("gewyvern_validate");
    println!();
    println!("Native validation harness for gewyvern release and debugger checks.");
    println!();
    println!("Commands:");
    println!("  list");
    println!("  container-operator-path-validation [--deb|--rpm]");
    println!("  container-protocol-validation [--deb|--rpm]");
    println!("  container-runtime-validation [--deb|--rpm]");
    println!("  container-validation-summary [--deb|--rpm]");
    println!("  debugger-cross [--out-dir <path>]  # writes evidence-index.json");
    println!("  field-smoke [--out-dir <path>] [--socket] [--scan-all]");
    println!(
        "  socket-roundtrip [--socket-target <path-or-addr>] [--socket-kind <unix|tcp>] [--template <id>] [--output <path>]"
    );
    println!(
        "  training-roundtrip [--api-addr <addr>] [--out-dir <path>] [--target-path-segment <segment>] [--limit <n>]"
    );
    println!(
        "  external-engine-roundtrip [--ingest-addr <addr>] [--api-addr <addr>] [--template <id>] [--analysis-out <path>] [--engine-out <path>] [--target-path-segment <segment>] [--engine-root <path>] [--engine-cmd <cmd>]"
    );
    println!("  high-frequency [--out-dir <path>]");
    println!("  linux-attach-smoke [--hookpoint <category/event>] [--out-dir <path>]");
    println!("  linux-kprobe-smoke [--symbol <kernel-symbol>] [--out-dir <path>]");
    println!("  linux-tc-smoke --dev <netdev> [--out-dir <path>]");
    println!("  package-install-smoke [--deb|--rpm]");
    println!(
        "  remote-linux-host-validation [--host <ssh-host>] [--remote-dir <path>] [--skip-build] [--keep-remote-dir]"
    );
    println!("  registry [--out-dir <path>] [--limit <n>]");
    println!("  resilience-log-evidence --log-source <path> [--out-dir <path>]");
    println!("  resilience-roundtrip [--api-addr <addr>] [--out-dir <path>]");
    println!("  resilience-bundle --log-source <path> [--api-addr <addr>] [--out-dir <path>]");
    println!("  resilience-emit-helper --mode <timeout|fail|healthy> --output <path>");
    println!(
        "  resilience-drive-bad-json --host <host> --port <port> [--count <n>] [--out-dir <path>]"
    );
    println!("  pathological-container-validation [--out-dir <path>]");
    println!("  release-container-check [--deb|--rpm]");
    println!(
        "  release-gate [--skip-build] [--skip-release-check] [--skip-stack] [--skip-pathology] [--deb|--rpm]"
    );
    println!("  runtime-lifecycle [--out-dir <path>]");
    println!("  runtime-operator [--out-dir <path>] [--json-out <path>]");
    println!("  three-module-stack-smoke");
    gewyvern_validate_stack::print_stack_help();
}

fn print_release_container_check_help() {
    println!("Usage: gewyvern_validate release-container-check [--deb] [--rpm]");
    println!();
    println!("Run the current release-oriented packaged Linux validation suite:");
    println!("  package_install_smoke.sh");
    println!("  container_runtime_validation.sh");
    println!("  container_validation_summary.sh");
    println!();
    println!("By default, both the DEB and RPM paths run.");
}

fn print_package_install_smoke_help() {
    println!("Usage: gewyvern_validate package-install-smoke [--deb] [--rpm]");
    println!();
    println!("Run the packaged install smoke in clean Linux containers.");
    println!("By default, both the DEB and RPM paths run.");
}

fn print_linux_attach_smoke_help() {
    println!(
        "Usage: gewyvern_validate linux-attach-smoke [--hookpoint <category/event>] [--out-dir <path>]"
    );
    println!();
    println!(
        "Compile the minimal Linux tracepoint smoke object and loader, then attempt one real attach."
    );
    println!(
        "Run this on Linux with BPF attach privileges; unprivileged runs may fail with `Operation not permitted`."
    );
}

fn print_linux_kprobe_smoke_help() {
    println!(
        "Usage: gewyvern_validate linux-kprobe-smoke [--symbol <kernel-symbol>] [--out-dir <path>]"
    );
    println!();
    println!("Compile the minimal Linux kprobe smoke object and attempt one real kprobe attach.");
    println!(
        "Run this on Linux with BPF attach privileges; unprivileged runs may fail with `Operation not permitted`."
    );
}

fn print_linux_tc_smoke_help() {
    println!("Usage: gewyvern_validate linux-tc-smoke --dev <netdev> [--out-dir <path>]");
    println!();
    println!(
        "Compile the minimal Linux tc ingress smoke object and attempt one real tc filter attach."
    );
    println!(
        "Run this on Linux with BPF attach privileges and pass the default-route device explicitly."
    );
}

fn print_remote_linux_host_validation_help() {
    println!(
        "Usage: gewyvern_validate remote-linux-host-validation [--host <ssh-host>] [--remote-dir <path>] [--skip-build] [--keep-remote-dir]"
    );
    println!();
    println!(
        "Collect remote Linux/x86_64 preflight evidence, sync the current workspace over SSH, build x86_64 packages there, then run host-mode package and runtime smoke checks."
    );
    println!(
        "Defaults: host from GEWY_REMOTE_HOST or `kyuubiki-lab`, remote dir under `~/.kyuubiki-remote-runs/`."
    );
}

fn print_container_protocol_validation_help() {
    println!("Usage: gewyvern_validate container-protocol-validation [--deb] [--rpm]");
    println!();
    println!("Run the packaged protocol registry validation in clean Linux containers.");
    println!("By default, both the DEB and RPM paths run.");
}

fn print_container_operator_path_validation_help() {
    println!("Usage: gewyvern_validate container-operator-path-validation [--deb] [--rpm]");
    println!();
    println!("Run the packaged operator-path validation in clean Linux containers.");
    println!("By default, both the DEB and RPM paths run.");
}

fn print_container_validation_summary_help() {
    println!("Usage: gewyvern_validate container-validation-summary [--deb] [--rpm]");
    println!();
    println!("Run the packaged protocol + operator-path container validation summary.");
    println!("By default, both the DEB and RPM paths run.");
}

fn print_container_runtime_validation_help() {
    println!("Usage: gewyvern_validate container-runtime-validation [--deb] [--rpm]");
    println!();
    println!("Run the packaged standalone runtime validation in clean Linux containers.");
    println!("By default, both the DEB and RPM paths run.");
}

fn print_release_gate_help() {
    println!(
        "Usage: gewyvern_validate release-gate [--skip-build] [--skip-release-check] [--skip-stack] [--skip-pathology] [--remote-host-validation] [--remote-host <ssh-host>] [--remote-dir <path>] [--skip-remote-build] [--keep-remote-dir] [--deb|--rpm]"
    );
    println!();
    println!("Run the current release gate as one deliberate sequence:");
    println!("1. rebuild fresh native packages in Docker");
    println!("2. run the packaged release validation wrapper");
    println!("3. run the three-module stack smoke");
    println!("4. run pathological container/runtime-ingest validation");
    println!("5. optionally run remote Linux host validation over SSH");
    println!();
    println!("Flags:");
    println!("  --skip-build          Reuse current package artifacts instead of rebuilding");
    println!("  --skip-release-check  Skip packaged DEB/RPM validation");
    println!("  --skip-stack          Skip three-module stack smoke");
    println!("  --skip-pathology      Skip pathological runtime-ingest validation");
    println!("  --remote-host-validation  Run remote Linux host validation after local gates");
    println!("  --remote-host         Override the SSH host used for remote validation");
    println!("  --remote-dir          Override the remote workspace path");
    println!("  --skip-remote-build   Reuse existing remote artifacts instead of rebuilding there");
    println!("  --keep-remote-dir     Keep the remote workspace after the run");
    println!("  --deb                 Run the packaged release check in DEB-only mode");
    println!("  --rpm                 Run the packaged release check in RPM-only mode");
}

fn print_three_module_stack_smoke_help() {
    println!("Usage: gewyvern_validate three-module-stack-smoke");
    println!();
    println!(
        "Run the full gewyvern + etragon + leserpent stack smoke with native Rust orchestration."
    );
    println!("Environment variables from the legacy shell entrypoint are still honored.");
}

fn print_pathological_container_validation_help() {
    println!("Usage: gewyvern_validate pathological-container-validation [--out-dir <path>]");
    println!();
    println!("Run the pathological container/runtime-ingest validation suite.");
    println!(
        "A single positional output path is also accepted for compatibility with the legacy shell entrypoint."
    );
}
