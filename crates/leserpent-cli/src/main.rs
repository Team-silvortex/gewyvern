use std::path::PathBuf;
use std::time::Duration;
use std::{io, io::Write};

use leserpent_cli::{
    CliCommand, CliError, DaemonRetirementWaitOptions, HttpsClient, ProvisioningWaitOptions,
    RemoteTrust, RetirementWaitOptions, RuntimeWatchOptions, bootstrap_request_for,
    daemon_retirement_phase_name, daemon_retirement_request_for, export_leselang, export_plan,
    parse_args_with_remote, provisioning_phase_name, provisioning_request_for,
    render_bootstrap_response, render_daemon_retirement_response, render_provisioning_response,
    render_response, render_retirement_response, request_for, retirement_phase_name,
    retirement_request_for, send_bootstrap_request, send_daemon_retirement_request,
    send_provisioning_request, send_request, send_retirement_request,
};
use leserpent_domain::QueryResult;
use leserpent_domain::bootstrap_retirement::DaemonRetirementPhase;
use leserpent_domain::provisioning::ProvisioningPhase;
use leserpent_domain::retirement::RetirementPhase;
use leserpent_protocol::{ProtocolResponse, RequestEnvelope};
use zeroize::Zeroizing;

fn main() {
    match run() {
        Ok(exit_code) => std::process::exit(exit_code),
        Err(CliError::Usage(message)) if message.starts_with(leserpent_cli::USAGE) => {
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
            let client = match &remote.trust {
                RemoteTrust::CaFile(path) => HttpsClient::new(&remote.endpoint, path, token)?,
                RemoteTrust::BootstrapHandle { root, handle } => {
                    HttpsClient::new_with_bootstrap_trust(&remote.endpoint, root, handle, token)?
                }
            };
            ActiveTransport::Remote(client)
        }
        _ => {
            return Err(CliError::Configuration(
                "exactly one daemon transport is required for execution".into(),
            ));
        }
    };
    if let Some(request) = bootstrap_request_for(&options)? {
        let response = transport.send_bootstrap(&request)?;
        let is_error = matches!(
            response.response,
            leserpent_protocol::bootstrap::BootstrapResponse::Error(_)
        );
        match render_bootstrap_response(&response, options.json) {
            Ok(rendered) => println!("{rendered}"),
            Err(error) if is_error => {
                eprintln!("leserpent: {error}");
                return Ok(3);
            }
            Err(error) => return Err(error),
        }
        return Ok(if is_error { 3 } else { 0 });
    }
    if let Some(request) = provisioning_request_for(&options)? {
        let CliCommand::RuntimeProvision(provision) = &options.command else {
            unreachable!("provisioning request requires a runtime provision command");
        };
        return run_provisioning(&transport, &request, provision.wait, options.json);
    }
    if let Some(request) = daemon_retirement_request_for(&options)? {
        let CliCommand::BootstrapRetire(retirement) = &options.command else {
            unreachable!("daemon retirement request requires a bootstrap retire command");
        };
        return run_daemon_retirement(&transport, &request, retirement.wait, options.json);
    }
    if let Some(request) = retirement_request_for(&options)? {
        let CliCommand::RuntimeRetire(retirement) = &options.command else {
            unreachable!("retirement request requires a runtime retire command");
        };
        return run_retirement(&transport, &request, retirement.wait, options.json);
    }
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

fn run_daemon_retirement(
    transport: &ActiveTransport,
    request: &leserpent_protocol::bootstrap_retirement_control::DaemonRetirementRequestEnvelope,
    wait: Option<DaemonRetirementWaitOptions>,
    json: bool,
) -> Result<i32, CliError> {
    let observations = wait.map_or(1, |options| options.count);
    let mut last_phase = None;
    for observation in 0..observations {
        let response = transport.send_daemon_retirement(request)?;
        match &response.response {
            leserpent_protocol::bootstrap_retirement_control::DaemonRetirementResponse::Error(
                _,
            ) => {
                let error = render_daemon_retirement_response(&response, json).unwrap_err();
                eprintln!("leserpent: {error}");
                return Ok(3);
            }
            leserpent_protocol::bootstrap_retirement_control::DaemonRetirementResponse::State(
                state,
            ) => {
                if last_phase != Some(state.phase) {
                    println!("{}", render_daemon_retirement_response(&response, json)?);
                    io::stdout()
                        .flush()
                        .map_err(|error| CliError::Transport(error.to_string()))?;
                    last_phase = Some(state.phase);
                }
                match state.phase {
                    DaemonRetirementPhase::ServiceRetired => return Ok(0),
                    DaemonRetirementPhase::Failed => return Ok(4),
                    _ if wait.is_none() => return Ok(0),
                    _ => {}
                }
            }
        }
        if observation + 1 < observations {
            std::thread::sleep(Duration::from_millis(
                wait.expect("multiple observations require wait options")
                    .interval_ms,
            ));
        }
    }
    let retirement_id = request.request.intent.retirement_id.as_str();
    let phase = last_phase
        .map(daemon_retirement_phase_name)
        .unwrap_or("unknown");
    eprintln!(
        "leserpent: daemon retirement {} did not reach a terminal phase after {} observations (last_phase={})",
        retirement_id, observations, phase
    );
    Ok(5)
}

fn run_retirement(
    transport: &ActiveTransport,
    request: &leserpent_protocol::retirement::RetirementRequestEnvelope,
    wait: Option<RetirementWaitOptions>,
    json: bool,
) -> Result<i32, CliError> {
    let observations = wait.map_or(1, |options| options.count);
    let mut last_phase = None;
    for observation in 0..observations {
        let response = transport.send_retirement(request)?;
        match &response.response {
            leserpent_protocol::retirement::RetirementResponse::Error(_) => {
                let error = render_retirement_response(&response, json).unwrap_err();
                eprintln!("leserpent: {error}");
                return Ok(3);
            }
            leserpent_protocol::retirement::RetirementResponse::State(state) => {
                if last_phase != Some(state.phase) {
                    println!("{}", render_retirement_response(&response, json)?);
                    io::stdout()
                        .flush()
                        .map_err(|error| CliError::Transport(error.to_string()))?;
                    last_phase = Some(state.phase);
                }
                match state.phase {
                    RetirementPhase::RuntimeUnregistered => return Ok(0),
                    RetirementPhase::Failed => return Ok(4),
                    _ if wait.is_none() => return Ok(0),
                    _ => {}
                }
            }
        }
        if observation + 1 < observations {
            std::thread::sleep(Duration::from_millis(
                wait.expect("multiple observations require wait options")
                    .interval_ms,
            ));
        }
    }
    let retirement_id = request.request.intent.retirement_id.as_str();
    let phase = last_phase.map(retirement_phase_name).unwrap_or("unknown");
    eprintln!(
        "leserpent: retirement {} did not reach a terminal phase after {} observations (last_phase={})",
        retirement_id, observations, phase
    );
    Ok(5)
}

fn run_provisioning(
    transport: &ActiveTransport,
    request: &leserpent_protocol::provisioning::ProvisioningRequestEnvelope,
    wait: Option<ProvisioningWaitOptions>,
    json: bool,
) -> Result<i32, CliError> {
    let observations = wait.map_or(1, |options| options.count);
    let mut last_phase = None;
    for observation in 0..observations {
        let response = transport.send_provisioning(request)?;
        match &response.response {
            leserpent_protocol::provisioning::ProvisioningResponse::Error(_) => {
                let error = render_provisioning_response(&response, json).unwrap_err();
                eprintln!("leserpent: {error}");
                return Ok(3);
            }
            leserpent_protocol::provisioning::ProvisioningResponse::State(state) => {
                if last_phase != Some(state.phase) {
                    println!("{}", render_provisioning_response(&response, json)?);
                    io::stdout()
                        .flush()
                        .map_err(|error| CliError::Transport(error.to_string()))?;
                    last_phase = Some(state.phase);
                }
                match state.phase {
                    ProvisioningPhase::RuntimeRegistered => return Ok(0),
                    ProvisioningPhase::Failed => return Ok(4),
                    _ if wait.is_none() => return Ok(0),
                    _ => {}
                }
            }
        }
        if observation + 1 < observations {
            std::thread::sleep(Duration::from_millis(
                wait.expect("multiple observations require wait options")
                    .interval_ms,
            ));
        }
    }
    let provisioning_id = request.request.intent.provisioning_id.as_str();
    let phase = last_phase.map(provisioning_phase_name).unwrap_or("unknown");
    eprintln!(
        "leserpent: provisioning {} did not reach a terminal phase after {} observations (last_phase={})",
        provisioning_id, observations, phase
    );
    Ok(5)
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

    fn send_bootstrap(
        &self,
        request: &leserpent_protocol::bootstrap::BootstrapRequestEnvelope,
    ) -> Result<leserpent_protocol::bootstrap::BootstrapResponseEnvelope, CliError> {
        match self {
            Self::Local { socket, token } => {
                send_bootstrap_request(socket, token.as_str(), request)
            }
            Self::Remote(client) => client.send_bootstrap(request),
        }
    }

    fn send_provisioning(
        &self,
        request: &leserpent_protocol::provisioning::ProvisioningRequestEnvelope,
    ) -> Result<leserpent_protocol::provisioning::ProvisioningResponseEnvelope, CliError> {
        match self {
            Self::Local { socket, token } => {
                send_provisioning_request(socket, token.as_str(), request)
            }
            Self::Remote(client) => client.send_provisioning(request),
        }
    }

    fn send_retirement(
        &self,
        request: &leserpent_protocol::retirement::RetirementRequestEnvelope,
    ) -> Result<leserpent_protocol::retirement::RetirementResponseEnvelope, CliError> {
        match self {
            Self::Local { socket, token } => {
                send_retirement_request(socket, token.as_str(), request)
            }
            Self::Remote(client) => client.send_retirement(request),
        }
    }

    fn send_daemon_retirement(
        &self,
        request: &leserpent_protocol::bootstrap_retirement_control::DaemonRetirementRequestEnvelope,
    ) -> Result<
        leserpent_protocol::bootstrap_retirement_control::DaemonRetirementResponseEnvelope,
        CliError,
    > {
        match self {
            Self::Local { socket, token } => {
                send_daemon_retirement_request(socket, token.as_str(), request)
            }
            Self::Remote(client) => client.send_daemon_retirement(request),
        }
    }
}
