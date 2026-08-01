use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process;

use serde_json::json;

#[path = "../validation_harness_cli_stack.rs"]
mod gewyvern_validate_stack;

use gewyvern::validation_harness::{
    ReleaseCheckMode, ReleaseGateOptions, RemoteLinuxHostOptions, ValidationError,
    read_bounded_json_file, read_bounded_nonempty_lines, read_bounded_phase_timings,
    read_bounded_unique_key_value_file, run_container_operator_path_validation,
    run_container_protocol_validation, run_container_runtime_validation,
    run_container_validation_summary, run_debugger_cross_validation,
    run_external_engine_roundtrip_demo, run_field_smoke_validation,
    run_ftp_denied_container_validation, run_high_frequency_validation,
    run_juice_shop_container_validation, run_ldap_bind_denied_container_validation,
    run_leselang_fuzz_validation, run_leserpent_accessibility_validation,
    run_leserpent_aot_validation, run_leserpent_benchmark_validation,
    run_leserpent_parity_recovery_validation, run_leserpent_schema_freeze_validation,
    run_leserpent_transport_validation, run_linux_attach_smoke, run_linux_kprobe_smoke,
    run_linux_tc_smoke, run_package_install_smoke, run_pathological_container_validation,
    run_registry_validation, run_release_container_check, run_release_gate,
    run_remote_linux_host_validation, run_resilience_bundle_validation,
    run_resilience_drive_bad_json_validation, run_resilience_emit_helper_validation,
    run_resilience_log_evidence_validation, run_resilience_roundtrip_validation,
    run_runtime_lifecycle_validation, run_runtime_operator_validation, run_socket_roundtrip_demo,
    run_three_module_stack_smoke, run_training_dataset_roundtrip_demo, set_validation_json_mode,
    validate_leserpent_control_plane_aot_evidence,
};

const TOP_LEVEL_COMMANDS: &[&str] = &[
    "container-operator-path-validation",
    "container-protocol-validation",
    "container-runtime-validation",
    "container-validation-summary",
    "debugger-cross",
    "external-engine-roundtrip",
    "field-smoke",
    "ftp-denied-container-validation",
    "help",
    "high-frequency",
    "ldap-bind-denied-container-validation",
    "leselang-fuzz",
    "leserpent-accessibility",
    "leserpent-aot",
    "leserpent-benchmark",
    "leserpent-parity-recovery",
    "leserpent-schema-freeze",
    "leserpent-transport",
    "juice-shop-container-validation",
    "linux-attach-smoke",
    "linux-kprobe-smoke",
    "linux-tc-smoke",
    "list",
    "package-install-smoke",
    "pathological-container-validation",
    "registry",
    "release-container-check",
    "release-gate",
    "remote-linux-host-validation",
    "resilience-bundle",
    "resilience-drive-bad-json",
    "resilience-emit-helper",
    "resilience-log-evidence",
    "resilience-roundtrip",
    "runtime-lifecycle",
    "runtime-operator",
    "socket-roundtrip",
    "three-module-stack-smoke",
    "training-roundtrip",
];
const JSON_SCHEMA_VERSION: u32 = 1;

fn listed_commands() -> Vec<&'static str> {
    let mut commands = TOP_LEVEL_COMMANDS
        .iter()
        .filter(|command| !matches!(**command, "help" | "list"))
        .chain(gewyvern_validate_stack::STACK_COMMANDS.iter())
        .copied()
        .collect::<Vec<_>>();
    commands.sort_unstable();
    commands
}

fn main() {
    let raw_args = env::args().skip(1).collect::<Vec<_>>();
    let (global_options, args) = parse_global_cli_options(raw_args);
    let json_enabled = global_options.json;
    let json_errors_enabled = global_options.json_errors;
    let failure_json_out = global_options.json_out.clone();
    set_validation_json_mode(global_options.json);

    if global_options.json_out_missing {
        eprintln!("validation failed: --json-out requires a path");
        process::exit(1);
    }

    if let Err(err) = run(args, global_options) {
        let message = err.to_string();
        if json_enabled || json_errors_enabled {
            print_failure_guidance_json(&message, failure_json_out.as_deref());
        } else {
            eprintln!("validation failed: {message}");
            print_failure_guidance(&message);
        }
        process::exit(1);
    }
}

fn run(args: Vec<String>, global_options: GlobalCliOptions) -> Result<(), ValidationError> {
    let mut args = args.into_iter();
    let command = args.next().unwrap_or_else(|| "help".to_string());
    let rest = args.collect::<Vec<_>>();

    if gewyvern_validate_stack::run_stack_command(
        &command,
        rest.clone(),
        global_options.json,
        global_options.json_out.as_deref(),
    )? {
        return Ok(());
    }

    match command.as_str() {
        "debugger-cross" => {
            let options = parse_options(rest)?;
            let report = run_debugger_cross_validation(options.out_dir)?;
            let extra = json!({
                "index": report.out_dir.join("evidence-index.json").display().to_string(),
            });
            print_validation_report(
                &command,
                &report,
                global_options.json,
                global_options.json_out.as_deref(),
                Some(extra),
            );
            Ok(())
        }
        "registry" => {
            let options = parse_options(rest)?;
            let report = run_registry_validation(options.out_dir, options.limit)?;
            print_validation_report(
                &command,
                &report,
                global_options.json,
                global_options.json_out.as_deref(),
                None,
            );
            Ok(())
        }
        "high-frequency" => {
            let options = parse_options(rest)?;
            let report = run_high_frequency_validation(options.out_dir)?;
            print_validation_report(
                &command,
                &report,
                global_options.json,
                global_options.json_out.as_deref(),
                None,
            );
            Ok(())
        }
        "juice-shop-container-validation" => {
            if wants_subcommand_help(&rest) {
                print_juice_shop_container_validation_help();
                return Ok(());
            }
            let out_dir = parse_optional_out_dir("juice-shop-container-validation", rest)?;
            let report = run_juice_shop_container_validation(out_dir)?;
            print_validation_report(
                &command,
                &report,
                global_options.json,
                global_options.json_out.as_deref(),
                None,
            );
            Ok(())
        }
        "field-smoke" => {
            let options = parse_options(rest)?;
            let report =
                run_field_smoke_validation(options.out_dir, options.socket, options.scan_all)?;
            print_validation_report(
                &command,
                &report,
                global_options.json,
                global_options.json_out.as_deref(),
                None,
            );
            Ok(())
        }
        "ftp-denied-container-validation" => {
            if wants_subcommand_help(&rest) {
                print_ftp_denied_container_validation_help();
                return Ok(());
            }
            let out_dir = parse_optional_out_dir("ftp-denied-container-validation", rest)?;
            let report = run_ftp_denied_container_validation(out_dir)?;
            let extra = json!({
                "index": report.out_dir.join("evidence-index.json").display().to_string(),
            });
            print_validation_report(
                &command,
                &report,
                global_options.json,
                global_options.json_out.as_deref(),
                Some(extra),
            );
            Ok(())
        }
        "ldap-bind-denied-container-validation" => {
            if wants_subcommand_help(&rest) {
                print_ldap_bind_denied_container_validation_help();
                return Ok(());
            }
            let out_dir = parse_optional_out_dir("ldap-bind-denied-container-validation", rest)?;
            let report = run_ldap_bind_denied_container_validation(out_dir)?;
            let extra = json!({
                "index": report.out_dir.join("evidence-index.json").display().to_string(),
            });
            print_validation_report(
                &command,
                &report,
                global_options.json,
                global_options.json_out.as_deref(),
                Some(extra),
            );
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
            print_validation_report(
                &command,
                &report,
                global_options.json,
                global_options.json_out.as_deref(),
                None,
            );
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
            print_validation_report(
                &command,
                &report,
                global_options.json,
                global_options.json_out.as_deref(),
                None,
            );
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
            print_validation_report(
                &command,
                &report,
                global_options.json,
                global_options.json_out.as_deref(),
                None,
            );
            Ok(())
        }
        "runtime-lifecycle" => {
            let options = parse_options(rest)?;
            let report = run_runtime_lifecycle_validation(options.out_dir)?;
            print_validation_report(
                &command,
                &report,
                global_options.json,
                global_options.json_out.as_deref(),
                None,
            );
            Ok(())
        }
        "resilience-log-evidence" => {
            let options = parse_options(rest)?;
            let log_source = require_path_option(options.log_source, "--log-source")?;
            let report = run_resilience_log_evidence_validation(log_source, options.out_dir)?;
            print_validation_report(
                &command,
                &report,
                global_options.json,
                global_options.json_out.as_deref(),
                None,
            );
            Ok(())
        }
        "resilience-roundtrip" => {
            let options = parse_options(rest)?;
            let report =
                run_resilience_roundtrip_validation(options.api_addr.as_deref(), options.out_dir)?;
            print_validation_report(
                &command,
                &report,
                global_options.json,
                global_options.json_out.as_deref(),
                None,
            );
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
            print_validation_report(
                &command,
                &report,
                global_options.json,
                global_options.json_out.as_deref(),
                None,
            );
            Ok(())
        }
        "resilience-emit-helper" => {
            let options = parse_options(rest)?;
            let mode = require_string_option(options.mode, "--mode")?;
            let output_path = require_path_option(options.output, "--output")?;
            let report = run_resilience_emit_helper_validation(&mode, output_path)?;
            print_validation_report(
                &command,
                &report,
                global_options.json,
                global_options.json_out.as_deref(),
                None,
            );
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
            print_validation_report(
                &command,
                &report,
                global_options.json,
                global_options.json_out.as_deref(),
                None,
            );
            Ok(())
        }
        "runtime-operator" => {
            let options = parse_options(rest)?;
            let report = run_runtime_operator_validation(options.out_dir, options.json_out)?;
            print_validation_report(
                &command,
                &report,
                global_options.json,
                global_options.json_out.as_deref(),
                None,
            );
            Ok(())
        }
        "leserpent-aot" => {
            if wants_subcommand_help(&rest) {
                print_leserpent_aot_help();
                return Ok(());
            }
            let options = parse_options(rest)?;
            let report = run_leserpent_aot_validation(options.out_dir)?;
            print_validation_report(
                &command,
                &report,
                global_options.json,
                global_options.json_out.as_deref(),
                None,
            );
            Ok(())
        }
        "leserpent-accessibility" => {
            if wants_subcommand_help(&rest) {
                print_leserpent_accessibility_help();
                return Ok(());
            }
            let options = parse_options(rest)?;
            let report = run_leserpent_accessibility_validation(options.out_dir)?;
            print_validation_report(
                &command,
                &report,
                global_options.json,
                global_options.json_out.as_deref(),
                None,
            );
            Ok(())
        }
        "leserpent-transport" => {
            if wants_subcommand_help(&rest) {
                print_leserpent_transport_help();
                return Ok(());
            }
            let options = parse_options(rest)?;
            let report = run_leserpent_transport_validation(options.out_dir)?;
            print_validation_report(
                &command,
                &report,
                global_options.json,
                global_options.json_out.as_deref(),
                None,
            );
            Ok(())
        }
        "leserpent-schema-freeze" => {
            if wants_subcommand_help(&rest) {
                print_leserpent_schema_freeze_help();
                return Ok(());
            }
            let options = parse_options(rest)?;
            let report = run_leserpent_schema_freeze_validation(options.out_dir)?;
            print_validation_report(
                &command,
                &report,
                global_options.json,
                global_options.json_out.as_deref(),
                None,
            );
            Ok(())
        }
        "leserpent-benchmark" => {
            if wants_subcommand_help(&rest) {
                print_leserpent_benchmark_help();
                return Ok(());
            }
            let options = parse_options(rest)?;
            let report = run_leserpent_benchmark_validation(options.out_dir)?;
            print_validation_report(
                &command,
                &report,
                global_options.json,
                global_options.json_out.as_deref(),
                None,
            );
            Ok(())
        }
        "leserpent-parity-recovery" => {
            if wants_subcommand_help(&rest) {
                print_leserpent_parity_recovery_help();
                return Ok(());
            }
            let options = parse_options(rest)?;
            let report = run_leserpent_parity_recovery_validation(options.out_dir)?;
            print_validation_report(
                &command,
                &report,
                global_options.json,
                global_options.json_out.as_deref(),
                None,
            );
            Ok(())
        }
        "leselang-fuzz" => {
            if wants_subcommand_help(&rest) {
                print_leselang_fuzz_help();
                return Ok(());
            }
            let options = parse_options(rest)?;
            let report = run_leselang_fuzz_validation(options.out_dir)?;
            print_validation_report(
                &command,
                &report,
                global_options.json,
                global_options.json_out.as_deref(),
                None,
            );
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
            print_validation_report(
                &command,
                &report,
                global_options.json,
                global_options.json_out.as_deref(),
                None,
            );
            Ok(())
        }
        "linux-kprobe-smoke" => {
            if wants_subcommand_help(&rest) {
                print_linux_kprobe_smoke_help();
                return Ok(());
            }
            let options = parse_linux_ebpf_smoke_options(rest, "ip_route_output_flow", "--symbol")?;
            let report = run_linux_kprobe_smoke(&options.target, options.out_dir)?;
            print_validation_report(
                &command,
                &report,
                global_options.json,
                global_options.json_out.as_deref(),
                None,
            );
            Ok(())
        }
        "linux-tc-smoke" => {
            if wants_subcommand_help(&rest) {
                print_linux_tc_smoke_help();
                return Ok(());
            }
            let options = parse_linux_ebpf_smoke_options(rest, "eth0", "--dev")?;
            let report = run_linux_tc_smoke(&options.target, options.out_dir)?;
            print_validation_report(
                &command,
                &report,
                global_options.json,
                global_options.json_out.as_deref(),
                None,
            );
            Ok(())
        }
        "release-container-check" => {
            if wants_subcommand_help(&rest) {
                print_release_container_check_help();
                return Ok(());
            }
            let mode = parse_release_check_mode("release-container-check", rest)?;
            let report = run_release_container_check(mode)?;
            let extra = release_container_check_summary_value(&report);
            print_validation_report(
                &command,
                &report,
                global_options.json,
                global_options.json_out.as_deref(),
                Some(extra),
            );
            Ok(())
        }
        "package-install-smoke" => {
            if wants_subcommand_help(&rest) {
                print_package_install_smoke_help();
                return Ok(());
            }
            let mode = parse_release_check_mode("package-install-smoke", rest)?;
            let report = run_package_install_smoke(mode)?;
            print_validation_report(
                &command,
                &report,
                global_options.json,
                global_options.json_out.as_deref(),
                None,
            );
            Ok(())
        }
        "container-protocol-validation" => {
            if wants_subcommand_help(&rest) {
                print_container_protocol_validation_help();
                return Ok(());
            }
            let mode = parse_release_check_mode("container-protocol-validation", rest)?;
            let report = run_container_protocol_validation(mode)?;
            print_validation_report(
                &command,
                &report,
                global_options.json,
                global_options.json_out.as_deref(),
                None,
            );
            Ok(())
        }
        "container-operator-path-validation" => {
            if wants_subcommand_help(&rest) {
                print_container_operator_path_validation_help();
                return Ok(());
            }
            let mode = parse_release_check_mode("container-operator-path-validation", rest)?;
            let report = run_container_operator_path_validation(mode)?;
            print_validation_report(
                &command,
                &report,
                global_options.json,
                global_options.json_out.as_deref(),
                None,
            );
            Ok(())
        }
        "container-validation-summary" => {
            if wants_subcommand_help(&rest) {
                print_container_validation_summary_help();
                return Ok(());
            }
            let mode = parse_release_check_mode("container-validation-summary", rest)?;
            let report = run_container_validation_summary(mode)?;
            print_validation_report(
                &command,
                &report,
                global_options.json,
                global_options.json_out.as_deref(),
                None,
            );
            Ok(())
        }
        "container-runtime-validation" => {
            if wants_subcommand_help(&rest) {
                print_container_runtime_validation_help();
                return Ok(());
            }
            let mode = parse_release_check_mode("container-runtime-validation", rest)?;
            let report = run_container_runtime_validation(mode)?;
            print_validation_report(
                &command,
                &report,
                global_options.json,
                global_options.json_out.as_deref(),
                None,
            );
            Ok(())
        }
        "remote-linux-host-validation" => {
            if wants_subcommand_help(&rest) {
                print_remote_linux_host_validation_help();
                return Ok(());
            }
            let options = parse_remote_linux_host_options(rest)?;
            let report = run_remote_linux_host_validation(options)?;
            let extra = remote_linux_host_summary_value(&report.out_dir)?;
            print_validation_report(
                &command,
                &report,
                global_options.json,
                global_options.json_out.as_deref(),
                Some(extra),
            );
            Ok(())
        }
        "release-gate" => {
            if wants_subcommand_help(&rest) {
                print_release_gate_help();
                return Ok(());
            }
            let options = parse_release_gate_options(rest)?;
            let report = run_release_gate(options)?;
            let extra = release_gate_summary_value(&report)?;
            print_validation_report(
                &command,
                &report,
                global_options.json,
                global_options.json_out.as_deref(),
                Some(extra),
            );
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
            print_validation_report(
                &command,
                &report,
                global_options.json,
                global_options.json_out.as_deref(),
                None,
            );
            Ok(())
        }
        "pathological-container-validation" => {
            if wants_subcommand_help(&rest) {
                print_pathological_container_validation_help();
                return Ok(());
            }
            let out_dir = parse_optional_out_dir("pathological-container-validation", rest)?;
            let report = run_pathological_container_validation(out_dir)?;
            print_validation_report(
                &command,
                &report,
                global_options.json,
                global_options.json_out.as_deref(),
                None,
            );
            Ok(())
        }
        "list" => {
            if global_options.json {
                emit_json_payload(
                    &json!({
                        "schema_version": JSON_SCHEMA_VERSION,
                        "ok": true,
                        "commands": listed_commands(),
                    }),
                    global_options.json_out.as_deref(),
                    false,
                );
            } else {
                for command in listed_commands() {
                    println!("{command}");
                }
            }
            Ok(())
        }
        "help" | "--help" | "-h" => {
            if global_options.json {
                print_help_json(global_options.json_out.as_deref());
            } else {
                print_help();
            }
            Ok(())
        }
        other => Err(unknown_command_error(other)),
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
    for arg in args {
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
            "--skip-debugger-cross" => options.run_debugger_cross = false,
            "--skip-pathology" => options.run_pathology = false,
            "--leserpent-proof" => options.run_leserpent_proof = true,
            "--macos-release-preflight" => {
                options.macos_release_preflight =
                    Some(PathBuf::from(iter.next().ok_or_else(|| {
                        ValidationError::new("--macos-release-preflight requires a value")
                    })?));
            }
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

fn parse_optional_out_dir(
    command_name: &str,
    args: Vec<String>,
) -> Result<Option<PathBuf>, ValidationError> {
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
                    "unknown {command_name} option `{other}`"
                )));
            }
            other => {
                if out_dir.is_some() {
                    return Err(ValidationError::new(format!(
                        "{command_name} accepts at most one output path"
                    )));
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
    println!("Global flags:");
    println!("  --json          Emit machine-readable JSON on success and failure when supported");
    println!("  --json-errors   Emit machine-readable JSON on failure");
    println!(
        "  --json-out <path>  Write the final JSON result to a file; place before the command"
    );
    println!();
    println!("Commands:");
    println!("  list");
    println!("  container-operator-path-validation [--deb|--rpm]");
    println!("  container-protocol-validation [--deb|--rpm]");
    println!("  container-runtime-validation [--deb|--rpm]");
    println!("  container-validation-summary [--deb|--rpm]");
    println!("  debugger-cross [--out-dir <path>]  # writes evidence-index.json");
    println!("  field-smoke [--out-dir <path>] [--socket] [--scan-all]");
    println!("  ftp-denied-container-validation [--out-dir <path>]");
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
    println!("  ldap-bind-denied-container-validation [--out-dir <path>]");
    println!("  leserpent-transport [--out-dir <path>]");
    println!("  leserpent-benchmark [--out-dir <path>]");
    println!("  leserpent-parity-recovery [--out-dir <path>]");
    println!("  leserpent-schema-freeze [--out-dir <path>]");
    println!("  juice-shop-container-validation [--out-dir <path>]");
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
        "  release-gate [--skip-build] [--skip-release-check] [--skip-stack] [--skip-pathology] [--leserpent-proof] [--deb|--rpm]"
    );
    println!("  runtime-lifecycle [--out-dir <path>]");
    println!("  runtime-operator [--out-dir <path>] [--json-out <path>]");
    println!("  three-module-stack-smoke");
    gewyvern_validate_stack::print_stack_help();
}

fn print_help_json(json_out: Option<&std::path::Path>) {
    emit_json_payload(
        &json!({
            "schema_version": JSON_SCHEMA_VERSION,
            "ok": true,
            "name": "gewyvern_validate",
            "summary": "Native validation harness for gewyvern release and debugger checks.",
            "global_flags": [
                {
                    "name": "--json",
                    "description": "Emit machine-readable JSON on success and failure when supported",
                },
                {
                    "name": "--json-errors",
                    "description": "Emit machine-readable JSON on failure",
                },
                {
                    "name": "--json-out",
                    "description": "Write the final JSON result to a file; place before the command to use the global form",
                }
            ],
            "commands": listed_commands(),
        }),
        json_out,
        false,
    );
}

fn print_validation_report(
    command: &str,
    report: &gewyvern::validation_harness::ValidationReport,
    json_output: bool,
    json_out: Option<&std::path::Path>,
    extra: Option<serde_json::Value>,
) {
    if json_output {
        let mut payload = json!({
            "schema_version": JSON_SCHEMA_VERSION,
            "ok": true,
            "command": command,
            "name": report.name,
            "checks": report.checks,
            "evidence_dir": report.out_dir.display().to_string(),
        });
        if let Some(extra) = extra {
            payload["extra"] = extra;
        }
        emit_json_payload(&payload, json_out, false);
        return;
    }

    println!("{}: ok", report.name);
    if command == "registry" {
        println!("checks: {}", report.checks.len());
    } else {
        println!("checks: {}", report.checks.join(", "));
    }
    println!("evidence: {}", report.out_dir.display());

    match command {
        "release-container-check" => print_release_container_check_summary(report),
        "remote-linux-host-validation" => {
            if let Some(extra) = extra.as_ref() {
                print_remote_linux_host_validation_summary(extra);
            }
        }
        _ => {
            if let Some(extra) = extra {
                print_report_extra_lines(command, &extra);
            }
        }
    }
}

fn print_report_extra_lines(command: &str, extra: &serde_json::Value) {
    match command {
        "debugger-cross" => {
            if let Some(index) = extra.get("index").and_then(|value| value.as_str()) {
                println!("index: {index}");
            }
        }
        "release-container-check" => {
            if let Some(mode) = extra.get("release_mode").and_then(|value| value.as_str()) {
                println!("release-mode: {mode}");
            }
            if let Some(covered) = extra
                .get("covered_checks")
                .and_then(|value| value.as_array())
                .map(|items| {
                    items
                        .iter()
                        .filter_map(|item| item.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                })
            {
                println!("covered-checks: {covered}");
            }
        }
        "remote-linux-host-validation" => {
            if let Some(summary) = extra.as_object() {
                for key in [
                    "remote_dir",
                    "source_cache",
                    "target_cache",
                    "build_packages",
                    "remote_ebpf",
                    "slowest_phases",
                ] {
                    if let Some(value) = summary.get(key).and_then(|value| value.as_str()) {
                        println!("{}: {value}", key.replace('_', "-"));
                    }
                }
            }
        }
        "release-gate" => {
            if let Some(stages) =
                extra
                    .get("stages")
                    .and_then(|value| value.as_object())
                    .map(|items| {
                        items
                            .iter()
                            .map(|(name, ran)| format!("{name}={}", ran.as_bool().unwrap_or(false)))
                            .collect::<Vec<_>>()
                            .join(", ")
                    })
            {
                println!("stages: {stages}");
            }
            if let Some(remote) = extra.get("remote").and_then(|value| value.as_object()) {
                if let Some(remote_dir) = remote.get("remote_dir").and_then(|value| value.as_str())
                {
                    println!("remote-dir: {remote_dir}");
                }
                if let Some(remote_ebpf) =
                    remote.get("remote_ebpf").and_then(|value| value.as_str())
                {
                    println!("remote-ebpf: {remote_ebpf}");
                }
                if let Some(slowest) = remote
                    .get("slowest_phases")
                    .and_then(|value| value.as_str())
                {
                    println!("slowest-phases: {slowest}");
                }
            }
        }
        _ => {}
    }
}

fn unknown_command_error(command: &str) -> ValidationError {
    let mut message =
        format!("unknown validation command `{command}`; try `gewyvern_validate list`");
    if let Some(suggested) = suggest_command(command) {
        message.push_str(&format!("; did you mean `{suggested}`?"));
    }
    ValidationError::new(message)
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct GlobalCliOptions {
    json: bool,
    json_errors: bool,
    json_out: Option<PathBuf>,
    json_out_missing: bool,
}

fn parse_global_cli_options(args: Vec<String>) -> (GlobalCliOptions, Vec<String>) {
    let mut options = GlobalCliOptions::default();
    let mut filtered = Vec::with_capacity(args.len());
    let mut iter = args.into_iter().peekable();

    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--json" => {
                options.json = true;
                options.json_errors = true;
            }
            "--json-errors" => options.json_errors = true,
            "--json-out" => {
                if let Some(value) = iter.next() {
                    options.json_out = Some(PathBuf::from(value));
                } else {
                    options.json_out_missing = true;
                }
            }
            _ if arg.starts_with('-') => filtered.push(arg),
            _ => {
                filtered.push(arg);
                filtered.extend(iter);
                break;
            }
        }
    }

    (options, filtered)
}

fn suggest_command(input: &str) -> Option<&'static str> {
    let mut best = None;

    for command in TOP_LEVEL_COMMANDS
        .iter()
        .chain(gewyvern_validate_stack::STACK_COMMANDS.iter())
    {
        let distance = levenshtein_distance(input, command);
        let max_distance = if command.len() <= 8 { 2 } else { 4 };
        if distance > max_distance {
            continue;
        }

        match best {
            Some((best_distance, _)) if distance >= best_distance => {}
            _ => best = Some((distance, *command)),
        }
    }

    best.map(|(_, command)| command)
}

fn levenshtein_distance(left: &str, right: &str) -> usize {
    let right_chars = right.chars().collect::<Vec<_>>();
    let mut previous = (0..=right_chars.len()).collect::<Vec<_>>();
    let mut current = vec![0; right_chars.len() + 1];

    for (left_index, left_char) in left.chars().enumerate() {
        current[0] = left_index + 1;
        for (right_index, right_char) in right_chars.iter().enumerate() {
            let substitution_cost = usize::from(left_char != *right_char);
            current[right_index + 1] = (previous[right_index + 1] + 1)
                .min(current[right_index] + 1)
                .min(previous[right_index] + substitution_cost);
        }
        previous.clone_from(&current);
    }

    previous[right_chars.len()]
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FailureClass {
    Input,
    Environment,
    Privilege,
    Remote,
    Dependency,
    Artifact,
    Timeout,
}

impl FailureClass {
    fn code(self) -> &'static str {
        match self {
            Self::Input => "input",
            Self::Environment => "environment",
            Self::Privilege => "privilege",
            Self::Remote => "remote",
            Self::Dependency => "dependency",
            Self::Artifact => "artifact",
            Self::Timeout => "timeout",
        }
    }
}

fn classify_failure(message: &str) -> Option<(FailureClass, &'static str)> {
    if message.contains("requires a path")
        || message.contains("requires a value")
        || message.contains("requires a number")
        || message.contains("is required")
        || message.contains("invalid --limit value")
        || message.contains("invalid --port value")
        || message.contains("invalid --count value")
        || message.contains("unknown validation option `")
        || message.contains("unknown release-gate option `")
        || message.contains("unknown remote-linux-host-validation option `")
        || message.contains("unknown linux smoke option `")
        || message.contains("unknown pathological-container-validation option `")
    {
        return Some((FailureClass::Input, "invalid_cli_input"));
    }
    if message.contains("docker daemon is not reachable")
        || message.contains("failed to query docker")
    {
        return Some((FailureClass::Environment, "docker_unreachable"));
    }
    if message.contains("no .deb artifact found under ")
        || message.contains("no .rpm artifact found under ")
    {
        return Some((FailureClass::Artifact, "missing_package_artifact"));
    }
    if message.contains("container validation timed out after ")
        || message.contains("timed out waiting for ")
        || message.contains("did not exit in time")
    {
        return Some((FailureClass::Timeout, "validation_timeout"));
    }
    if message.contains("remote workspace retained at ") {
        return Some((FailureClass::Remote, "remote_workspace_retained"));
    }
    if message.contains("remote host must be Linux, got `") {
        return Some((FailureClass::Remote, "remote_host_not_linux"));
    }
    if message.contains("remote host must be x86_64/amd64 for packaged validation, got `") {
        return Some((FailureClass::Remote, "remote_host_wrong_arch"));
    }
    if message.contains(
        "GEWY_REMOTE_EBPF_ADMIN_USER is set but GEWY_REMOTE_EBPF_ADMIN_PASSWORD is missing",
    ) || message.contains(
        "GEWY_REMOTE_EBPF_ADMIN_PASSWORD is set but GEWY_REMOTE_EBPF_ADMIN_USER is missing",
    ) {
        return Some((FailureClass::Remote, "remote_admin_credentials_incomplete"));
    }
    if message.contains("Operation not permitted")
        || message
            .contains("linux eBPF smoke requires Linux kernel support and BPF attach privileges")
        || message.contains("linux eBPF smoke requires a Linux environment")
    {
        return Some((FailureClass::Privilege, "linux_ebpf_privilege_required"));
    }
    if message.contains("required command not found: sshpass") {
        return Some((FailureClass::Dependency, "missing_sshpass"));
    }
    if message.contains("required command not found: rsync")
        || message.contains("required command not found: ssh")
        || message.contains("required command not found: docker")
    {
        return Some((FailureClass::Dependency, "missing_system_command"));
    }
    if message.contains("failed to query dotnet SDK")
        || message.contains("install xvfb and xauth on the Linux host")
    {
        return Some((FailureClass::Dependency, "missing_native_aot_dependency"));
    }
    None
}

fn print_failure_guidance(message: &str) {
    if let Some((class, code)) = classify_failure(message) {
        eprintln!("failure-class: {}", class.code());
        eprintln!("failure-code: {code}");
    }

    for line in failure_guidance_lines(message) {
        eprintln!("{line}");
    }
}

fn print_failure_guidance_json(message: &str, json_out: Option<&std::path::Path>) {
    let classified = classify_failure(message);
    let next_steps = failure_guidance_lines(message);
    let payload = json!({
        "schema_version": JSON_SCHEMA_VERSION,
        "ok": false,
        "message": message,
        "failure_class": classified.map(|(class, _)| class.code()),
        "failure_code": classified.map(|(_, code)| code),
        "next_steps": next_steps,
    });
    emit_json_payload(&payload, json_out, true);
}

fn emit_json_payload(
    payload: &serde_json::Value,
    json_out: Option<&std::path::Path>,
    stderr: bool,
) {
    let rendered = payload.to_string();
    if let Some(path) = json_out {
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let _ = fs::write(path, rendered.as_bytes());
    }
    if stderr {
        eprintln!("{rendered}");
    } else {
        println!("{rendered}");
    }
}

fn failure_guidance_lines(message: &str) -> Vec<&'static str> {
    let mut guidance = Vec::new();

    if message.contains("requires a path")
        || message.contains("requires a value")
        || message.contains("requires a number")
        || message.contains("is required")
        || message.contains("invalid --limit value")
        || message.contains("invalid --port value")
        || message.contains("invalid --count value")
        || message.contains("unknown validation option `")
        || message.contains("unknown release-gate option `")
        || message.contains("unknown remote-linux-host-validation option `")
        || message.contains("unknown linux smoke option `")
        || message.contains("unknown pathological-container-validation option `")
    {
        guidance.push(
            "next-step: rerun with `gewyvern_validate help` or the subcommand `--help` output and correct the missing or invalid CLI option",
        );
    }

    if message.contains("docker daemon is not reachable")
        || message.contains("failed to query docker")
    {
        guidance.push(
            "next-step: start Docker Desktop or another local daemon, then retry `gewyvern_validate release-container-check` or the narrower packaged command that failed",
        );
    }

    if message.contains("remote workspace retained at ") {
        guidance.push(
            "next-step: SSH into the remote host, inspect the retained workspace, or rerun with `--keep-remote-dir` if you want the directory preserved on purpose",
        );
    }

    if message.contains("no .deb artifact found under ")
        || message.contains("no .rpm artifact found under ")
    {
        guidance.push(
            "next-step: rebuild native packages first, for example `bash scripts/packaging/build_packages_in_container.sh --format all`, then rerun the packaged validation entrypoint",
        );
    }

    if message.contains("container validation timed out after ")
        || message.contains("timed out waiting for ")
        || message.contains("did not exit in time")
    {
        guidance.push(
            "next-step: inspect the retained evidence or logs, then rerun the narrower validation command to isolate whether startup, HTTP readiness, package install, or remote attach is hanging",
        );
    }

    if message.contains("remote host must be Linux, got `")
        || message.contains("remote host must be x86_64/amd64 for packaged validation, got `")
    {
        guidance.push(
            "next-step: rerun against a Linux x86_64 host, or disable the remote-host stage while narrowing local packaged validation first",
        );
    }

    if message.contains(
        "GEWY_REMOTE_EBPF_ADMIN_USER is set but GEWY_REMOTE_EBPF_ADMIN_PASSWORD is missing",
    ) || message.contains(
        "GEWY_REMOTE_EBPF_ADMIN_PASSWORD is set but GEWY_REMOTE_EBPF_ADMIN_USER is missing",
    ) {
        guidance.push(
            "next-step: set both `GEWY_REMOTE_EBPF_ADMIN_USER` and `GEWY_REMOTE_EBPF_ADMIN_PASSWORD`, or unset both to skip the admin-assisted remote eBPF path",
        );
    }

    if message.contains("Operation not permitted")
        || message
            .contains("linux eBPF smoke requires Linux kernel support and BPF attach privileges")
        || message.contains("linux eBPF smoke requires a Linux environment")
    {
        guidance.push(
            "next-step: rerun on Linux with sudo or equivalent BPF privileges, for example `sudo cargo run --quiet --bin gewyvern_validate -- linux-attach-smoke`",
        );
    }

    if message.contains("required command not found: sshpass") {
        guidance.push(
            "next-step: install `sshpass`, or unset `GEWY_REMOTE_EBPF_ADMIN_USER` / `GEWY_REMOTE_EBPF_ADMIN_PASSWORD` if you want to skip the admin-assisted remote eBPF path",
        );
    }

    if message.contains("required command not found: rsync")
        || message.contains("required command not found: ssh")
        || message.contains("required command not found: docker")
    {
        guidance.push(
            "next-step: install the missing system command and rerun the same validation entrypoint",
        );
    }
    if message.contains("failed to query dotnet SDK") {
        guidance.push(
            "next-step: install the locked project's .NET SDK, then rerun `gewyvern_validate leserpent-aot` on the same host",
        );
    }
    if message.contains("install xvfb and xauth on the Linux host") {
        guidance.push(
            "next-step: install `xvfb` and `xauth` on the Linux host, then rerun `gewyvern_validate leserpent-aot` without sudo",
        );
    }
    guidance
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

fn print_release_container_check_summary(report: &gewyvern::validation_harness::ValidationReport) {
    let summary = release_container_check_summary_value(report);
    print_report_extra_lines("release-container-check", &summary);
}

fn release_container_check_summary_value(
    report: &gewyvern::validation_harness::ValidationReport,
) -> serde_json::Value {
    let mode = report
        .name
        .split('(')
        .nth(1)
        .and_then(|rest| rest.strip_suffix(')'))
        .unwrap_or("unknown");

    let covered = report
        .checks
        .iter()
        .map(|check| match check.as_str() {
            "package_install_smoke" => "package-install-smoke",
            "packaged_runtime_validation" => "container-runtime-validation",
            "packaged_protocol_operator_summary" => "container-validation-summary",
            other => other,
        })
        .collect::<Vec<_>>();
    json!({
        "release_mode": mode,
        "covered_checks": covered,
    })
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

fn print_leserpent_aot_help() {
    println!("Usage: gewyvern_validate leserpent-aot [--out-dir <path>]");
    println!();
    println!(
        "Restore the locked Avalonia RID graph, publish NativeAOT for the current host, and run all control fixtures."
    );
    println!("Supported hosts: macOS arm64 and Linux x86_64. Linux requires xvfb-run and xauth.");
}

fn print_leserpent_accessibility_help() {
    println!("Usage: gewyvern_validate leserpent-accessibility [--out-dir <path>]");
    println!();
    println!(
        "Build the managed Avalonia shell and audit real controls across all fixtures for Automation metadata and WCAG AA text contrast."
    );
    println!("Supported hosts: macOS arm64 and Linux x86_64; Linux requires xvfb-run and xauth.");
}

fn print_leselang_fuzz_help() {
    println!("Usage: gewyvern_validate leselang-fuzz [--out-dir <path>]");
    println!();
    println!(
        "Run the deterministic UTF-8 parser/HIR/VM and continuation decoder fuzz shelves with retained evidence."
    );
    println!("The fixed seed replays 2048 source cases and 2048 continuation mutations.");
}

fn print_leserpent_transport_help() {
    println!("Usage: gewyvern_validate leserpent-transport [--out-dir <path>]");
    println!();
    println!(
        "Prove wire-v1 compatibility, CLI/Leselang parity, and authenticated local Unix IPC security with retained evidence."
    );
    println!(
        "Windows named pipes and authenticated HTTPS/WebSocket remain explicit future transport boundaries."
    );
}

fn print_leserpent_schema_freeze_help() {
    println!("Usage: gewyvern_validate leserpent-schema-freeze [--out-dir <path>]");
    println!();
    println!(
        "Validate the bounded v1 command, query, effect, UI, and wire inventory and run its fixed proof registry."
    );
    println!(
        "Candidate evidence does not claim a final freeze until every Gate 7 release criterion is reproducible."
    );
}

fn print_leserpent_benchmark_help() {
    println!("Usage: gewyvern_validate leserpent-benchmark [--out-dir <path>]");
    println!();
    println!(
        "Measure bounded runtime, UI IR, and release-binary workloads and enforce disaster-regression budgets."
    );
    println!("Timing comparisons are valid only within the same host class.");
}

fn print_leserpent_parity_recovery_help() {
    println!("Usage: gewyvern_validate leserpent-parity-recovery [--out-dir <path>]");
    println!();
    println!(
        "Prove current command-origin parity, authorization, idempotency, and injected VM/runtime recovery paths."
    );
    println!("Every suite must execute its minimum nonzero test count and retain a transcript.");
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
    println!(
        "The command prints a compact post-run summary including resolved remote dir, cache-backed phases, eBPF status, and the slowest observed timings."
    );
}

fn print_remote_linux_host_validation_summary(summary: &serde_json::Value) {
    if let Some(kernel) = summary
        .get("preflight")
        .and_then(|value| value.get("kernel"))
        .and_then(|value| value.as_str())
    {
        println!("kernel: {kernel}");
    }
    if let Some(default_route_device) = summary
        .get("ebpf")
        .and_then(|value| value.get("default_route_device"))
        .and_then(|value| value.as_str())
    {
        println!("default-route-device: {default_route_device}");
    }
    if let Some(remote_dir) = summary.get("remote_dir").and_then(|value| value.as_str()) {
        println!("remote-dir: {remote_dir}");
    }
    if let Some(source_cache) = summary.get("source_cache").and_then(|value| value.as_str()) {
        println!("source-cache: {source_cache}");
    }
    if let Some(target_cache) = summary.get("target_cache").and_then(|value| value.as_str()) {
        println!("target-cache: {target_cache}");
    }
    if let Some(build_packages) = summary
        .get("build_packages")
        .and_then(|value| value.as_str())
    {
        println!("build-packages: {build_packages}");
    }
    if let Some(remote_ebpf) = summary.get("remote_ebpf").and_then(|value| value.as_str()) {
        println!("remote-ebpf: {remote_ebpf}");
    }
    if let Some(remediation) = summary
        .get("remote_ebpf_remediation")
        .and_then(|value| value.as_str())
    {
        println!("remote-ebpf-remediation: {remediation}");
    }
    if let Some(validation_posture) = summary
        .get("validation_posture")
        .and_then(|value| value.as_str())
    {
        println!("validation-posture: {validation_posture}");
    }
    if let Some(release_gate_signal) = summary
        .get("release_gate_signal")
        .and_then(|value| value.as_str())
    {
        println!("release-gate-signal: {release_gate_signal}");
    }
    if let Some(next_step) = summary.get("next_step").and_then(|value| value.as_str()) {
        println!("next-step: {next_step}");
    }
    if let Some(linux_proof_complete) = summary
        .get("linux_proof_complete")
        .and_then(|value| value.as_bool())
    {
        println!("linux-proof-complete: {linux_proof_complete}");
    }
    if let Some(requires_followup) = summary
        .get("requires_followup")
        .and_then(|value| value.as_bool())
    {
        println!("requires-followup: {requires_followup}");
    }
    if let Some(slowest_phases) = summary
        .get("slowest_phases")
        .and_then(|value| value.as_str())
    {
        println!("slowest-phases: {slowest_phases}");
    }
    if let Some(total_seconds) = summary
        .get("total_seconds")
        .and_then(|value| value.as_f64())
    {
        println!("total-seconds: {total_seconds:.3}");
    }
    if let Some(warnings) = summary
        .get("budget_warnings")
        .and_then(|value| value.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str())
                .collect::<Vec<_>>()
                .join(" | ")
        })
        .filter(|text| !text.is_empty())
    {
        println!("budget-warnings: {warnings}");
    }
    if let Some(trend) = summary
        .get("recent_ebpf_trend")
        .and_then(|value| value.as_str())
    {
        println!("recent-ebpf-trend: {trend}");
    }
    if let Some(lines) = summary
        .get("recent_ebpf_lines")
        .and_then(|value| value.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str())
                .take(3)
                .collect::<Vec<_>>()
        })
        .filter(|items| !items.is_empty())
    {
        for line in lines {
            println!("recent-ebpf: {line}");
        }
    }
}

fn remote_linux_host_summary_value(
    out_dir: &std::path::Path,
) -> Result<serde_json::Value, ValidationError> {
    let run = parse_evidence_key_value_file(
        &out_dir.join("remote-run.txt"),
        "remote run evidence",
        &[
            "host",
            "remote_dir",
            "build_packages",
            "keep_remote_dir",
            "checks",
        ],
    )?;
    let preflight = parse_evidence_key_value_file(
        &out_dir.join("remote-preflight.txt"),
        "remote preflight evidence",
        &[
            "os",
            "arch",
            "kernel",
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
    let ebpf = parse_evidence_key_value_file(
        &out_dir.join("remote-ebpf.txt"),
        "remote eBPF evidence",
        &["status", "reason", "default_route_device"],
    )?;
    let timings = parse_phase_timings(
        &out_dir.join("remote-phase-timings.txt"),
        "remote phase timings",
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
            "remote_ebpf_evidence_sync",
            "remote_workspace_cleanup",
            "total",
        ],
        &["total"],
    )?;
    let build_packages_enabled =
        parse_required_bool(&run, "build_packages", "remote run evidence")?;
    let aot_evidence_covered = run.get("checks").is_some_and(|checks| {
        checks
            .split(',')
            .any(|check| check == "remote_leserpent_control_plane_aot")
    });
    if aot_evidence_covered {
        validate_leserpent_control_plane_aot_evidence(
            &out_dir.join("leserpent-control-plane-aot-linux-x64"),
        )?;
    }
    let package_build_timings = if build_packages_enabled {
        parse_phase_timings(
            &out_dir.join("remote-package-build-timings.txt"),
            "remote package build timings",
            &["release_build", "stage_layout", "package_all", "total"],
            &["release_build", "stage_layout", "package_all", "total"],
        )?
    } else {
        Vec::new()
    };
    let package_smoke_timings = parse_phase_timings(
        &out_dir.join("remote-package-smoke-timings.txt"),
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
    )?;
    let runtime_smoke_timings = parse_phase_timings(
        &out_dir.join("remote-runtime-smoke-timings.txt"),
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
    )?;
    let mut summary = serde_json::Map::new();

    if let Some(remote_dir) = run.get("remote_dir") {
        summary.insert("remote_dir".to_string(), json!(remote_dir));
    }
    if let Some(home_dir) = preflight.get("home_dir") {
        summary.insert(
            "source_cache".to_string(),
            json!(format!("{home_dir}/.cache/gewyvern/remote-source")),
        );
        summary.insert(
            "target_cache".to_string(),
            json!(format!("{home_dir}/.cache/gewyvern/remote-target")),
        );
    }
    if let Some(build_packages) = run.get("build_packages") {
        summary.insert("build_packages".to_string(), json!(build_packages));
    }
    summary.insert(
        "build_packages_enabled".to_string(),
        json!(build_packages_enabled),
    );
    if aot_evidence_covered {
        summary.insert(
            "leserpent_control_plane_aot_evidence_validated".to_string(),
            json!(true),
        );
    }
    summary.insert(
        "keep_remote_dir".to_string(),
        json!(parse_required_bool(
            &run,
            "keep_remote_dir",
            "remote run evidence"
        )?),
    );
    if let Some(checks) = run.get("checks") {
        summary.insert(
            "remote_checks".to_string(),
            json!(
                checks
                    .split(',')
                    .filter(|item| !item.is_empty())
                    .collect::<Vec<_>>()
            ),
        );
    }
    if !preflight.is_empty() {
        summary.insert(
            "preflight".to_string(),
            json!({
                "os": preflight.get("os"),
                "arch": preflight.get("arch"),
                "kernel": preflight.get("kernel"),
                "host_fingerprint": preflight.get("host_fingerprint"),
                "home_dir": preflight.get("home_dir"),
                "commands": preflight
                    .get("commands")
                    .map(|value| value.split(',').filter(|item| !item.is_empty()).collect::<Vec<_>>()),
                "rustc_version": preflight.get("rustc_version"),
                "cargo_version": preflight.get("cargo_version"),
                "dpkg_deb_version": preflight.get("dpkg_deb_version"),
                "rpm_version": preflight.get("rpm_version"),
                "rpmbuild_version": preflight.get("rpmbuild_version"),
                "sudo_available": parse_bool_string(preflight.get("sudo_available")),
                "ebpf_helper_available": parse_bool_string(preflight.get("ebpf_helper_available")),
                "ebpf_helper_state": preflight.get("ebpf_helper_state"),
                "ebpf_helper_version": preflight.get("ebpf_helper_version"),
                "default_route_device": preflight.get("default_route_device"),
            }),
        );
    }
    if let Some(status) = ebpf.get("status") {
        let reason = ebpf.get("reason").map(String::as_str).unwrap_or("unknown");
        summary.insert(
            "remote_ebpf".to_string(),
            json!(format!("{status} ({reason})")),
        );
        if let Some(remediation) = remote_ebpf_remediation(reason) {
            summary.insert("remote_ebpf_remediation".to_string(), json!(remediation));
        }
    }
    if !ebpf.is_empty() {
        summary.insert(
            "ebpf".to_string(),
            json!({
                "status": ebpf.get("status"),
                "reason": ebpf.get("reason"),
                "default_route_device": ebpf.get("default_route_device"),
            }),
        );
    }
    let history_summary = parse_bounded_json_file(
        &out_dir.join("remote-ebpf-status-summary.json"),
        "remote eBPF status summary",
    )?;
    {
        if let Some(entries) = history_summary
            .get("entries")
            .and_then(|value| value.as_u64())
        {
            summary.insert("remote_ebpf_history_entries".to_string(), json!(entries));
        }
        if let Some(status_counts) = history_summary.get("status_counts") {
            summary.insert(
                "remote_ebpf_status_counts".to_string(),
                status_counts.clone(),
            );
        }
        if let Some(reason_counts) = history_summary.get("reason_counts") {
            summary.insert(
                "remote_ebpf_reason_counts".to_string(),
                reason_counts.clone(),
            );
        }
        if let Some(integrity) = history_summary.get("integrity") {
            summary.insert(
                "remote_ebpf_history_integrity".to_string(),
                integrity.clone(),
            );
        }
        if let Some(matrix) = history_summary.get("matrix") {
            summary.insert("remote_ebpf_matrix".to_string(), matrix.clone());
        }
        if let Some(trend) = summarize_recent_ebpf_trend(&history_summary) {
            summary.insert("recent_ebpf_trend".to_string(), json!(trend));
        }
    }
    let recent_lines = read_bounded_recent_lines(
        &out_dir.join("remote-ebpf-recent.txt"),
        "remote eBPF recent evidence",
    )?;
    if !recent_lines.is_empty() {
        summary.insert("recent_ebpf_lines".to_string(), json!(recent_lines));
    }

    if !timings.is_empty() {
        let phase_timings = timings
            .iter()
            .map(|(name, seconds)| (name.clone(), json!(seconds)))
            .collect::<serde_json::Map<String, serde_json::Value>>();
        summary.insert(
            "phase_timings".to_string(),
            serde_json::Value::Object(phase_timings),
        );
    }
    if !package_build_timings.is_empty() {
        let package_phase_timings = package_build_timings
            .iter()
            .map(|(name, seconds)| (name.clone(), json!(seconds)))
            .collect::<serde_json::Map<String, serde_json::Value>>();
        summary.insert(
            "package_build_timings".to_string(),
            serde_json::Value::Object(package_phase_timings),
        );
    }
    if !package_smoke_timings.is_empty() {
        let package_smoke_phase_timings = package_smoke_timings
            .iter()
            .map(|(name, seconds)| (name.clone(), json!(seconds)))
            .collect::<serde_json::Map<String, serde_json::Value>>();
        summary.insert(
            "package_smoke_timings".to_string(),
            serde_json::Value::Object(package_smoke_phase_timings),
        );
    }
    if !runtime_smoke_timings.is_empty() {
        let runtime_phase_timings = runtime_smoke_timings
            .iter()
            .map(|(name, seconds)| (name.clone(), json!(seconds)))
            .collect::<serde_json::Map<String, serde_json::Value>>();
        summary.insert(
            "runtime_smoke_timings".to_string(),
            serde_json::Value::Object(runtime_phase_timings),
        );
    }
    if let Some((_, total_seconds)) = timings.iter().find(|(name, _)| name == "total") {
        summary.insert("total_seconds".to_string(), json!(total_seconds));
    }

    let budget_warnings = remote_phase_budget_warnings(&timings);

    let mut slowest = timings
        .iter()
        .map(|(name, seconds)| (name.clone(), *seconds))
        .filter(|(name, _)| name != "total")
        .collect::<Vec<_>>();
    slowest.sort_by(|left, right| right.1.total_cmp(&left.1));
    if !slowest.is_empty() {
        let slowest_summary = slowest
            .iter()
            .take(3)
            .map(|(name, seconds)| format!("{name}={seconds:.3}s"))
            .collect::<Vec<_>>()
            .join(", ");
        summary.insert("slowest_phases".to_string(), json!(slowest_summary));
        summary.insert(
            "slowest_phase_entries".to_string(),
            json!(
                slowest
                    .iter()
                    .take(3)
                    .map(|(name, seconds)| json!({
                        "name": name,
                        "seconds": seconds,
                    }))
                    .collect::<Vec<_>>()
            ),
        );
    }
    if !budget_warnings.is_empty() {
        summary.insert("budget_warnings".to_string(), json!(budget_warnings));
    }
    let (validation_posture, release_gate_signal, next_step) =
        summarize_remote_validation_posture(&ebpf, &summary);
    let linux_proof_complete = validation_posture == "full";
    let requires_followup = release_gate_signal != "ready";
    summary.insert("validation_posture".to_string(), json!(validation_posture));
    summary.insert(
        "release_gate_signal".to_string(),
        json!(release_gate_signal),
    );
    summary.insert("next_step".to_string(), json!(next_step));
    summary.insert(
        "linux_proof_complete".to_string(),
        json!(linux_proof_complete),
    );
    summary.insert("requires_followup".to_string(), json!(requires_followup));
    Ok(serde_json::Value::Object(summary))
}

fn remote_ebpf_remediation(reason: &str) -> Option<&'static str> {
    match reason {
        "privileged_helper_missing" => Some(
            "install the packaged helper and provision its root-owned config and command-limited sudoers rule",
        ),
        "privileged_helper_unavailable" => Some(
            "verify helper ownership, /etc/gewyvern/ebpf-helper.conf, and the command-limited sudoers rule",
        ),
        "privileged_helper_incompatible" => {
            Some("replace the installed helper with the current Gewyvern package version")
        }
        _ => None,
    }
}

fn remote_phase_budget_warnings(timings: &[(String, f64)]) -> Vec<String> {
    const REMOTE_TOTAL_BUDGET_SECONDS: f64 = 180.0;
    const WORKSPACE_SYNC_BUDGET_SECONDS: f64 = 8.0;
    const REMOTE_PACKAGE_BUILD_BUDGET_SECONDS: f64 = 20.0;
    const REMOTE_LESERPENT_CONTROL_PLANE_AOT_BUDGET_SECONDS: f64 = 120.0;
    const REMOTE_PACKAGE_SMOKE_BUDGET_SECONDS: f64 = 2.0;
    const REMOTE_RUNTIME_SMOKE_BUDGET_SECONDS: f64 = 3.0;
    const REMOTE_EBPF_VALIDATOR_BUILD_BUDGET_SECONDS: f64 = 20.0;
    const REMOTE_EBPF_ATTACH_BUDGET_SECONDS: f64 = 5.0;
    const LEGACY_REMOTE_EBPF_SMOKE_BUDGET_SECONDS: f64 = 10.0;
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
                "remote_ebpf_validator_build" => Some(REMOTE_EBPF_VALIDATOR_BUILD_BUDGET_SECONDS),
                "remote_ebpf_attach" => Some(REMOTE_EBPF_ATTACH_BUDGET_SECONDS),
                "remote_ebpf_smoke" => Some(LEGACY_REMOTE_EBPF_SMOKE_BUDGET_SECONDS),
                "remote_ebpf_evidence_sync" => Some(REMOTE_EBPF_SYNC_BUDGET_SECONDS),
                _ => None,
            }?;
            (seconds > &budget)
                .then(|| format!("{name} exceeded budget ({seconds:.3}s > {budget:.3}s)"))
        })
        .collect()
}

fn release_gate_summary_value(
    report: &gewyvern::validation_harness::ValidationReport,
) -> Result<serde_json::Value, ValidationError> {
    let checks = &report.checks;
    let remote_out_dir = report.out_dir.join("remote-linux-host-validation");
    let remote_ran = checks
        .iter()
        .any(|check| check == "remote_linux_host_validation");
    let remote = if remote_ran && remote_out_dir.is_dir() {
        remote_linux_host_summary_value(&remote_out_dir)?
    } else {
        serde_json::Value::Null
    };
    let (gate_posture, ship_signal, next_step) =
        summarize_release_gate_posture(checks, remote.as_object());

    Ok(json!({
        "stages": {
            "build_packages": checks.iter().any(|check| check == "build_packages_in_container"),
            "release_container_check": checks.iter().any(|check| check == "release_container_check"),
            "three_module_stack_smoke": checks.iter().any(|check| check == "three_module_stack_smoke"),
            "debugger_cross_validation": checks.iter().any(|check| check == "debugger_cross_validation"),
            "pathological_container_validation": checks.iter().any(|check| check == "pathological_container_validation"),
            "leserpent_parity_recovery": checks.iter().any(|check| check == "leserpent_parity_recovery"),
            "leserpent_schema_freeze": checks.iter().any(|check| check == "leserpent_schema_freeze"),
            "macos_release_preflight": checks.iter().any(|check| check == "macos_release_preflight_ready" || check == "macos_release_preflight_blocked"),
            "macos_release_preflight_ready": checks.iter().any(|check| check == "macos_release_preflight_ready"),
            "macos_release_preflight_blocked": checks.iter().any(|check| check == "macos_release_preflight_blocked"),
            "remote_linux_host_validation": remote_ran,
        },
        "remote": remote,
        "gate_posture": gate_posture,
        "ship_signal": ship_signal,
        "next_step": next_step
    }))
}

fn summarize_release_gate_posture(
    checks: &[String],
    remote: Option<&serde_json::Map<String, serde_json::Value>>,
) -> (&'static str, &'static str, &'static str) {
    let packaged_ready = checks
        .iter()
        .any(|check| check == "release_container_check");
    let stack_ready = checks
        .iter()
        .any(|check| check == "three_module_stack_smoke");
    let debugger_ready = checks
        .iter()
        .any(|check| check == "debugger_cross_validation");
    let pathology_ready = checks
        .iter()
        .any(|check| check == "pathological_container_validation");
    let remote_ran = checks
        .iter()
        .any(|check| check == "remote_linux_host_validation");
    let remote_full = checks.iter().any(|check| check == "remote_ebpf_smoke");
    let remote_partial = checks
        .iter()
        .any(|check| check == "remote_ebpf_smoke_skipped");
    let remote_signal = remote
        .and_then(|remote| remote.get("release_gate_signal"))
        .and_then(|value| value.as_str())
        .unwrap_or("unknown");
    let remote_budget_watch = remote_signal == "watch"
        && remote
            .and_then(|remote| remote.get("budget_warnings"))
            .and_then(|value| value.as_array())
            .is_some_and(|warnings| !warnings.is_empty());
    let remote_requires_followup = remote_signal != "ready"
        || remote
            .and_then(|remote| remote.get("requires_followup"))
            .and_then(|value| value.as_bool())
            .unwrap_or(false);
    let macos_preflight_blocked = checks
        .iter()
        .any(|check| check == "macos_release_preflight_blocked");

    if packaged_ready
        && stack_ready
        && debugger_ready
        && pathology_ready
        && remote_full
        && remote_signal == "ready"
        && macos_preflight_blocked
    {
        (
            "blocked_external",
            "apple_credentials_blocked",
            "the product and Linux gates passed, but the macOS preflight reports missing or invalid Apple release credentials; provision Developer ID and the notary Keychain profile before ship",
        )
    } else if packaged_ready
        && stack_ready
        && debugger_ready
        && pathology_ready
        && remote_full
        && remote_budget_watch
    {
        (
            "watch",
            "timing_watch",
            "remote Linux proof passed, but warned phases exceeded the current soft budget; inspect the timing drift before treating this run as the freshest release reference",
        )
    } else if packaged_ready
        && stack_ready
        && debugger_ready
        && pathology_ready
        && remote_full
        && remote_requires_followup
    {
        (
            "partial",
            "followup_required",
            "the current Linux host attach proof passed, but its remote release signal still requires follow-up; inspect coverage, integrity, and timing evidence before ship",
        )
    } else if packaged_ready
        && stack_ready
        && debugger_ready
        && pathology_ready
        && remote_full
        && remote_signal == "ready"
    {
        (
            "full",
            "ready",
            "hold this release-gate run as the current 1.0 candidate reference and watch later regressions against it",
        )
    } else if packaged_ready && stack_ready && debugger_ready && pathology_ready && remote_partial {
        (
            "partial",
            "followup_required",
            "package and runtime confidence passed, but Linux attach proof is still partial; rerun the remote stage with full eBPF privilege before ship",
        )
    } else if packaged_ready && stack_ready && debugger_ready && pathology_ready && !remote_ran {
        (
            "local_only",
            "remote_missing",
            "the local release gate passed, but remote Linux host evidence did not run; execute --remote-host-validation before treating this as a 1.0 candidate",
        )
    } else if let Some(remote) = remote {
        if remote
            .get("requires_followup")
            .and_then(|value| value.as_bool())
            .unwrap_or(false)
        {
            (
                "partial",
                "followup_required",
                "inspect the remote Linux summary and clear the reported follow-up item before treating this gate as ship-ready",
            )
        } else {
            (
                "incomplete",
                "needs_review",
                "inspect the missing release-gate stages and rerun the skipped validation shelves before ship",
            )
        }
    } else {
        (
            "incomplete",
            "needs_review",
            "inspect the missing release-gate stages and rerun the skipped validation shelves before ship",
        )
    }
}

fn parse_evidence_key_value_file(
    path: &std::path::Path,
    context: &str,
    allowed_keys: &[&str],
) -> Result<BTreeMap<String, String>, ValidationError> {
    read_bounded_unique_key_value_file(path, context, allowed_keys)
}

fn parse_phase_timings(
    path: &std::path::Path,
    context: &str,
    allowed_keys: &[&str],
    required_keys: &[&str],
) -> Result<Vec<(String, f64)>, ValidationError> {
    read_bounded_phase_timings(path, context, allowed_keys, required_keys)
}

fn parse_required_bool(
    values: &BTreeMap<String, String>,
    key: &str,
    context: &str,
) -> Result<bool, ValidationError> {
    match values.get(key).map(String::as_str) {
        Some("true") => Ok(true),
        Some("false") => Ok(false),
        Some(_) => Err(ValidationError::new(format!(
            "{context} {key} must be true or false"
        ))),
        None => Err(ValidationError::new(format!("{context} missing {key}"))),
    }
}

fn parse_bounded_json_file(
    path: &std::path::Path,
    context: &str,
) -> Result<serde_json::Value, ValidationError> {
    read_bounded_json_file(path, context, 64 * 1024)
}

fn read_bounded_recent_lines(
    path: &std::path::Path,
    context: &str,
) -> Result<Vec<String>, ValidationError> {
    read_bounded_nonempty_lines(path, context, 16 * 1024, 5, 512)
}

fn summarize_recent_ebpf_trend(history_summary: &serde_json::Value) -> Option<String> {
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

fn summarize_remote_validation_posture(
    ebpf: &BTreeMap<String, String>,
    summary: &serde_json::Map<String, serde_json::Value>,
) -> (&'static str, &'static str, &'static str) {
    let has_budget_warnings = summary
        .get("budget_warnings")
        .and_then(|value| value.as_array())
        .is_some_and(|items| !items.is_empty());
    let has_history_integrity_warning = summary
        .get("remote_ebpf_history_integrity")
        .and_then(|value| value.get("status"))
        .and_then(|value| value.as_str())
        .is_some_and(|status| status != "clean");
    let has_matrix_coverage_gap = summary
        .get("remote_ebpf_matrix")
        .and_then(|value| value.get("ready"))
        .and_then(|value| value.as_bool())
        == Some(false);
    match ebpf.get("status").map(String::as_str) {
        Some("ok") if has_history_integrity_warning => (
            "full",
            "watch",
            "inspect remote-ebpf-history-rejected.jsonl before treating this Linux history as a clean release reference",
        ),
        Some("ok") if has_budget_warnings => (
            "full",
            "watch",
            "inspect the warned remote phases before treating this Linux host result as the current release reference",
        ),
        Some("ok") if has_matrix_coverage_gap => (
            "full",
            "coverage_incomplete",
            "collect successful evidence from at least two physical hosts and two kernel releases before treating the Linux matrix as release-ready",
        ),
        Some("ok") => (
            "full",
            "ready",
            "hold this run as the current Linux hot-path reference and watch future remote regressions against it",
        ),
        Some("skipped") => match ebpf.get("reason").map(String::as_str) {
            Some("sudo_not_available") => (
                "partial",
                "package_runtime_only",
                "rerun with sudo or GEWY_REMOTE_EBPF_ADMIN_USER / GEWY_REMOTE_EBPF_ADMIN_PASSWORD to prove native Linux attach confidence",
            ),
            Some("default_route_device_not_detected") => (
                "partial",
                "route_device_missing",
                "rerun on a host with a detectable default-route device so the tc smoke can prove attach confidence",
            ),
            _ => (
                "partial",
                "incomplete_linux_evidence",
                "inspect the remote eBPF reason and rerun once the missing Linux privilege or routing prerequisite is available",
            ),
        },
        _ => (
            "unknown",
            "needs_review",
            "inspect the remote validation evidence directory before treating this run as a release reference",
        ),
    }
}

fn parse_bool_string(value: Option<&String>) -> Option<bool> {
    match value.map(String::as_str) {
        Some("true") => Some(true),
        Some("false") => Some(false),
        _ => None,
    }
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
        "Usage: gewyvern_validate release-gate [--skip-build] [--skip-release-check] [--skip-stack] [--skip-debugger-cross] [--skip-pathology] [--leserpent-proof] [--macos-release-preflight <file>] [--remote-host-validation] [--remote-host <ssh-host>] [--remote-dir <path>] [--skip-remote-build] [--keep-remote-dir] [--deb|--rpm]"
    );
    println!();
    println!("Run the current release gate as one deliberate sequence:");
    println!("1. rebuild fresh native packages in Docker");
    println!("2. run the packaged release validation wrapper");
    println!("3. run the three-module stack smoke");
    println!("4. run debugger cross validation");
    println!("5. run pathological container/runtime-ingest validation");
    println!("6. optionally run the Leserpent parity/recovery and schema/scope freeze proofs");
    println!("7. optionally consume strict macOS Apple release preflight evidence");
    println!("8. optionally run remote Linux host validation over SSH");
    println!();
    println!("Flags:");
    println!("  --skip-build          Reuse current package artifacts instead of rebuilding");
    println!("  --skip-release-check  Skip packaged DEB/RPM validation");
    println!("  --skip-stack          Skip three-module stack smoke");
    println!("  --skip-debugger-cross Skip debugger cross validation");
    println!("  --skip-pathology      Skip pathological runtime-ingest validation");
    println!(
        "  --leserpent-proof     Run the opt-in Leserpent parity/recovery and schema/scope freeze shelves"
    );
    println!(
        "  --macos-release-preflight  Validate and index a machine-readable Apple release preflight report"
    );
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

fn print_ftp_denied_container_validation_help() {
    println!("Usage: gewyvern_validate ftp-denied-container-validation [--out-dir <path>]");
    println!();
    println!(
        "Run a Linux-only practical lab check against an FTP server that rejects bad credentials, then preserve client-side 530 evidence, target-side FAIL LOGIN logs, and same-host attach/kprobe/tc smoke evidence."
    );
}

fn print_ldap_bind_denied_container_validation_help() {
    println!("Usage: gewyvern_validate ldap-bind-denied-container-validation [--out-dir <path>]");
    println!();
    println!(
        "Run a Linux-only practical lab check against an LDAP server that rejects a bad bind, then preserve client-side err=49 evidence, target-side bind logs, and same-host attach/kprobe/tc smoke evidence."
    );
}

fn print_juice_shop_container_validation_help() {
    println!("Usage: gewyvern_validate juice-shop-container-validation [--out-dir <path>]");
    println!();
    println!(
        "Run a Linux-only practical lab check against an OWASP Juice Shop container, then preserve target-side anomaly evidence next to native attach/kprobe/tc smoke evidence."
    );
    println!(
        "This validates suspicious target behavior and same-host Linux attach capability; it does not claim direct vulnerability classification by gewyvern itself."
    );
}

fn print_pathological_container_validation_help() {
    println!("Usage: gewyvern_validate pathological-container-validation [--out-dir <path>]");
    println!();
    println!("Run the pathological container/runtime-ingest validation suite.");
    println!(
        "A single positional output path is also accepted for compatibility with the legacy shell entrypoint."
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn read_fixture(relative: &str) -> serde_json::Value {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative);
        let contents = fs::read_to_string(&path).unwrap_or_else(|err| {
            panic!("failed to read {}: {}", path.display(), err);
        });
        serde_json::from_str(&contents).unwrap_or_else(|err| {
            panic!("failed to parse fixture {}: {}", path.display(), err);
        })
    }

    struct TempDirGuard {
        path: PathBuf,
    }

    impl TempDirGuard {
        fn new(prefix: &str) -> Self {
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos();
            let path = std::env::temp_dir().join(format!("{prefix}-{unique}"));
            fs::create_dir_all(&path).unwrap_or_else(|err| {
                panic!("failed to create temp dir {}: {}", path.display(), err);
            });
            Self { path }
        }
    }

    impl Drop for TempDirGuard {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn remote_linux_host_summary_value_matches_fixture_shape() {
        let temp = TempDirGuard::new("gewyvern-remote-summary-shape");

        fs::write(
            temp.path.join("remote-run.txt"),
            "remote_dir=/tmp/gewyvern-remote\nbuild_packages=true\nkeep_remote_dir=false\nchecks=remote_preflight,remote_artifacts_present,remote_ebpf_smoke,remote_ebpf_evidence_synced\n",
        )
        .unwrap();
        fs::write(
            temp.path.join("remote-preflight.txt"),
            "os=linux\narch=x86_64\nkernel=6.8.0-test\nhost_fingerprint=sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\nhome_dir=/home/demo\ncommands=cargo,docker,sshpass\nrustc_version=rustc 1.95.0\ncargo_version=cargo 1.95.0\ndpkg_deb_version=dpkg-deb 1.22.6\nrpm_version=RPM version 4.18.2\nrpmbuild_version=RPM version 4.18.2\nsudo_available=true\nebpf_helper_available=true\nebpf_helper_state=ready\nebpf_helper_version=1.5.0\ndefault_route_device=eth0\n",
        )
        .unwrap();
        fs::write(
            temp.path.join("remote-ebpf.txt"),
            "status=ok\nreason=all_smokes_passed_admin_ssh\ndefault_route_device=eth0\n",
        )
        .unwrap();
        fs::write(
            temp.path.join("remote-ebpf-status-summary.json"),
            r#"{"entries":2,"integrity":{"status":"clean","valid_entries":2,"rejected_entries":0,"rejected_entries_this_run":0},"status_counts":{"ok":2},"reason_counts":{"all_smokes_passed_admin_ssh":2},"matrix":{"ready":false,"minimum_hosts":2,"minimum_kernels":2,"unique_hosts":1,"unique_kernels":1,"unique_architectures":1,"unidentified_successful_runs":0,"successful_host_counts":{"sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa":2},"successful_kernel_counts":{"6.8.0-test":2},"successful_arch_counts":{"x86_64":2}}}"#,
        )
        .unwrap();
        fs::write(
            temp.path.join("remote-ebpf-recent.txt"),
            "2026-07-12T09:00:00Z ok all_smokes_passed_admin_ssh 4.000\n2026-07-11T09:00:00Z ok all_smokes_passed_admin_ssh 4.200\n",
        )
        .unwrap();
        fs::write(
            temp.path.join("remote-phase-timings.txt"),
            "workspace_sync=1.6\nremote_package_build=12.8\nremote_package_smoke=1.1\nremote_runtime_smoke=2.4\nremote_ebpf_validator_build=3.2\nremote_ebpf_attach=0.8\nremote_ebpf_evidence_sync=0.9\ntotal=22.8\n",
        )
        .unwrap();
        fs::write(
            temp.path.join("remote-package-build-timings.txt"),
            "release_build=12.5\nstage_layout=0.5\npackage_all=1.25\ntotal=14.25\n",
        )
        .unwrap();
        fs::write(
            temp.path.join("remote-package-smoke-timings.txt"),
            "deb_list_contents=0.1\ndeb_verify=0.4\nrpm_list_contents=0.2\nrpm_verify=0.6\ntotal=1.3\n",
        )
        .unwrap();
        fs::write(
            temp.path.join("remote-runtime-smoke-timings.txt"),
            "tcp_boot_health=0.7\nudp_boot_health=0.6\ntcp_summary=0.4\nudp_summary=0.3\nudp_analysis=0.5\ntcp_health_after_bad=0.2\ntcp_analysis=0.4\ntotal=3.1\n",
        )
        .unwrap();

        let summary = remote_linux_host_summary_value(&temp.path).unwrap();
        let expected =
            read_fixture("docs/fixtures/gewyvern_validate_remote_linux_host_summary.json");
        assert_eq!(summary, expected);
    }

    #[test]
    fn remote_ebpf_helper_failures_have_specific_remediation() {
        assert!(
            remote_ebpf_remediation("privileged_helper_missing")
                .unwrap()
                .contains("install")
        );
        assert!(
            remote_ebpf_remediation("privileged_helper_unavailable")
                .unwrap()
                .contains("sudoers")
        );
        assert!(
            remote_ebpf_remediation("privileged_helper_incompatible")
                .unwrap()
                .contains("current Gewyvern package version")
        );
        assert!(remote_ebpf_remediation("all_smokes_passed_privileged_helper").is_none());
    }

    #[test]
    fn local_evidence_reader_rejects_ambiguous_unknown_and_unbounded_files() {
        let temp = TempDirGuard::new("gewyvern-local-evidence-codec");
        let path = temp.path.join("evidence.txt");

        for body in ["status=ok\nstatus=bad\n", "unknown=value\n", "malformed\n"] {
            fs::write(&path, body).unwrap();
            assert!(
                parse_evidence_key_value_file(&path, "test evidence", &["status"]).is_err(),
                "{body}"
            );
        }
        fs::write(&path, "x".repeat(8 * 1024 + 1)).unwrap();
        assert!(parse_evidence_key_value_file(&path, "test evidence", &["status"]).is_err());
    }

    #[test]
    fn local_summary_reader_rejects_malformed_and_unbounded_json() {
        let temp = TempDirGuard::new("gewyvern-local-summary-json");
        let path = temp.path.join("summary.json");

        fs::write(&path, "not-json").unwrap();
        assert!(parse_bounded_json_file(&path, "test summary").is_err());

        fs::write(
            &path,
            format!("{{\"padding\":\"{}\"}}", "x".repeat(64 * 1024)),
        )
        .unwrap();
        assert!(parse_bounded_json_file(&path, "test summary").is_err());
    }

    #[test]
    fn local_recent_reader_enforces_line_contract() {
        let temp = TempDirGuard::new("gewyvern-local-recent-lines");
        let path = temp.path.join("recent.txt");

        fs::write(&path, "one\ntwo\nthree\nfour\nfive\nsix\n").unwrap();
        assert!(read_bounded_recent_lines(&path, "test recent evidence").is_err());

        fs::write(&path, format!("{}\n", "x".repeat(513))).unwrap();
        assert!(read_bounded_recent_lines(&path, "test recent evidence").is_err());

        fs::write(&path, "valid\ninvalid\u{0007}\n").unwrap();
        assert!(read_bounded_recent_lines(&path, "test recent evidence").is_err());
    }

    #[test]
    fn release_gate_propagates_incomplete_remote_coverage() {
        let checks = [
            "release_container_check",
            "three_module_stack_smoke",
            "debugger_cross_validation",
            "pathological_container_validation",
            "remote_linux_host_validation",
            "remote_ebpf_smoke",
        ]
        .into_iter()
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
        let remote = serde_json::json!({
            "release_gate_signal": "coverage_incomplete",
            "requires_followup": true,
        });

        let (posture, signal, next_step) =
            summarize_release_gate_posture(&checks, remote.as_object());
        assert_eq!(posture, "partial");
        assert_eq!(signal, "followup_required");
        assert!(next_step.contains("coverage"));
    }

    #[test]
    fn release_gate_leserpent_proof_is_explicit_opt_in() {
        let defaults = parse_release_gate_options(Vec::new()).unwrap();
        assert!(!defaults.run_leserpent_proof);
        assert!(defaults.macos_release_preflight.is_none());

        let selected = parse_release_gate_options(vec!["--leserpent-proof".to_string()]).unwrap();
        assert!(selected.run_leserpent_proof);

        let selected = parse_release_gate_options(vec![
            "--macos-release-preflight".to_string(),
            "preflight.json".to_string(),
        ])
        .unwrap();
        assert_eq!(
            selected.macos_release_preflight,
            Some(PathBuf::from("preflight.json"))
        );
        assert!(parse_release_gate_options(vec!["--macos-release-preflight".to_string()]).is_err());
    }

    #[test]
    fn release_gate_surfaces_valid_apple_credential_blockers() {
        let checks = [
            "release_container_check",
            "three_module_stack_smoke",
            "debugger_cross_validation",
            "pathological_container_validation",
            "remote_linux_host_validation",
            "remote_ebpf_smoke",
            "macos_release_preflight_blocked",
        ]
        .into_iter()
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
        let remote = serde_json::json!({
            "release_gate_signal": "ready",
            "requires_followup": false,
        });

        let (posture, signal, next_step) =
            summarize_release_gate_posture(&checks, remote.as_object());
        assert_eq!(posture, "blocked_external");
        assert_eq!(signal, "apple_credentials_blocked");
        assert!(next_step.contains("Developer ID"));
    }

    #[test]
    fn classify_failure_covers_release_facing_error_shapes() {
        assert_eq!(
            classify_failure("docker daemon is not reachable"),
            Some((FailureClass::Environment, "docker_unreachable"))
        );
        assert_eq!(
            classify_failure("timed out waiting for LDAP bind readiness on 127.0.0.1:389"),
            Some((FailureClass::Timeout, "validation_timeout"))
        );
        assert_eq!(
            classify_failure(
                "remote host must be x86_64/amd64 for packaged validation, got `arm64`"
            ),
            Some((FailureClass::Remote, "remote_host_wrong_arch"))
        );
        assert_eq!(
            classify_failure(
                "GEWY_REMOTE_EBPF_ADMIN_USER is set but GEWY_REMOTE_EBPF_ADMIN_PASSWORD is missing",
            ),
            Some((FailureClass::Remote, "remote_admin_credentials_incomplete"))
        );
        assert_eq!(
            classify_failure("required command not found: docker"),
            Some((FailureClass::Dependency, "missing_system_command"))
        );
        assert_eq!(
            classify_failure("linux eBPF smoke requires a Linux environment"),
            Some((FailureClass::Privilege, "linux_ebpf_privilege_required"))
        );
    }

    #[test]
    fn failure_guidance_lines_match_release_facing_error_shapes() {
        assert_eq!(
            failure_guidance_lines("docker daemon is not reachable"),
            vec![
                "next-step: start Docker Desktop or another local daemon, then retry `gewyvern_validate release-container-check` or the narrower packaged command that failed",
            ]
        );
        assert_eq!(
            failure_guidance_lines("required command not found: docker"),
            vec![
                "next-step: install the missing system command and rerun the same validation entrypoint",
            ]
        );
        assert_eq!(
            failure_guidance_lines(
                "GEWY_REMOTE_EBPF_ADMIN_PASSWORD is set but GEWY_REMOTE_EBPF_ADMIN_USER is missing",
            ),
            vec![
                "next-step: set both `GEWY_REMOTE_EBPF_ADMIN_USER` and `GEWY_REMOTE_EBPF_ADMIN_PASSWORD`, or unset both to skip the admin-assisted remote eBPF path",
            ]
        );
        assert_eq!(
            failure_guidance_lines(
                "remote host must be x86_64/amd64 for packaged validation, got `arm64`",
            ),
            vec![
                "next-step: rerun against a Linux x86_64 host, or disable the remote-host stage while narrowing local packaged validation first",
            ]
        );
    }
}
