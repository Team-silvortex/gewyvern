use std::path::PathBuf;
use std::time::Duration;
use std::{io, io::Write};

use leserpent_cli::{
    CliCommand, CliError, HttpsClient, RuntimeWatchOptions, export_leselang, export_plan,
    parse_args_with_remote, render_response, request_for, send_request,
};
use leserpent_domain::QueryResult;
use leserpent_protocol::{ProtocolResponse, RequestEnvelope};
use zeroize::Zeroizing;

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
    let options = parse_args_with_remote(
        std::env::args().skip(1),
        std::env::var_os("LESERPENT_SOCKET").map(PathBuf::from),
        std::env::var("LESERPENT_REMOTE").ok(),
        std::env::var_os("LESERPENT_REMOTE_CA").map(PathBuf::from),
        std::env::var("LESERPENT_PRINCIPAL").ok(),
    )?;
    if let Some(source) = export_leselang(&options) {
        print!("{source}");
        return Ok(0);
    }
    if let Some(plan) = export_plan(&options)? {
        println!("{plan}");
        return Ok(0);
    }
    let transport = match (&options.socket, &options.remote) {
        (Some(socket), None) => {
            let token = std::env::var("LESERPENT_IPC_TOKEN")
                .map_err(|_| CliError::Configuration("LESERPENT_IPC_TOKEN is required".into()))?;
            ActiveTransport::Local {
                socket: socket.clone(),
                token: Zeroizing::new(token),
            }
        }
        (None, Some(remote)) => {
            let token = std::env::var("LESERPENT_REMOTE_TOKEN").map_err(|_| {
                CliError::Configuration("LESERPENT_REMOTE_TOKEN is required".into())
            })?;
            ActiveTransport::Remote(HttpsClient::new(&remote.endpoint, &remote.ca, token)?)
        }
        _ => {
            return Err(CliError::Configuration(
                "exactly one daemon transport is required for execution".into(),
            ));
        }
    };
    let request = request_for(&options)?;
    if let CliCommand::RuntimeWatch(watch) = &options.command {
        return run_watch(&transport, &request, watch, options.json);
    }
    let response = transport.send(&request)?;
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
    transport: &ActiveTransport,
    request: &RequestEnvelope,
    watch: &RuntimeWatchOptions,
    json: bool,
) -> Result<i32, CliError> {
    let mut last_revision = None;
    for iteration in 0..watch.count {
        let response = transport.send(request)?;
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

enum ActiveTransport {
    Local {
        socket: PathBuf,
        token: Zeroizing<String>,
    },
    Remote(HttpsClient),
}

impl ActiveTransport {
    fn send(
        &self,
        request: &RequestEnvelope,
    ) -> Result<leserpent_protocol::ResponseEnvelope, CliError> {
        match self {
            Self::Local { socket, token } => send_request(socket, token.as_str(), request),
            Self::Remote(client) => client.send(request),
        }
    }
}
