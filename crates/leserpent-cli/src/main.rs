use std::path::PathBuf;

use leserpent_cli::{
    CliError, export_leselang, export_plan, parse_args, render_response, request_for, send_request,
};
use leserpent_protocol::ProtocolResponse;

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
    let response = send_request(socket, &token, &request_for(&options)?)?;
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
