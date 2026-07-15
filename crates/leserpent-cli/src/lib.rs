use std::fmt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use leserpent_domain::{
    CAPABILITY_RUNTIME_READ, CAPABILITY_RUNTIME_REFRESH, CapabilitySet, Command, CommandEnvelope,
    CommandId, CommandOrigin, CommandStatus, Confirmation, DOMAIN_SCHEMA_VERSION, IdempotencyKey,
    Principal, Query, QueryEnvelope, QueryResult, Revision, RuntimeId, RuntimeListFilter,
};
use leserpent_protocol::{
    HealthRequest, PROTOCOL_SCHEMA_VERSION, ProtocolRequest, ProtocolResponse, RequestEnvelope,
    ResponseEnvelope,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CliOptions {
    pub socket: Option<PathBuf>,
    pub json: bool,
    pub principal: String,
    pub command: CliCommand,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CliCommand {
    Health,
    RuntimeList(RuntimeListFilter),
    RuntimeRefresh(RuntimeRefreshOptions),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeRefreshOptions {
    pub runtime_id: RuntimeId,
    pub expected_revision: Option<Revision>,
    pub dry_run: bool,
    pub confirmed: bool,
    pub idempotency_key: Option<String>,
    pub export_leselang: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CliError {
    Usage(String),
    Configuration(String),
    Transport(String),
    Protocol(String),
}

impl fmt::Display for CliError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Usage(error) => write!(formatter, "{error}"),
            Self::Configuration(error) => write!(formatter, "configuration error: {error}"),
            Self::Transport(error) => write!(formatter, "transport error: {error}"),
            Self::Protocol(error) => write!(formatter, "protocol error: {error}"),
        }
    }
}

impl std::error::Error for CliError {}

pub const USAGE: &str = "Usage:\n  leserpent --socket PATH [--json] health\n  leserpent --socket PATH [--json] runtime list [--environment VALUE] [--cluster VALUE] [--role VALUE]\n  leserpent --socket PATH [--json] runtime refresh RUNTIME_ID (--dry-run | --yes) [--expected-revision N] [--idempotency-key KEY]\n  leserpent runtime refresh RUNTIME_ID --export-leselang\n\nEnvironment:\n  LESERPENT_SOCKET may provide PATH\n  LESERPENT_IPC_TOKEN must contain the daemon IPC token\n  LESERPENT_PRINCIPAL optionally sets the audit principal";

static REQUEST_SEQUENCE: AtomicU64 = AtomicU64::new(1);

pub fn parse_args(
    arguments: impl IntoIterator<Item = String>,
    socket_from_env: Option<PathBuf>,
    principal_from_env: Option<String>,
) -> Result<CliOptions, CliError> {
    let mut arguments = arguments.into_iter().peekable();
    let mut socket = socket_from_env;
    let mut json = false;
    while let Some(argument) = arguments.peek() {
        match argument.as_str() {
            "--socket" => {
                arguments.next();
                socket =
                    Some(PathBuf::from(arguments.next().ok_or_else(|| {
                        CliError::Usage("--socket requires a path".into())
                    })?));
            }
            "--json" => {
                arguments.next();
                json = true;
            }
            "-h" | "--help" => return Err(CliError::Usage(USAGE.into())),
            _ => break,
        }
    }
    let command = match arguments.next().as_deref() {
        Some("health") => {
            reject_trailing(arguments)?;
            CliCommand::Health
        }
        Some("runtime") => match arguments.next().as_deref() {
            Some("list") => CliCommand::RuntimeList(parse_runtime_filters(arguments)?),
            Some("refresh") => CliCommand::RuntimeRefresh(parse_runtime_refresh(arguments)?),
            Some(command) => {
                return Err(CliError::Usage(format!(
                    "unknown runtime command '{command}'"
                )));
            }
            None => return Err(CliError::Usage("runtime requires a command".into())),
        },
        Some(command) => return Err(CliError::Usage(format!("unknown command '{command}'"))),
        None => return Err(CliError::Usage(USAGE.into())),
    };
    if socket.is_none()
        && !matches!(
            &command,
            CliCommand::RuntimeRefresh(RuntimeRefreshOptions {
                export_leselang: true,
                ..
            })
        )
    {
        return Err(CliError::Configuration(
            "socket path is required via --socket or LESERPENT_SOCKET".into(),
        ));
    }
    let principal = principal_from_env.unwrap_or_else(|| "leserpent-cli".into());
    if !valid_identifier(&principal) {
        return Err(CliError::Configuration(
            "LESERPENT_PRINCIPAL must be a valid identifier".into(),
        ));
    }
    Ok(CliOptions {
        socket,
        json,
        principal,
        command,
    })
}

pub fn request_for(options: &CliOptions) -> Result<RequestEnvelope, CliError> {
    let request = match &options.command {
        CliCommand::Health => ProtocolRequest::Health(HealthRequest {}),
        CliCommand::RuntimeList(filter) => ProtocolRequest::Query(QueryEnvelope {
            schema_version: DOMAIN_SCHEMA_VERSION,
            principal: Principal {
                id: options.principal.clone(),
            },
            capabilities: CapabilitySet::new([CAPABILITY_RUNTIME_READ]),
            query: Query::RuntimeList {
                filter: filter.clone(),
            },
        }),
        CliCommand::RuntimeRefresh(refresh) if !refresh.export_leselang => {
            let request_id = new_request_id();
            let idempotency_key = refresh
                .idempotency_key
                .clone()
                .unwrap_or_else(|| request_id.clone());
            ProtocolRequest::Command(CommandEnvelope {
                schema_version: DOMAIN_SCHEMA_VERSION,
                command_id: CommandId::new(request_id)
                    .map_err(|error| CliError::Protocol(error.to_string()))?,
                idempotency_key: IdempotencyKey::new(idempotency_key)
                    .map_err(|error| CliError::Usage(error.to_string()))?,
                expected_revision: refresh.expected_revision,
                principal: Principal {
                    id: options.principal.clone(),
                },
                capabilities: CapabilitySet::new([CAPABILITY_RUNTIME_REFRESH]),
                origin: CommandOrigin::Cli,
                confirmation: if refresh.confirmed {
                    Confirmation::Confirmed
                } else {
                    Confirmation::NotRequired
                },
                dry_run: refresh.dry_run,
                command: Command::RuntimeRefresh {
                    runtime_id: refresh.runtime_id.clone(),
                },
            })
        }
        CliCommand::RuntimeRefresh(_) => {
            return Err(CliError::Usage(
                "Leselang export does not produce a daemon request".into(),
            ));
        }
    };
    Ok(RequestEnvelope {
        schema_version: PROTOCOL_SCHEMA_VERSION,
        request,
    })
}

pub fn export_leselang(options: &CliOptions) -> Option<String> {
    let CliCommand::RuntimeRefresh(refresh) = &options.command else {
        return None;
    };
    refresh.export_leselang.then(|| {
        format!(
            "fn main() = runtime.refresh(runtime_id: {})",
            serde_json::to_string(refresh.runtime_id.as_str())
                .expect("string encoding cannot fail")
        )
    })
}

pub fn render_response(response: &ResponseEnvelope, json: bool) -> Result<String, CliError> {
    if json {
        return serde_json::to_string(response)
            .map_err(|error| CliError::Protocol(error.to_string()));
    }
    match &response.response {
        ProtocolResponse::Health(health) => {
            let mut output = format!(
                "status={} authority_owned={}",
                health.status, health.authority_owned
            );
            if let Some(queue) = &health.effect_queue {
                output.push_str(&format!(
                    " queue_active={} queue_terminal={} queue_capacity={} saturated={}",
                    queue.active, queue.terminal, queue.capacity, queue.saturated
                ));
            }
            Ok(output)
        }
        ProtocolResponse::Query(QueryResult::RuntimeList { revision, runtimes }) => {
            let mut output = format!("revision={} runtimes={}\n", revision.0, runtimes.len());
            output.push_str("ID\tNAME\tREFRESH\tSOURCE\tENDPOINT\n");
            for runtime in runtimes {
                output.push_str(&safe_cell(runtime.id.as_str()));
                output.push('\t');
                output.push_str(&safe_cell(&runtime.name));
                output.push('\t');
                output.push_str(match runtime.refresh_status {
                    leserpent_domain::RefreshStatus::NeverRequested => "never_requested",
                    leserpent_domain::RefreshStatus::Pending => "pending",
                    leserpent_domain::RefreshStatus::Ready => "ready",
                    leserpent_domain::RefreshStatus::Failed => "failed",
                });
                output.push('\t');
                output.push_str(&safe_cell(&runtime.status.status_source));
                output.push('\t');
                output.push_str(&safe_cell(&runtime.endpoint));
                output.push('\n');
            }
            Ok(output.trim_end().to_string())
        }
        ProtocolResponse::Error(error) => Err(CliError::Protocol(format!(
            "{}: {}",
            error.code, error.message
        ))),
        ProtocolResponse::Command(result) => Ok(format!(
            "status={} runtime={} revision={} refresh_status={}",
            match result.status {
                CommandStatus::Planned => "planned",
                CommandStatus::Applied => "applied",
            },
            result.runtime.id.as_str(),
            result.runtime.revision.0,
            match result.runtime.refresh_status {
                leserpent_domain::RefreshStatus::NeverRequested => "never_requested",
                leserpent_domain::RefreshStatus::Pending => "pending",
                leserpent_domain::RefreshStatus::Ready => "ready",
                leserpent_domain::RefreshStatus::Failed => "failed",
            }
        )),
    }
}

#[cfg(unix)]
pub fn send_request(
    socket: &std::path::Path,
    token: &str,
    request: &RequestEnvelope,
) -> Result<ResponseEnvelope, CliError> {
    use std::io::{BufRead, BufReader, Read, Write};
    use std::os::unix::fs::{FileTypeExt, PermissionsExt};
    use std::os::unix::net::UnixStream;
    use std::time::Duration;

    use leserpent_protocol::{MAX_PROTOCOL_MESSAGE_BYTES, decode_response};
    use serde::Serialize;

    #[derive(Serialize)]
    struct AuthenticatedRequest<'a> {
        token: &'a str,
        request: &'a RequestEnvelope,
    }

    validate_token(token)?;
    let metadata = std::fs::symlink_metadata(socket).map_err(|_| {
        CliError::Transport(format!(
            "daemon socket '{}' is unavailable",
            socket.display()
        ))
    })?;
    if metadata.file_type().is_symlink()
        || !metadata.file_type().is_socket()
        || metadata.permissions().mode() & 0o077 != 0
    {
        return Err(CliError::Transport(
            "daemon socket must be an owner-private socket, not a link".into(),
        ));
    }
    let mut stream = UnixStream::connect(socket).map_err(|_| {
        CliError::Transport(format!(
            "cannot connect to daemon socket '{}': unavailable",
            socket.display()
        ))
    })?;
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
        .map_err(|error| CliError::Transport(error.to_string()))?;
    stream
        .set_write_timeout(Some(Duration::from_secs(3)))
        .map_err(|error| CliError::Transport(error.to_string()))?;
    let mut encoded = serde_json::to_vec(&AuthenticatedRequest { token, request })
        .map_err(|error| CliError::Protocol(error.to_string()))?;
    if encoded.len() > MAX_PROTOCOL_MESSAGE_BYTES + 1024 {
        return Err(CliError::Protocol(
            "authenticated request is too large".into(),
        ));
    }
    encoded.push(b'\n');
    stream
        .write_all(&encoded)
        .map_err(|error| CliError::Transport(error.to_string()))?;
    let mut response = Vec::new();
    BufReader::new(stream)
        .take((MAX_PROTOCOL_MESSAGE_BYTES + 1) as u64)
        .read_until(b'\n', &mut response)
        .map_err(|error| CliError::Transport(error.to_string()))?;
    if response.len() > MAX_PROTOCOL_MESSAGE_BYTES || !response.ends_with(b"\n") {
        return Err(CliError::Protocol(
            "daemon response is missing or exceeds the protocol limit".into(),
        ));
    }
    response.pop();
    decode_response(&response).map_err(|error| CliError::Protocol(format!("{error:?}")))
}

#[cfg(not(unix))]
pub fn send_request(
    _socket: &std::path::Path,
    _token: &str,
    _request: &RequestEnvelope,
) -> Result<ResponseEnvelope, CliError> {
    Err(CliError::Transport(
        "local daemon transport is not implemented on this platform".into(),
    ))
}

fn parse_runtime_filters(
    mut arguments: impl Iterator<Item = String>,
) -> Result<RuntimeListFilter, CliError> {
    let mut filter = RuntimeListFilter::default();
    while let Some(argument) = arguments.next() {
        let value = arguments
            .next()
            .ok_or_else(|| CliError::Usage(format!("{argument} requires a value")))?;
        if value.is_empty() || value.len() > 128 {
            return Err(CliError::Usage(format!("{argument} has an invalid value")));
        }
        match argument.as_str() {
            "--environment" if filter.environment.is_none() => filter.environment = Some(value),
            "--cluster" if filter.cluster.is_none() => filter.cluster = Some(value),
            "--role" if filter.role.is_none() => filter.role = Some(value),
            "--environment" | "--cluster" | "--role" => {
                return Err(CliError::Usage(format!(
                    "{argument} was provided more than once"
                )));
            }
            _ => {
                return Err(CliError::Usage(format!(
                    "unknown runtime list option '{argument}'"
                )));
            }
        }
    }
    Ok(filter)
}

fn parse_runtime_refresh(
    mut arguments: impl Iterator<Item = String>,
) -> Result<RuntimeRefreshOptions, CliError> {
    let runtime_id = arguments
        .next()
        .ok_or_else(|| CliError::Usage("runtime refresh requires RUNTIME_ID".into()))?;
    let runtime_id =
        RuntimeId::new(runtime_id).map_err(|error| CliError::Usage(error.to_string()))?;
    let mut expected_revision = None;
    let mut dry_run = false;
    let mut confirmed = false;
    let mut idempotency_key = None;
    let mut export_leselang = false;
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--dry-run" if !dry_run => dry_run = true,
            "--yes" if !confirmed => confirmed = true,
            "--export-leselang" if !export_leselang => export_leselang = true,
            "--expected-revision" if expected_revision.is_none() => {
                let value = arguments.next().ok_or_else(|| {
                    CliError::Usage("--expected-revision requires an integer".into())
                })?;
                expected_revision = Some(Revision(value.parse::<u64>().map_err(|_| {
                    CliError::Usage("--expected-revision requires an integer".into())
                })?));
            }
            "--idempotency-key" if idempotency_key.is_none() => {
                let value = arguments
                    .next()
                    .ok_or_else(|| CliError::Usage("--idempotency-key requires a value".into()))?;
                if !valid_identifier(&value) {
                    return Err(CliError::Usage("invalid --idempotency-key".into()));
                }
                idempotency_key = Some(value);
            }
            "--dry-run"
            | "--yes"
            | "--export-leselang"
            | "--expected-revision"
            | "--idempotency-key" => {
                return Err(CliError::Usage(format!(
                    "{argument} was provided more than once"
                )));
            }
            _ => {
                return Err(CliError::Usage(format!(
                    "unknown runtime refresh option '{argument}'"
                )));
            }
        }
    }
    if export_leselang
        && (dry_run || confirmed || expected_revision.is_some() || idempotency_key.is_some())
    {
        return Err(CliError::Usage(
            "--export-leselang cannot be combined with execution options".into(),
        ));
    }
    if dry_run && confirmed {
        return Err(CliError::Usage(
            "--dry-run cannot be combined with --yes".into(),
        ));
    }
    if !export_leselang && !dry_run && !confirmed {
        return Err(CliError::Usage(
            "runtime refresh requires --dry-run or explicit --yes confirmation".into(),
        ));
    }
    Ok(RuntimeRefreshOptions {
        runtime_id,
        expected_revision,
        dry_run,
        confirmed,
        idempotency_key,
        export_leselang,
    })
}

fn reject_trailing(mut arguments: impl Iterator<Item = String>) -> Result<(), CliError> {
    match arguments.next() {
        Some(argument) => Err(CliError::Usage(format!("unexpected argument '{argument}'"))),
        None => Ok(()),
    }
}

fn validate_token(token: &str) -> Result<(), CliError> {
    if token.len() < 32 || token.len() > 256 || token.bytes().any(|byte| byte <= 0x20) {
        return Err(CliError::Configuration(
            "LESERPENT_IPC_TOKEN must contain 32 to 256 non-whitespace bytes".into(),
        ));
    }
    Ok(())
}

fn valid_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b':' | b'.'))
}

fn new_request_id() -> String {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let sequence = REQUEST_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    format!("cli-{}-{timestamp}-{sequence}", std::process::id())
}

fn safe_cell(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_control() {
                '\u{fffd}'
            } else {
                character
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parser_builds_normalized_runtime_list_request() {
        let options = parse_args(
            [
                "--json",
                "runtime",
                "list",
                "--environment",
                " production ",
                "--role",
                "edge",
            ]
            .into_iter()
            .map(str::to_string),
            Some("/tmp/leserpent.sock".into()),
            Some("operator-a".into()),
        )
        .unwrap();
        assert!(options.json);
        let ProtocolRequest::Query(query) = request_for(&options).unwrap().request else {
            panic!("runtime list must produce a query request");
        };
        let Query::RuntimeList { filter } = query.query;
        assert_eq!(filter.environment.as_deref(), Some(" production "));
        assert_eq!(filter.role.as_deref(), Some("edge"));
        assert!(query.capabilities.contains(CAPABILITY_RUNTIME_READ));
    }

    #[test]
    fn parser_rejects_ambiguous_or_incomplete_input() {
        assert!(parse_args(["health".into()], None, None).is_err());
        assert!(
            parse_args(
                ["runtime", "list", "--role", "a", "--role", "b"]
                    .into_iter()
                    .map(str::to_string),
                Some("/tmp/x".into()),
                None,
            )
            .is_err()
        );
        assert!(
            parse_args(
                ["health", "extra"].into_iter().map(str::to_string),
                Some("/tmp/x".into()),
                None,
            )
            .is_err()
        );
    }

    #[test]
    fn human_cells_replace_terminal_control_characters() {
        assert_eq!(
            safe_cell("Runtime\nA\t\u{1b}[31m"),
            "Runtime\u{fffd}A\u{fffd}\u{fffd}[31m"
        );
    }
}
