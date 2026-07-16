use std::path::PathBuf;
use std::time::Duration;
use std::{io, io::Write};

use leserpent_cli::{
    CliCommand, CliError, RuntimeWatchOptions, export_leselang, export_plan, parse_args,
    render_response, request_for, send_request,
};
use leserpent_domain::QueryResult;
use leserpent_protocol::{ProtocolResponse, RequestEnvelope};

fn main() {
    match run() {
        Ok(exit_code) => std::process::exit(exit_code),
        Err(CliError::Usage(message)) if message == leserpent_cli::USAGE => {
            println!("{message}");
        }
        Err(error) => {
            eprintln!("leserpent: {error}");
            std::process::exit(2);
        }
    }
}

fn run() -> Result<i32, CliError> {
    let options = parse_args(
        std::env::args().skip(1),
        std::env::var_os("LESERPENT_SOCKET").map(PathBuf::from),
        std::env::var("LESERPENT_PRINCIPAL").ok(),
    )?;
    if let Some(source) = export_leselang(&options) {
        println!("{source}");
        return Ok(0);
    }
    if let Some(plan) = export_plan(&options)? {
        println!("{plan}");
        return Ok(0);
    }
    let socket = options
        .socket
        .as_deref()
        .ok_or_else(|| CliError::Configuration("daemon socket is required for execution".into()))?;
    let token = std::env::var("LESERPENT_IPC_TOKEN")
        .map_err(|_| CliError::Configuration("LESERPENT_IPC_TOKEN is required".into()))?;
    let request = request_for(&options)?;
    if let CliCommand::RuntimeWatch(watch) = &options.command {
        return run_watch(socket, &token, &request, watch, options.json);
    }
    let response = send_request(socket, &token, &request)?;
    let is_error = matches!(response.response, ProtocolResponse::Error(_));
    match render_response(&response, options.json) {
        Ok(rendered) => println!("{rendered}"),
        Err(error) if is_error => {
            eprintln!("leserpent: {error}");
            return Ok(3);
        }
        Err(error) => return Err(error),
    }
    Ok(if is_error { 3 } else { 0 })
}

fn run_watch(
    socket: &std::path::Path,
    token: &str,
    request: &RequestEnvelope,
    watch: &RuntimeWatchOptions,
    json: bool,
) -> Result<i32, CliError> {
    let mut last_revision = None;
    for iteration in 0..watch.count {
        let response = send_request(socket, token, request)?;
        if matches!(response.response, ProtocolResponse::Error(_)) {
            let error = render_response(&response, json).unwrap_err();
            eprintln!("leserpent: {error}");
            return Ok(3);
        }
        let revision = match &response.response {
            ProtocolResponse::Query(QueryResult::RuntimeInspect { revision, .. }) => *revision,
            _ => {
                return Err(CliError::Protocol(
                    "runtime watch received an unexpected response".into(),
                ));
            }
        };
        if last_revision != Some(revision) {
            println!("{}", render_response(&response, json)?);
            io::stdout()
                .flush()
                .map_err(|error| CliError::Transport(error.to_string()))?;
            last_revision = Some(revision);
        }
        if iteration + 1 < watch.count {
            std::thread::sleep(Duration::from_millis(watch.interval_ms));
        }
    }
    Ok(0)
}
