use std::env;
use std::path::PathBuf;
use std::process;

use gewyvern::validation_harness::{
    ValidationError, run_debugger_cross_validation, run_high_frequency_validation,
    run_registry_validation, run_runtime_lifecycle_validation,
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

    match command.as_str() {
        "debugger-cross" => {
            let options = parse_options(args.collect())?;
            let report = run_debugger_cross_validation(options.out_dir)?;

            println!("{}: ok", report.name);
            println!("checks: {}", report.checks.join(", "));
            println!("evidence: {}", report.out_dir.display());
            Ok(())
        }
        "registry" => {
            let options = parse_options(args.collect())?;
            let report = run_registry_validation(options.out_dir, options.limit)?;

            println!("{}: ok", report.name);
            println!("checks: {}", report.checks.len());
            println!("evidence: {}", report.out_dir.display());
            Ok(())
        }
        "high-frequency" => {
            let options = parse_options(args.collect())?;
            let report = run_high_frequency_validation(options.out_dir)?;

            println!("{}: ok", report.name);
            println!("checks: {}", report.checks.join(", "));
            println!("evidence: {}", report.out_dir.display());
            Ok(())
        }
        "runtime-lifecycle" => {
            let options = parse_options(args.collect())?;
            let report = run_runtime_lifecycle_validation(options.out_dir)?;

            println!("{}: ok", report.name);
            println!("checks: {}", report.checks.join(", "));
            println!("evidence: {}", report.out_dir.display());
            Ok(())
        }
        "list" => {
            println!("debugger-cross");
            println!("high-frequency");
            println!("registry");
            println!("runtime-lifecycle");
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
}

fn parse_options(args: Vec<String>) -> Result<Options, ValidationError> {
    let mut out_dir = None;
    let mut limit = None;
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
            other => {
                return Err(ValidationError::new(format!(
                    "unknown validation option `{other}`"
                )));
            }
        }
    }

    Ok(Options { out_dir, limit })
}

fn print_help() {
    println!("gewyvern_validate");
    println!();
    println!("Native validation harness for gewyvern release and debugger checks.");
    println!();
    println!("Commands:");
    println!("  list");
    println!("  debugger-cross [--out-dir <path>]");
    println!("  high-frequency [--out-dir <path>]");
    println!("  registry [--out-dir <path>] [--limit <n>]");
    println!("  runtime-lifecycle [--out-dir <path>]");
}
