use std::env;
use std::path::PathBuf;
use std::process;

#[path = "../validation_harness_cli_stack.rs"]
mod gewyvern_validate_stack;

use gewyvern::validation_harness::{
    ValidationError, run_debugger_cross_validation, run_external_engine_roundtrip_demo,
    run_field_smoke_validation, run_high_frequency_validation, run_registry_validation,
    run_resilience_bundle_validation, run_resilience_drive_bad_json_validation,
    run_resilience_emit_helper_validation, run_resilience_log_evidence_validation,
    run_resilience_roundtrip_validation, run_runtime_lifecycle_validation,
    run_runtime_operator_validation, run_socket_roundtrip_demo,
    run_training_dataset_roundtrip_demo,
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
        "list" => {
            println!("debugger-cross");
            println!("external-engine-roundtrip");
            println!("field-smoke");
            println!("high-frequency");
            println!("registry");
            println!("resilience-bundle");
            println!("resilience-drive-bad-json");
            println!("resilience-emit-helper");
            println!("resilience-log-evidence");
            println!("resilience-roundtrip");
            println!("runtime-lifecycle");
            println!("runtime-operator");
            println!("socket-roundtrip");
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

fn env_flag(name: &str) -> bool {
    matches!(env::var(name).as_deref(), Ok("1") | Ok("true") | Ok("yes"))
}

fn print_help() {
    println!("gewyvern_validate");
    println!();
    println!("Native validation harness for gewyvern release and debugger checks.");
    println!();
    println!("Commands:");
    println!("  list");
    println!("  debugger-cross [--out-dir <path>]");
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
    println!("  registry [--out-dir <path>] [--limit <n>]");
    println!("  resilience-log-evidence --log-source <path> [--out-dir <path>]");
    println!("  resilience-roundtrip [--api-addr <addr>] [--out-dir <path>]");
    println!("  resilience-bundle --log-source <path> [--api-addr <addr>] [--out-dir <path>]");
    println!("  resilience-emit-helper --mode <timeout|fail|healthy> --output <path>");
    println!(
        "  resilience-drive-bad-json --host <host> --port <port> [--count <n>] [--out-dir <path>]"
    );
    println!("  runtime-lifecycle [--out-dir <path>]");
    println!("  runtime-operator [--out-dir <path>] [--json-out <path>]");
    gewyvern_validate_stack::print_stack_help();
}
