use std::path::PathBuf;

use gewyvern::validation_harness::{
    ValidationError, run_stack_json_file_validation, run_stack_probe_validation,
    run_stack_register_runtime_json, write_stack_resilience_summary,
};

pub const STACK_COMMANDS: &[&str] = &[
    "stack-check-json",
    "stack-probe",
    "stack-register-runtime-json",
    "stack-resilience-summary",
];

pub fn run_stack_command(command: &str, args: Vec<String>) -> Result<bool, ValidationError> {
    match command {
        "stack-check-json" => run_check_json(args),
        "stack-probe" => run_probe(args),
        "stack-register-runtime-json" => run_register_runtime(args),
        "stack-resilience-summary" => run_resilience_summary(args),
        _ => Ok(false),
    }
}

pub fn print_stack_list() {
    for command in STACK_COMMANDS {
        println!("{command}");
    }
}

pub fn print_stack_help() {
    println!("  stack-check-json --input <json> --profile <name>");
    println!(
        "  stack-probe --url <http-url> --profile <name> [--admin-token <token>] [--output <path>]"
    );
    println!(
        "  stack-register-runtime-json --name <name> --endpoint <url> --environment <env> --cluster <cluster> --role <role> [--sidecar-endpoint <url>] [--sidecar-admin-token <token>]"
    );
    println!(
        "  stack-resilience-summary --healthy-a <json> --healthy-b <json> --degraded-b <json> --output <path>"
    );
}

fn run_probe(args: Vec<String>) -> Result<bool, ValidationError> {
    let options = StackOptions::parse(args)?;
    let report = run_stack_probe_validation(
        &required(options.url, "--url")?,
        &required(options.profile, "--profile")?,
        options.admin_token.as_deref(),
        options.output,
    )?;
    print_report(&report.name, &report.checks, &report.out_dir);
    Ok(true)
}

fn run_register_runtime(args: Vec<String>) -> Result<bool, ValidationError> {
    let options = StackOptions::parse(args)?;
    println!(
        "{}",
        run_stack_register_runtime_json(
            &required(options.name, "--name")?,
            &required(options.endpoint, "--endpoint")?,
            &required(options.environment, "--environment")?,
            &required(options.cluster, "--cluster")?,
            &required(options.role, "--role")?,
            options.sidecar_endpoint.as_deref(),
            options.sidecar_admin_token.as_deref(),
        )?
    );
    Ok(true)
}

fn run_check_json(args: Vec<String>) -> Result<bool, ValidationError> {
    let options = StackOptions::parse(args)?;
    let report = run_stack_json_file_validation(
        &required_path(options.input, "--input")?,
        &required(options.profile, "--profile")?,
    )?;
    print_report(&report.name, &report.checks, &report.out_dir);
    Ok(true)
}

fn run_resilience_summary(args: Vec<String>) -> Result<bool, ValidationError> {
    let options = StackOptions::parse(args)?;
    let report = write_stack_resilience_summary(
        &required_path(options.healthy_a, "--healthy-a")?,
        &required_path(options.healthy_b, "--healthy-b")?,
        &required_path(options.degraded_b, "--degraded-b")?,
        &required_path(options.output, "--output")?,
    )?;
    print_report(&report.name, &report.checks, &report.out_dir);
    Ok(true)
}

struct StackOptions {
    url: Option<String>,
    profile: Option<String>,
    admin_token: Option<String>,
    name: Option<String>,
    endpoint: Option<String>,
    environment: Option<String>,
    cluster: Option<String>,
    role: Option<String>,
    sidecar_endpoint: Option<String>,
    sidecar_admin_token: Option<String>,
    input: Option<PathBuf>,
    output: Option<PathBuf>,
    healthy_a: Option<PathBuf>,
    healthy_b: Option<PathBuf>,
    degraded_b: Option<PathBuf>,
}

impl StackOptions {
    fn parse(args: Vec<String>) -> Result<Self, ValidationError> {
        let mut options = Self::empty();
        let mut iter = args.into_iter();
        while let Some(arg) = iter.next() {
            match arg.as_str() {
                "--url" => options.url = Some(next_value(&mut iter, "--url")?),
                "--profile" => options.profile = Some(next_value(&mut iter, "--profile")?),
                "--admin-token" => {
                    options.admin_token = Some(next_value(&mut iter, "--admin-token")?)
                }
                "--name" => options.name = Some(next_value(&mut iter, "--name")?),
                "--endpoint" => options.endpoint = Some(next_value(&mut iter, "--endpoint")?),
                "--environment" => {
                    options.environment = Some(next_value(&mut iter, "--environment")?)
                }
                "--cluster" => options.cluster = Some(next_value(&mut iter, "--cluster")?),
                "--role" => options.role = Some(next_value(&mut iter, "--role")?),
                "--sidecar-endpoint" => {
                    options.sidecar_endpoint = Some(next_value(&mut iter, "--sidecar-endpoint")?)
                }
                "--sidecar-admin-token" => {
                    options.sidecar_admin_token =
                        Some(next_value(&mut iter, "--sidecar-admin-token")?)
                }
                "--output" => {
                    options.output = Some(PathBuf::from(next_value(&mut iter, "--output")?))
                }
                "--input" => options.input = Some(PathBuf::from(next_value(&mut iter, "--input")?)),
                "--healthy-a" => {
                    options.healthy_a = Some(PathBuf::from(next_value(&mut iter, "--healthy-a")?))
                }
                "--healthy-b" => {
                    options.healthy_b = Some(PathBuf::from(next_value(&mut iter, "--healthy-b")?))
                }
                "--degraded-b" => {
                    options.degraded_b = Some(PathBuf::from(next_value(&mut iter, "--degraded-b")?))
                }
                other => {
                    return Err(ValidationError::new(format!(
                        "unknown stack option `{other}`"
                    )));
                }
            }
        }
        Ok(options)
    }

    fn empty() -> Self {
        Self {
            url: None,
            profile: None,
            admin_token: None,
            name: None,
            endpoint: None,
            environment: None,
            cluster: None,
            role: None,
            sidecar_endpoint: None,
            sidecar_admin_token: None,
            input: None,
            output: None,
            healthy_a: None,
            healthy_b: None,
            degraded_b: None,
        }
    }
}

fn next_value(
    iter: &mut impl Iterator<Item = String>,
    name: &str,
) -> Result<String, ValidationError> {
    iter.next()
        .ok_or_else(|| ValidationError::new(format!("{name} requires a value")))
}

fn required(value: Option<String>, name: &str) -> Result<String, ValidationError> {
    value.ok_or_else(|| ValidationError::new(format!("{name} is required")))
}

fn required_path(value: Option<PathBuf>, name: &str) -> Result<PathBuf, ValidationError> {
    value.ok_or_else(|| ValidationError::new(format!("{name} is required")))
}

fn print_report(name: &str, checks: &[String], out_dir: &std::path::Path) {
    println!("{name}: ok");
    println!("checks: {}", checks.join(", "));
    println!("evidence: {}", out_dir.display());
}
