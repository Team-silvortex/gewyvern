use std::fmt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use leselang_command::{
    LoweringContext, PlannedOperation, encode_plan, plan_runtime_capabilities_refresh,
    plan_runtime_deploy, plan_runtime_history, plan_runtime_inspect, plan_runtime_list,
    plan_runtime_logs, plan_runtime_refresh,
};
use leselang_syntax::{format as format_leselang, parse as parse_leselang};
use leserpent_domain::{
    CAPABILITY_RUNTIME_DEPLOY, CAPABILITY_RUNTIME_READ, CAPABILITY_RUNTIME_REFRESH, CapabilitySet,
    CommandId, CommandOrigin, CommandStatus, Confirmation, IdempotencyKey, Principal, QueryResult,
    Revision, RuntimeId, RuntimeListFilter, validate_deployment_intent,
};
use leserpent_protocol::{
    HealthRequest, PROTOCOL_SCHEMA_VERSION, ProtocolRequest, ProtocolResponse, RequestEnvelope,
    ResponseEnvelope,
};

mod https;
pub use https::HttpsClient;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CliOptions {
    pub socket: Option<PathBuf>,
    pub remote: Option<RemoteOptions>,
    pub json: bool,
    pub principal: String,
    pub command: CliCommand,
    pub local_export: Option<LocalExport>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RemoteOptions {
    pub endpoint: String,
    pub ca: PathBuf,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LocalExport {
    Leselang,
    Plan,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CliCommand {
    Health,
    RuntimeList(RuntimeListFilter),
    RuntimeInspect(RuntimeId),
    RuntimeHistory(RuntimeId),
    RuntimeLogs(RuntimeId),
    RuntimeWatch(RuntimeWatchOptions),
    RuntimeRefresh(RuntimeRefreshOptions),
    RuntimeCapabilitiesRefresh(RuntimeRefreshOptions),
    RuntimeDeploy(RuntimeDeployOptions),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeWatchOptions {
    pub runtime_id: RuntimeId,
    pub count: u16,
    pub interval_ms: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeRefreshOptions {
    pub runtime_id: RuntimeId,
    pub expected_revision: Option<Revision>,
    pub dry_run: bool,
    pub confirmed: bool,
    pub idempotency_key: Option<String>,
    pub export_leselang: bool,
    pub export_plan: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeDeployOptions {
    pub runtime_id: RuntimeId,
    pub pipeline_kind: String,
    pub target: Option<String>,
    pub expected_revision: Option<Revision>,
    pub dry_run: bool,
    pub confirmed: bool,
    pub idempotency_key: Option<String>,
    pub export_leselang: bool,
    pub export_plan: bool,
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

pub const USAGE: &str = "Usage:\n  leserpent [--socket PATH | --remote HTTPS_URL --remote-ca PATH] [--json] health\n  leserpent [--socket PATH | --remote HTTPS_URL --remote-ca PATH] [--json] runtime list [--environment VALUE] [--cluster VALUE] [--role VALUE]\n  leserpent [--socket PATH | --remote HTTPS_URL --remote-ca PATH] [--json] runtime inspect RUNTIME_ID\n  leserpent [--socket PATH | --remote HTTPS_URL --remote-ca PATH] [--json] runtime history RUNTIME_ID\n  leserpent [--socket PATH | --remote HTTPS_URL --remote-ca PATH] [--json] runtime logs RUNTIME_ID\n  leserpent [--socket PATH | --remote HTTPS_URL --remote-ca PATH] [--json] runtime watch RUNTIME_ID [--count N] [--interval-ms N]\n  leserpent runtime list [FILTERS] (--export-leselang | --export-plan)\n  leserpent runtime inspect RUNTIME_ID (--export-leselang | --export-plan)\n  leserpent runtime history RUNTIME_ID (--export-leselang | --export-plan)\n  leserpent runtime logs RUNTIME_ID (--export-leselang | --export-plan)\n  leserpent [--socket PATH | --remote HTTPS_URL --remote-ca PATH] [--json] runtime refresh RUNTIME_ID (--dry-run | --yes) [--expected-revision N] [--idempotency-key KEY]\n  leserpent runtime refresh RUNTIME_ID --export-leselang\n  leserpent runtime refresh RUNTIME_ID (--dry-run | --yes) --idempotency-key KEY --export-plan\n  leserpent [--socket PATH | --remote HTTPS_URL --remote-ca PATH] [--json] runtime refresh-capabilities RUNTIME_ID (--dry-run | --yes) [--expected-revision N] [--idempotency-key KEY]\n  leserpent runtime refresh-capabilities RUNTIME_ID --export-leselang\n  leserpent runtime refresh-capabilities RUNTIME_ID (--dry-run | --yes) --idempotency-key KEY --export-plan\n  leserpent [--socket PATH | --remote HTTPS_URL --remote-ca PATH] [--json] runtime deploy RUNTIME_ID --pipeline-kind KIND [--target VALUE] (--dry-run | --yes) [--expected-revision N] [--idempotency-key KEY]\n  leserpent runtime deploy RUNTIME_ID --pipeline-kind KIND [--target VALUE] --export-leselang\n  leserpent runtime deploy RUNTIME_ID --pipeline-kind KIND [--target VALUE] (--dry-run | --yes) --idempotency-key KEY --export-plan\n\nEnvironment:\n  LESERPENT_SOCKET may provide PATH\n  LESERPENT_IPC_TOKEN must contain the daemon IPC token\n  LESERPENT_REMOTE and LESERPENT_REMOTE_CA may provide the HTTPS endpoint and CA path\n  LESERPENT_REMOTE_TOKEN must contain the remote bearer token\n  LESERPENT_PRINCIPAL optionally sets the audit principal";

static REQUEST_SEQUENCE: AtomicU64 = AtomicU64::new(1);

pub fn parse_args(
    arguments: impl IntoIterator<Item = String>,
    socket_from_env: Option<PathBuf>,
    principal_from_env: Option<String>,
) -> Result<CliOptions, CliError> {
    parse_args_with_remote(arguments, socket_from_env, None, None, principal_from_env)
}

pub fn parse_args_with_remote(
    arguments: impl IntoIterator<Item = String>,
    socket_from_env: Option<PathBuf>,
    remote_from_env: Option<String>,
    remote_ca_from_env: Option<PathBuf>,
    principal_from_env: Option<String>,
) -> Result<CliOptions, CliError> {
    let mut arguments = arguments.into_iter().peekable();
    let mut socket = socket_from_env;
    let mut remote = remote_from_env;
    let mut remote_ca = remote_ca_from_env;
    let mut explicit_socket = false;
    let mut explicit_remote = false;
    let mut explicit_remote_ca = false;
    let mut json = false;
    while let Some(argument) = arguments.peek() {
        match argument.as_str() {
            "--socket" => {
                arguments.next();
                if explicit_socket || explicit_remote || explicit_remote_ca {
                    return Err(CliError::Usage(
                        "transport options must not be repeated or mixed".into(),
                    ));
                }
                explicit_socket = true;
                socket =
                    Some(PathBuf::from(arguments.next().ok_or_else(|| {
                        CliError::Usage("--socket requires a path".into())
                    })?));
                remote = None;
                remote_ca = None;
            }
            "--json" => {
                arguments.next();
                json = true;
            }
            "--remote" => {
                arguments.next();
                if explicit_remote || explicit_socket {
                    return Err(CliError::Usage(
                        "transport options must not be repeated or mixed".into(),
                    ));
                }
                explicit_remote = true;
                socket = None;
                remote = Some(
                    arguments
                        .next()
                        .ok_or_else(|| CliError::Usage("--remote requires an HTTPS URL".into()))?,
                );
            }
            "--remote-ca" => {
                arguments.next();
                if explicit_remote_ca || explicit_socket {
                    return Err(CliError::Usage(
                        "transport options must not be repeated or mixed".into(),
                    ));
                }
                explicit_remote_ca = true;
                socket = None;
                remote_ca =
                    Some(PathBuf::from(arguments.next().ok_or_else(|| {
                        CliError::Usage("--remote-ca requires a path".into())
                    })?));
            }
            "-h" | "--help" => return Err(CliError::Usage(USAGE.into())),
            _ => break,
        }
    }
    let (command, local_export) = match arguments.next().as_deref() {
        Some("health") => {
            reject_trailing(arguments)?;
            (CliCommand::Health, None)
        }
        Some("runtime") => match arguments.next().as_deref() {
            Some("list") => {
                let (filter, export) = parse_runtime_filters(arguments)?;
                (CliCommand::RuntimeList(filter), export)
            }
            Some("inspect") => {
                let runtime_id = arguments
                    .next()
                    .ok_or_else(|| CliError::Usage("runtime inspect requires RUNTIME_ID".into()))?;
                let runtime_id = RuntimeId::new(runtime_id)
                    .map_err(|error| CliError::Usage(error.to_string()))?;
                let export = parse_local_export(arguments)?;
                (CliCommand::RuntimeInspect(runtime_id), export)
            }
            Some("history") => {
                let runtime_id = arguments
                    .next()
                    .ok_or_else(|| CliError::Usage("runtime history requires RUNTIME_ID".into()))?;
                let runtime_id = RuntimeId::new(runtime_id)
                    .map_err(|error| CliError::Usage(error.to_string()))?;
                let export = parse_local_export(arguments)?;
                (CliCommand::RuntimeHistory(runtime_id), export)
            }
            Some("logs") => {
                let runtime_id = arguments
                    .next()
                    .ok_or_else(|| CliError::Usage("runtime logs requires RUNTIME_ID".into()))?;
                let runtime_id = RuntimeId::new(runtime_id)
                    .map_err(|error| CliError::Usage(error.to_string()))?;
                let export = parse_local_export(arguments)?;
                (CliCommand::RuntimeLogs(runtime_id), export)
            }
            Some("watch") => (
                CliCommand::RuntimeWatch(parse_runtime_watch(arguments)?),
                None,
            ),
            Some("refresh") => (
                CliCommand::RuntimeRefresh(parse_runtime_refresh(arguments, "runtime refresh")?),
                None,
            ),
            Some("refresh-capabilities") => (
                CliCommand::RuntimeCapabilitiesRefresh(parse_runtime_refresh(
                    arguments,
                    "runtime refresh-capabilities",
                )?),
                None,
            ),
            Some("deploy") => (
                CliCommand::RuntimeDeploy(parse_runtime_deploy(arguments)?),
                None,
            ),
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
    if socket.is_some() && (remote.is_some() || remote_ca.is_some()) {
        return Err(CliError::Configuration(
            "--socket and remote HTTPS transport are mutually exclusive".into(),
        ));
    }
    let remote = match (remote, remote_ca) {
        (None, None) => None,
        (Some(endpoint), Some(ca)) => Some(RemoteOptions { endpoint, ca }),
        _ => {
            return Err(CliError::Configuration(
                "remote HTTPS transport requires both endpoint and CA path".into(),
            ));
        }
    };
    if socket.is_none()
        && remote.is_none()
        && local_export.is_none()
        && !matches!(
            &command,
            CliCommand::RuntimeRefresh(RuntimeRefreshOptions {
                export_leselang: true,
                ..
            }) | CliCommand::RuntimeRefresh(RuntimeRefreshOptions {
                export_plan: true,
                ..
            }) | CliCommand::RuntimeCapabilitiesRefresh(RuntimeRefreshOptions {
                export_leselang: true,
                ..
            }) | CliCommand::RuntimeCapabilitiesRefresh(RuntimeRefreshOptions {
                export_plan: true,
                ..
            }) | CliCommand::RuntimeDeploy(RuntimeDeployOptions {
                export_leselang: true,
                ..
            }) | CliCommand::RuntimeDeploy(RuntimeDeployOptions {
                export_plan: true,
                ..
            })
        )
    {
        return Err(CliError::Configuration(
            "transport is required via local socket or remote HTTPS configuration".into(),
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
        remote,
        json,
        principal,
        command,
        local_export,
    })
}

pub fn request_for(options: &CliOptions) -> Result<RequestEnvelope, CliError> {
    if options.local_export.is_some() {
        return Err(CliError::Usage(
            "local export does not produce a daemon request".into(),
        ));
    }
    let request = match &options.command {
        CliCommand::Health => ProtocolRequest::Health(HealthRequest {}),
        CliCommand::RuntimeList(filter) => {
            let plan = plan_runtime_list(filter, &query_lowering_context(options))
                .map_err(|error| CliError::Protocol(format!("plan lowering failed: {error:?}")))?;
            let PlannedOperation::Query(query) = plan.operation else {
                return Err(CliError::Protocol(
                    "runtime list lowered to a non-query operation".into(),
                ));
            };
            ProtocolRequest::Query(query)
        }
        CliCommand::RuntimeInspect(runtime_id) => {
            let plan = plan_runtime_inspect(runtime_id, &query_lowering_context(options))
                .map_err(|error| CliError::Protocol(format!("plan lowering failed: {error:?}")))?;
            let PlannedOperation::Query(query) = plan.operation else {
                return Err(CliError::Protocol(
                    "runtime inspect lowered to a non-query operation".into(),
                ));
            };
            ProtocolRequest::Query(query)
        }
        CliCommand::RuntimeHistory(runtime_id) => {
            let plan = plan_runtime_history(runtime_id, &query_lowering_context(options))
                .map_err(|error| CliError::Protocol(format!("plan lowering failed: {error:?}")))?;
            let PlannedOperation::Query(query) = plan.operation else {
                return Err(CliError::Protocol(
                    "runtime history lowered to a non-query operation".into(),
                ));
            };
            ProtocolRequest::Query(query)
        }
        CliCommand::RuntimeLogs(runtime_id) => {
            let plan = plan_runtime_logs(runtime_id, &query_lowering_context(options))
                .map_err(|error| CliError::Protocol(format!("plan lowering failed: {error:?}")))?;
            let PlannedOperation::Query(query) = plan.operation else {
                return Err(CliError::Protocol(
                    "runtime logs lowered to a non-query operation".into(),
                ));
            };
            ProtocolRequest::Query(query)
        }
        CliCommand::RuntimeWatch(watch) => {
            let plan = plan_runtime_inspect(&watch.runtime_id, &query_lowering_context(options))
                .map_err(|error| CliError::Protocol(format!("plan lowering failed: {error:?}")))?;
            let PlannedOperation::Query(query) = plan.operation else {
                return Err(CliError::Protocol(
                    "runtime watch lowered to a non-query operation".into(),
                ));
            };
            ProtocolRequest::Query(query)
        }
        CliCommand::RuntimeRefresh(refresh) if !refresh.export_leselang && !refresh.export_plan => {
            let request_id = new_request_id();
            let idempotency_key = refresh
                .idempotency_key
                .clone()
                .unwrap_or_else(|| request_id.clone());
            let plan = plan_runtime_refresh(
                &refresh.runtime_id,
                &LoweringContext {
                    principal: Principal {
                        id: options.principal.clone(),
                    },
                    capabilities: CapabilitySet::new([CAPABILITY_RUNTIME_REFRESH]),
                    expected_revision: refresh.expected_revision,
                    command_id: CommandId::new(request_id)
                        .map_err(|error| CliError::Protocol(error.to_string()))?,
                    idempotency_key: IdempotencyKey::new(idempotency_key)
                        .map_err(|error| CliError::Usage(error.to_string()))?,
                    origin: CommandOrigin::Cli,
                    confirmation: if refresh.confirmed {
                        Confirmation::Confirmed
                    } else {
                        Confirmation::NotRequired
                    },
                    dry_run: refresh.dry_run,
                },
            )
            .map_err(|error| CliError::Protocol(format!("plan lowering failed: {error:?}")))?;
            let PlannedOperation::Command(command) = plan.operation else {
                return Err(CliError::Protocol(
                    "runtime refresh lowered to a non-command operation".into(),
                ));
            };
            ProtocolRequest::Command(command)
        }
        CliCommand::RuntimeRefresh(_) => {
            return Err(CliError::Usage(
                "Leselang export does not produce a daemon request".into(),
            ));
        }
        CliCommand::RuntimeCapabilitiesRefresh(refresh)
            if !refresh.export_leselang && !refresh.export_plan =>
        {
            let request_id = new_request_id();
            let idempotency_key = refresh
                .idempotency_key
                .clone()
                .unwrap_or_else(|| request_id.clone());
            let plan = plan_runtime_capabilities_refresh(
                &refresh.runtime_id,
                &LoweringContext {
                    principal: Principal {
                        id: options.principal.clone(),
                    },
                    capabilities: CapabilitySet::new([CAPABILITY_RUNTIME_REFRESH]),
                    expected_revision: refresh.expected_revision,
                    command_id: CommandId::new(request_id)
                        .map_err(|error| CliError::Protocol(error.to_string()))?,
                    idempotency_key: IdempotencyKey::new(idempotency_key)
                        .map_err(|error| CliError::Usage(error.to_string()))?,
                    origin: CommandOrigin::Cli,
                    confirmation: if refresh.confirmed {
                        Confirmation::Confirmed
                    } else {
                        Confirmation::NotRequired
                    },
                    dry_run: refresh.dry_run,
                },
            )
            .map_err(|error| CliError::Protocol(format!("plan lowering failed: {error:?}")))?;
            let PlannedOperation::Command(command) = plan.operation else {
                return Err(CliError::Protocol(
                    "runtime capabilities refresh lowered to a non-command operation".into(),
                ));
            };
            ProtocolRequest::Command(command)
        }
        CliCommand::RuntimeCapabilitiesRefresh(_) => {
            return Err(CliError::Usage(
                "Leselang export does not produce a daemon request".into(),
            ));
        }
        CliCommand::RuntimeDeploy(deploy) if !deploy.export_leselang && !deploy.export_plan => {
            let request_id = new_request_id();
            let idempotency_key = deploy
                .idempotency_key
                .clone()
                .unwrap_or_else(|| request_id.clone());
            let plan = plan_runtime_deploy(
                &deploy.runtime_id,
                &deploy.pipeline_kind,
                deploy.target.as_deref(),
                &LoweringContext {
                    principal: Principal {
                        id: options.principal.clone(),
                    },
                    capabilities: CapabilitySet::new([CAPABILITY_RUNTIME_DEPLOY]),
                    expected_revision: deploy.expected_revision,
                    command_id: CommandId::new(request_id)
                        .map_err(|error| CliError::Protocol(error.to_string()))?,
                    idempotency_key: IdempotencyKey::new(idempotency_key)
                        .map_err(|error| CliError::Usage(error.to_string()))?,
                    origin: CommandOrigin::Cli,
                    confirmation: if deploy.confirmed {
                        Confirmation::Confirmed
                    } else {
                        Confirmation::NotRequired
                    },
                    dry_run: deploy.dry_run,
                },
            )
            .map_err(|error| CliError::Protocol(format!("plan lowering failed: {error:?}")))?;
            let PlannedOperation::Command(command) = plan.operation else {
                return Err(CliError::Protocol(
                    "runtime deploy lowered to a non-command operation".into(),
                ));
            };
            ProtocolRequest::Command(command)
        }
        CliCommand::RuntimeDeploy(_) => {
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
    let source = match (&options.command, options.local_export) {
        (CliCommand::RuntimeList(filter), Some(LocalExport::Leselang)) => {
            let filter = filter.clone().normalized();
            Some(format!(
                "fn main() = runtime.list(environment: {}, cluster: {}, role: {})",
                leselang_optional_string(filter.environment.as_deref()),
                leselang_optional_string(filter.cluster.as_deref()),
                leselang_optional_string(filter.role.as_deref()),
            ))
        }
        (CliCommand::RuntimeInspect(runtime_id), Some(LocalExport::Leselang)) => Some(format!(
            "fn main() = runtime.inspect(runtime_id: {})",
            serde_json::to_string(runtime_id.as_str()).expect("string encoding cannot fail")
        )),
        (CliCommand::RuntimeHistory(runtime_id), Some(LocalExport::Leselang)) => Some(format!(
            "fn main() = runtime.history(runtime_id: {})",
            serde_json::to_string(runtime_id.as_str()).expect("string encoding cannot fail")
        )),
        (CliCommand::RuntimeLogs(runtime_id), Some(LocalExport::Leselang)) => Some(format!(
            "fn main() = runtime.logs(runtime_id: {})",
            serde_json::to_string(runtime_id.as_str()).expect("string encoding cannot fail")
        )),
        (CliCommand::RuntimeRefresh(refresh), _) if refresh.export_leselang => Some(format!(
            "fn main() = runtime.refresh(runtime_id: {})",
            serde_json::to_string(refresh.runtime_id.as_str())
                .expect("string encoding cannot fail")
        )),
        (CliCommand::RuntimeCapabilitiesRefresh(refresh), _) if refresh.export_leselang => {
            Some(format!(
                "fn main() = runtime.refresh_capabilities(runtime_id: {})",
                serde_json::to_string(refresh.runtime_id.as_str())
                    .expect("string encoding cannot fail")
            ))
        }
        (CliCommand::RuntimeDeploy(deploy), _) if deploy.export_leselang => Some(format!(
            "fn main() = runtime.deploy(runtime_id: {}, pipeline_kind: {}, target: {})",
            serde_json::to_string(deploy.runtime_id.as_str()).expect("string encoding cannot fail"),
            serde_json::to_string(&deploy.pipeline_kind).expect("string encoding cannot fail"),
            leselang_optional_string(deploy.target.as_deref()),
        )),
        _ => None,
    }?;
    Some(
        format_leselang(&parse_leselang(&source))
            .expect("CLI-generated Leselang must satisfy the syntax contract"),
    )
}

pub fn export_plan(options: &CliOptions) -> Result<Option<String>, CliError> {
    let plan = match (&options.command, options.local_export) {
        (CliCommand::RuntimeList(filter), Some(LocalExport::Plan)) => {
            plan_runtime_list(filter, &query_lowering_context(options))
        }
        (CliCommand::RuntimeInspect(runtime_id), Some(LocalExport::Plan)) => {
            plan_runtime_inspect(runtime_id, &query_lowering_context(options))
        }
        (CliCommand::RuntimeHistory(runtime_id), Some(LocalExport::Plan)) => {
            plan_runtime_history(runtime_id, &query_lowering_context(options))
        }
        (CliCommand::RuntimeLogs(runtime_id), Some(LocalExport::Plan)) => {
            plan_runtime_logs(runtime_id, &query_lowering_context(options))
        }
        (CliCommand::RuntimeRefresh(refresh), _) if refresh.export_plan => {
            let idempotency_key = refresh
                .idempotency_key
                .as_ref()
                .expect("validated plan export idempotency key");
            plan_runtime_refresh(
                &refresh.runtime_id,
                &LoweringContext {
                    principal: Principal {
                        id: options.principal.clone(),
                    },
                    capabilities: CapabilitySet::new([CAPABILITY_RUNTIME_REFRESH]),
                    expected_revision: refresh.expected_revision,
                    command_id: CommandId::new(idempotency_key.clone())
                        .map_err(|error| CliError::Protocol(error.to_string()))?,
                    idempotency_key: IdempotencyKey::new(idempotency_key.clone())
                        .map_err(|error| CliError::Protocol(error.to_string()))?,
                    origin: CommandOrigin::Cli,
                    confirmation: if refresh.confirmed {
                        Confirmation::Confirmed
                    } else {
                        Confirmation::NotRequired
                    },
                    dry_run: refresh.dry_run,
                },
            )
        }
        (CliCommand::RuntimeCapabilitiesRefresh(refresh), _) if refresh.export_plan => {
            let idempotency_key = refresh
                .idempotency_key
                .as_ref()
                .expect("validated plan export idempotency key");
            plan_runtime_capabilities_refresh(
                &refresh.runtime_id,
                &LoweringContext {
                    principal: Principal {
                        id: options.principal.clone(),
                    },
                    capabilities: CapabilitySet::new([CAPABILITY_RUNTIME_REFRESH]),
                    expected_revision: refresh.expected_revision,
                    command_id: CommandId::new(idempotency_key.clone())
                        .map_err(|error| CliError::Protocol(error.to_string()))?,
                    idempotency_key: IdempotencyKey::new(idempotency_key.clone())
                        .map_err(|error| CliError::Protocol(error.to_string()))?,
                    origin: CommandOrigin::Cli,
                    confirmation: if refresh.confirmed {
                        Confirmation::Confirmed
                    } else {
                        Confirmation::NotRequired
                    },
                    dry_run: refresh.dry_run,
                },
            )
        }
        (CliCommand::RuntimeDeploy(deploy), _) if deploy.export_plan => {
            let idempotency_key = deploy
                .idempotency_key
                .as_ref()
                .expect("validated plan export idempotency key");
            plan_runtime_deploy(
                &deploy.runtime_id,
                &deploy.pipeline_kind,
                deploy.target.as_deref(),
                &LoweringContext {
                    principal: Principal {
                        id: options.principal.clone(),
                    },
                    capabilities: CapabilitySet::new([CAPABILITY_RUNTIME_DEPLOY]),
                    expected_revision: deploy.expected_revision,
                    command_id: CommandId::new(idempotency_key.clone())
                        .map_err(|error| CliError::Protocol(error.to_string()))?,
                    idempotency_key: IdempotencyKey::new(idempotency_key.clone())
                        .map_err(|error| CliError::Protocol(error.to_string()))?,
                    origin: CommandOrigin::Cli,
                    confirmation: if deploy.confirmed {
                        Confirmation::Confirmed
                    } else {
                        Confirmation::NotRequired
                    },
                    dry_run: deploy.dry_run,
                },
            )
        }
        _ => return Ok(None),
    }
    .map_err(|error| CliError::Protocol(format!("plan lowering failed: {error:?}")))?;
    let encoded = encode_plan(&plan)
        .map_err(|error| CliError::Protocol(format!("plan encoding failed: {error:?}")))?;
    String::from_utf8(encoded)
        .map(Some)
        .map_err(|error| CliError::Protocol(format!("plan encoding is not UTF-8: {error}")))
}

fn query_lowering_context(options: &CliOptions) -> LoweringContext {
    LoweringContext {
        principal: Principal {
            id: options.principal.clone(),
        },
        capabilities: CapabilitySet::new([CAPABILITY_RUNTIME_READ]),
        expected_revision: None,
        command_id: CommandId::new("unused-query-command")
            .expect("static query command identifier is valid"),
        idempotency_key: IdempotencyKey::new("unused-query-effect")
            .expect("static query idempotency key is valid"),
        origin: CommandOrigin::Cli,
        confirmation: Confirmation::NotRequired,
        dry_run: false,
    }
}

fn leselang_optional_string(value: Option<&str>) -> String {
    value.map_or_else(
        || "none".to_string(),
        |value| serde_json::to_string(value).expect("string encoding cannot fail"),
    )
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
        ProtocolResponse::Query(QueryResult::RuntimeInspect { revision, runtime }) => {
            let mut output = format!(
                "revision={} runtime={} name={} endpoint={} refresh_status={} source={}",
                revision.0,
                safe_cell(runtime.id.as_str()),
                safe_cell(&runtime.name),
                safe_cell(&runtime.endpoint),
                match runtime.refresh_status {
                    leserpent_domain::RefreshStatus::NeverRequested => "never_requested",
                    leserpent_domain::RefreshStatus::Pending => "pending",
                    leserpent_domain::RefreshStatus::Ready => "ready",
                    leserpent_domain::RefreshStatus::Failed => "failed",
                },
                safe_cell(&runtime.status.status_source),
            );
            append_capability_summary(
                &mut output,
                &runtime.capabilities,
                runtime.capabilities_observed_for_revision,
            );
            Ok(output)
        }
        ProtocolResponse::Query(QueryResult::RuntimeHistory { revision, entries }) => {
            let mut output = format!("revision={} entries={}\n", revision.0, entries.len());
            output.push_str("COMMAND\tRUNTIME\tREVISION\tSTATUS\n");
            for entry in entries {
                output.push_str(&safe_cell(entry.command_id.as_str()));
                output.push('\t');
                output.push_str(&safe_cell(entry.runtime.id.as_str()));
                output.push('\t');
                output.push_str(&entry.runtime.revision.0.to_string());
                output.push('\t');
                output.push_str(match entry.status {
                    CommandStatus::Planned => "planned",
                    CommandStatus::Applied => "applied",
                });
                output.push('\n');
            }
            Ok(output.trim_end().to_string())
        }
        ProtocolResponse::Query(QueryResult::RuntimeLogs {
            revision,
            runtime_id,
            runtime_name,
            entries,
        }) => {
            let mut output = format!(
                "revision={} runtime={} name={} entries={}\n",
                revision.0,
                safe_cell(runtime_id.as_str()),
                safe_cell(runtime_name),
                entries.len()
            );
            output.push_str("SEQUENCE\tLEVEL\tMESSAGE\n");
            for entry in entries {
                output.push_str(&entry.sequence.to_string());
                output.push('\t');
                output.push_str(match entry.level {
                    leserpent_domain::RuntimeLogLevel::Trace => "trace",
                    leserpent_domain::RuntimeLogLevel::Debug => "debug",
                    leserpent_domain::RuntimeLogLevel::Info => "info",
                    leserpent_domain::RuntimeLogLevel::Warning => "warning",
                    leserpent_domain::RuntimeLogLevel::Error => "error",
                });
                output.push('\t');
                output.push_str(&safe_cell(&entry.message));
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

fn append_capability_summary(
    output: &mut String,
    capabilities: &leserpent_domain::RuntimeCapabilitySnapshot,
    observed_for_revision: Option<Revision>,
) {
    if capabilities.is_unobserved() {
        output.push_str("\ncapabilities=unobserved capabilities_observed_for_revision=none");
        return;
    }
    let observed_for_revision = observed_for_revision
        .map(|revision| revision.0.to_string())
        .unwrap_or_else(|| "legacy-unknown".into());
    output.push_str(&format!(
        "\ncapabilities=observed capabilities_observed_for_revision={} service={} version={} latest_snapshot={} authenticated_deployment={} serve_required={} external_sidecar_context={} endpoints={} extensions={}",
        observed_for_revision,
        safe_cell(&capabilities.service),
        safe_cell(&capabilities.version),
        capabilities.latest_snapshot,
        capabilities.authenticated_deployment,
        capabilities.serve_required,
        capabilities.external_sidecar_context,
        capabilities.endpoints.len(),
        capabilities.extensions.len(),
    ));
    if !capabilities.endpoints.is_empty() {
        output.push_str("\ncapability_endpoints=");
        output.push_str(
            &capabilities
                .endpoints
                .iter()
                .map(|endpoint| safe_cell(endpoint))
                .collect::<Vec<_>>()
                .join(","),
        );
    }
    if !capabilities.extensions.is_empty() {
        output.push_str("\ncapability_extensions=");
        output.push_str(
            &capabilities
                .extensions
                .iter()
                .map(|(name, enabled)| format!("{}={enabled}", safe_cell(name)))
                .collect::<Vec<_>>()
                .join(","),
        );
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
    use zeroize::Zeroizing;

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
    let mut encoded = Zeroizing::new(
        serde_json::to_vec(&AuthenticatedRequest { token, request })
            .map_err(|error| CliError::Protocol(error.to_string()))?,
    );
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
) -> Result<(RuntimeListFilter, Option<LocalExport>), CliError> {
    let mut filter = RuntimeListFilter::default();
    let mut export = None;
    while let Some(argument) = arguments.next() {
        if matches!(argument.as_str(), "--export-leselang" | "--export-plan") {
            let candidate = if argument == "--export-leselang" {
                LocalExport::Leselang
            } else {
                LocalExport::Plan
            };
            if export.replace(candidate).is_some() {
                return Err(CliError::Usage(
                    "only one local export format may be selected".into(),
                ));
            }
            continue;
        }
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
    Ok((filter, export))
}

fn parse_local_export(
    mut arguments: impl Iterator<Item = String>,
) -> Result<Option<LocalExport>, CliError> {
    let Some(argument) = arguments.next() else {
        return Ok(None);
    };
    let export = match argument.as_str() {
        "--export-leselang" => LocalExport::Leselang,
        "--export-plan" => LocalExport::Plan,
        _ => {
            return Err(CliError::Usage(format!(
                "unknown runtime inspect option '{argument}'"
            )));
        }
    };
    reject_trailing(arguments)?;
    Ok(Some(export))
}

fn parse_runtime_refresh(
    mut arguments: impl Iterator<Item = String>,
    command_name: &str,
) -> Result<RuntimeRefreshOptions, CliError> {
    let runtime_id = arguments
        .next()
        .ok_or_else(|| CliError::Usage(format!("{command_name} requires RUNTIME_ID")))?;
    let runtime_id =
        RuntimeId::new(runtime_id).map_err(|error| CliError::Usage(error.to_string()))?;
    let mut expected_revision = None;
    let mut dry_run = false;
    let mut confirmed = false;
    let mut idempotency_key = None;
    let mut export_leselang = false;
    let mut export_plan = false;
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--dry-run" if !dry_run => dry_run = true,
            "--yes" if !confirmed => confirmed = true,
            "--export-leselang" if !export_leselang => export_leselang = true,
            "--export-plan" if !export_plan => export_plan = true,
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
            | "--export-plan"
            | "--expected-revision"
            | "--idempotency-key" => {
                return Err(CliError::Usage(format!(
                    "{argument} was provided more than once"
                )));
            }
            _ => {
                return Err(CliError::Usage(format!(
                    "unknown {command_name} option '{argument}'"
                )));
            }
        }
    }
    if export_leselang
        && (export_plan
            || dry_run
            || confirmed
            || expected_revision.is_some()
            || idempotency_key.is_some())
    {
        return Err(CliError::Usage(
            "--export-leselang cannot be combined with execution options".into(),
        ));
    }
    if export_plan && idempotency_key.is_none() {
        return Err(CliError::Usage(
            "--export-plan requires --idempotency-key for deterministic output".into(),
        ));
    }
    if dry_run && confirmed {
        return Err(CliError::Usage(
            "--dry-run cannot be combined with --yes".into(),
        ));
    }
    if !export_leselang && !dry_run && !confirmed {
        return Err(CliError::Usage(format!(
            "{command_name} requires --dry-run or explicit --yes confirmation"
        )));
    }
    Ok(RuntimeRefreshOptions {
        runtime_id,
        expected_revision,
        dry_run,
        confirmed,
        idempotency_key,
        export_leselang,
        export_plan,
    })
}

fn parse_runtime_deploy(
    mut arguments: impl Iterator<Item = String>,
) -> Result<RuntimeDeployOptions, CliError> {
    let runtime_id = arguments
        .next()
        .ok_or_else(|| CliError::Usage("runtime deploy requires RUNTIME_ID".into()))?;
    let runtime_id =
        RuntimeId::new(runtime_id).map_err(|error| CliError::Usage(error.to_string()))?;
    let mut pipeline_kind = None;
    let mut target = None;
    let mut target_seen = false;
    let mut expected_revision = None;
    let mut dry_run = false;
    let mut confirmed = false;
    let mut idempotency_key = None;
    let mut export_leselang = false;
    let mut export_plan = false;
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--pipeline-kind" if pipeline_kind.is_none() => {
                let value = arguments
                    .next()
                    .ok_or_else(|| CliError::Usage("--pipeline-kind requires a value".into()))?;
                pipeline_kind = Some(value);
            }
            "--target" if !target_seen => {
                let value = arguments
                    .next()
                    .ok_or_else(|| CliError::Usage("--target requires a value".into()))?;
                target = Some(value);
                target_seen = true;
            }
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
            "--dry-run" if !dry_run => dry_run = true,
            "--yes" if !confirmed => confirmed = true,
            "--export-leselang" if !export_leselang => export_leselang = true,
            "--export-plan" if !export_plan => export_plan = true,
            "--pipeline-kind"
            | "--target"
            | "--expected-revision"
            | "--idempotency-key"
            | "--dry-run"
            | "--yes"
            | "--export-leselang"
            | "--export-plan" => {
                return Err(CliError::Usage(format!(
                    "{argument} was provided more than once"
                )));
            }
            _ => {
                return Err(CliError::Usage(format!(
                    "unknown runtime deploy option '{argument}'"
                )));
            }
        }
    }
    let pipeline_kind = pipeline_kind
        .ok_or_else(|| CliError::Usage("runtime deploy requires --pipeline-kind".into()))?;
    validate_deployment_intent(&pipeline_kind, target.as_deref()).map_err(|error| {
        let field = match error {
            leserpent_domain::DomainError::InvalidIdentifier { field } => field,
            _ => "deployment intent",
        };
        CliError::Usage(format!("invalid --{}", field.replace('_', "-")))
    })?;
    if export_leselang
        && (export_plan
            || dry_run
            || confirmed
            || expected_revision.is_some()
            || idempotency_key.is_some())
    {
        return Err(CliError::Usage(
            "--export-leselang cannot be combined with execution options".into(),
        ));
    }
    if export_plan && idempotency_key.is_none() {
        return Err(CliError::Usage(
            "--export-plan requires --idempotency-key for deterministic output".into(),
        ));
    }
    if dry_run && confirmed {
        return Err(CliError::Usage(
            "--dry-run cannot be combined with --yes".into(),
        ));
    }
    if !export_leselang && !dry_run && !confirmed {
        return Err(CliError::Usage(
            "runtime deploy requires --dry-run or explicit --yes confirmation".into(),
        ));
    }
    Ok(RuntimeDeployOptions {
        runtime_id,
        pipeline_kind,
        target,
        expected_revision,
        dry_run,
        confirmed,
        idempotency_key,
        export_leselang,
        export_plan,
    })
}

fn parse_runtime_watch(
    mut arguments: impl Iterator<Item = String>,
) -> Result<RuntimeWatchOptions, CliError> {
    let runtime_id = arguments
        .next()
        .ok_or_else(|| CliError::Usage("runtime watch requires RUNTIME_ID".into()))?;
    let runtime_id =
        RuntimeId::new(runtime_id).map_err(|error| CliError::Usage(error.to_string()))?;
    let mut count = 20u16;
    let mut interval_ms = 1_000u64;
    let mut count_set = false;
    let mut interval_set = false;
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--count" if !count_set => {
                count_set = true;
                let value = arguments
                    .next()
                    .ok_or_else(|| CliError::Usage("--count requires an integer".into()))?;
                count = value.parse().map_err(|_| {
                    CliError::Usage("--count requires an integer from 1 to 1000".into())
                })?;
                if !(1..=1_000).contains(&count) {
                    return Err(CliError::Usage(
                        "--count requires an integer from 1 to 1000".into(),
                    ));
                }
            }
            "--interval-ms" if !interval_set => {
                interval_set = true;
                let value = arguments
                    .next()
                    .ok_or_else(|| CliError::Usage("--interval-ms requires an integer".into()))?;
                interval_ms = value.parse().map_err(|_| {
                    CliError::Usage("--interval-ms requires an integer from 50 to 60000".into())
                })?;
                if !(50..=60_000).contains(&interval_ms) {
                    return Err(CliError::Usage(
                        "--interval-ms requires an integer from 50 to 60000".into(),
                    ));
                }
            }
            "--count" | "--interval-ms" => {
                return Err(CliError::Usage(format!(
                    "{argument} was provided more than once"
                )));
            }
            _ => {
                return Err(CliError::Usage(format!(
                    "unknown runtime watch option '{argument}'"
                )));
            }
        }
    }
    Ok(RuntimeWatchOptions {
        runtime_id,
        count,
        interval_ms,
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
    use std::collections::BTreeMap;

    use leserpent_domain::{Command, Query};

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
        let Query::RuntimeList { filter } = query.query else {
            panic!("runtime list must produce a list query");
        };
        assert_eq!(filter.environment.as_deref(), Some("production"));
        assert_eq!(filter.role.as_deref(), Some("edge"));
        assert!(query.capabilities.contains(CAPABILITY_RUNTIME_READ));
    }

    #[test]
    fn inspect_renders_bounded_capabilities_without_adapter_secrets() {
        let mut control = leserpent_domain::InMemoryControlPlane::default();
        let runtime_id = RuntimeId::new("runtime-a").unwrap();
        control.register_runtime(
            runtime_id.clone(),
            "Runtime A",
            "https://configured-origin.invalid",
        );
        control
            .complete_runtime_capability_refresh(
                &runtime_id,
                Revision(1),
                leserpent_domain::RuntimeCapabilitySnapshot {
                    source: "gewyvern-api".into(),
                    service: "gewyvern-api".into(),
                    version: "1.2.0".into(),
                    latest_snapshot: true,
                    authenticated_deployment: true,
                    serve_required: true,
                    external_sidecar_context: false,
                    target_path_segment_encoding: "percent-encoding".into(),
                    target_direct_path_chars: "A-Za-z0-9._~:".into(),
                    endpoints: vec!["/v1/capabilities".into(), "/v1/deployments".into()],
                    extensions: BTreeMap::from([
                        ("protocol_catalog".into(), true),
                        ("training".into(), false),
                    ]),
                },
            )
            .unwrap();
        let query = control
            .query(leserpent_domain::QueryEnvelope {
                schema_version: leserpent_domain::DOMAIN_SCHEMA_VERSION,
                principal: Principal {
                    id: "operator".into(),
                },
                capabilities: CapabilitySet::new([CAPABILITY_RUNTIME_READ]),
                query: Query::RuntimeInspect { runtime_id },
            })
            .unwrap();
        let rendered = render_response(
            &ResponseEnvelope {
                schema_version: PROTOCOL_SCHEMA_VERSION,
                response: ProtocolResponse::Query(query),
            },
            false,
        )
        .unwrap();

        assert!(rendered.contains("capabilities=observed capabilities_observed_for_revision=1"));
        assert!(rendered.contains("service=gewyvern-api version=1.2.0"));
        assert!(rendered.contains("capability_endpoints=/v1/capabilities,/v1/deployments"));
        assert!(rendered.contains("capability_extensions=protocol_catalog=true,training=false"));
        assert!(!rendered.contains("Authorization"));
        assert!(!rendered.contains("secret"));
    }

    #[test]
    fn capability_summary_marks_legacy_observations_without_inventing_a_revision() {
        let mut output = String::new();
        append_capability_summary(
            &mut output,
            &leserpent_domain::RuntimeCapabilitySnapshot {
                source: "gewyvern-api".into(),
                service: "gewyvern-api".into(),
                version: "1.2.0".into(),
                latest_snapshot: true,
                authenticated_deployment: false,
                serve_required: true,
                external_sidecar_context: false,
                target_path_segment_encoding: "percent-encoding".into(),
                target_direct_path_chars: "A-Za-z0-9._~:".into(),
                endpoints: Vec::new(),
                extensions: BTreeMap::new(),
            },
            None,
        );

        assert!(output.contains("capabilities_observed_for_revision=legacy-unknown"));
    }

    #[test]
    fn parser_builds_runtime_inspect_request() {
        let options = parse_args(
            ["runtime", "inspect", "runtime-a"]
                .into_iter()
                .map(str::to_string),
            Some("/tmp/leserpent.sock".into()),
            Some("operator-a".into()),
        )
        .unwrap();
        let ProtocolRequest::Query(query) = request_for(&options).unwrap().request else {
            panic!("runtime inspect must produce a query request");
        };
        assert!(matches!(
            query.query,
            Query::RuntimeInspect { runtime_id } if runtime_id.as_str() == "runtime-a"
        ));
        assert!(query.capabilities.contains(CAPABILITY_RUNTIME_READ));
    }

    #[test]
    fn parser_and_exports_cover_runtime_history() {
        let options = parse_args(
            ["runtime", "history", "runtime-a"]
                .into_iter()
                .map(str::to_string),
            Some("/tmp/leserpent.sock".into()),
            Some("operator-a".into()),
        )
        .unwrap();
        let ProtocolRequest::Query(query) = request_for(&options).unwrap().request else {
            panic!("runtime history must produce a query request");
        };
        assert!(matches!(
            query.query,
            Query::RuntimeHistory { runtime_id } if runtime_id.as_str() == "runtime-a"
        ));

        let source = parse_args(
            ["runtime", "history", "runtime-a", "--export-leselang"]
                .into_iter()
                .map(str::to_string),
            None,
            None,
        )
        .unwrap();
        assert_eq!(
            export_leselang(&source).as_deref(),
            Some("fn main() = runtime.history(runtime_id: \"runtime-a\")\n")
        );
    }

    #[test]
    fn parser_and_exports_cover_bounded_runtime_logs() {
        let options = parse_args(
            ["runtime", "logs", "runtime-a"]
                .into_iter()
                .map(str::to_string),
            Some("/tmp/leserpent.sock".into()),
            Some("operator-a".into()),
        )
        .unwrap();
        let ProtocolRequest::Query(query) = request_for(&options).unwrap().request else {
            panic!("runtime logs must produce a query request");
        };
        assert!(matches!(
            query.query,
            Query::RuntimeLogs {
                runtime_id,
                after_sequence: None,
                limit: leserpent_domain::MAX_RUNTIME_LOG_QUERY_ENTRIES,
            } if runtime_id.as_str() == "runtime-a"
        ));

        let source = parse_args(
            ["runtime", "logs", "runtime-a", "--export-leselang"]
                .into_iter()
                .map(str::to_string),
            None,
            None,
        )
        .unwrap();
        assert_eq!(
            export_leselang(&source).as_deref(),
            Some("fn main() = runtime.logs(runtime_id: \"runtime-a\")\n")
        );

        let plan = parse_args(
            ["runtime", "logs", "runtime-a", "--export-plan"]
                .into_iter()
                .map(str::to_string),
            None,
            None,
        )
        .unwrap();
        assert!(export_plan(&plan).unwrap().is_some());
    }

    #[test]
    fn runtime_watch_is_bounded_and_reuses_inspect_query() {
        let options = parse_args(
            [
                "runtime",
                "watch",
                "runtime-a",
                "--count",
                "3",
                "--interval-ms",
                "50",
            ]
            .into_iter()
            .map(str::to_string),
            Some("/tmp/leserpent.sock".into()),
            Some("operator-a".into()),
        )
        .unwrap();
        assert!(matches!(
            &options.command,
            CliCommand::RuntimeWatch(RuntimeWatchOptions {
                runtime_id,
                count: 3,
                interval_ms: 50,
            }) if runtime_id.as_str() == "runtime-a"
        ));
        let ProtocolRequest::Query(query) = request_for(&options).unwrap().request else {
            panic!("runtime watch must reuse an inspect query");
        };
        assert!(matches!(
            query.query,
            Query::RuntimeInspect { runtime_id } if runtime_id.as_str() == "runtime-a"
        ));
        for arguments in [
            vec!["runtime", "watch", "runtime-a", "--count", "0"],
            vec!["runtime", "watch", "runtime-a", "--count", "1001"],
            vec!["runtime", "watch", "runtime-a", "--interval-ms", "49"],
        ] {
            assert!(
                parse_args(
                    arguments.into_iter().map(str::to_string),
                    Some("/tmp/leserpent.sock".into()),
                    None,
                )
                .is_err()
            );
        }
    }

    #[test]
    fn capabilities_refresh_exports_canonical_leselang() {
        let options = parse_args(
            [
                "runtime",
                "refresh-capabilities",
                "runtime-a",
                "--export-leselang",
            ]
            .into_iter()
            .map(str::to_string),
            None,
            None,
        )
        .unwrap();
        let source = export_leselang(&options).unwrap();
        assert_eq!(
            source,
            "fn main() = runtime.refresh_capabilities(runtime_id: \"runtime-a\")\n"
        );
        let program = leselang_hir::lower(&parse_leselang(&source)).unwrap();
        assert!(matches!(
            program.function.effect,
            leselang_hir::Effect::RuntimeCapabilitiesRefresh { runtime_id }
                if runtime_id.as_str() == "runtime-a"
        ));
    }

    #[test]
    fn deployment_parser_requires_explicit_confirmation_and_canonicalizes_export() {
        let unconfirmed = parse_args(
            [
                "runtime",
                "deploy",
                "runtime-a",
                "--pipeline-kind",
                "http/request",
            ]
            .into_iter()
            .map(str::to_string),
            Some("/tmp/leserpent.sock".into()),
            None,
        );
        assert!(
            matches!(unconfirmed, Err(CliError::Usage(ref error)) if error.contains("explicit --yes"))
        );

        let exported = parse_args(
            [
                "runtime",
                "deploy",
                "runtime-a",
                "--pipeline-kind",
                "http/request",
                "--target",
                "pid:42",
                "--export-leselang",
            ]
            .into_iter()
            .map(str::to_string),
            None,
            None,
        )
        .unwrap();
        assert_eq!(
            export_leselang(&exported).as_deref(),
            Some(
                "fn main() = runtime.deploy(\n  runtime_id: \"runtime-a\",\n  pipeline_kind: \"http/request\",\n  target: \"pid:42\",\n)\n"
            )
        );
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
    fn parser_selects_exactly_one_remote_transport() {
        let options = parse_args_with_remote(
            ["--json", "health"].into_iter().map(str::to_string),
            None,
            Some("https://localhost:9443".into()),
            Some("/tmp/leserpent-ca.pem".into()),
            Some("remote-operator".into()),
        )
        .unwrap();
        assert!(options.socket.is_none());
        assert_eq!(
            options
                .remote
                .as_ref()
                .map(|remote| remote.endpoint.as_str()),
            Some("https://localhost:9443")
        );
        assert!(
            parse_args_with_remote(
                [
                    "--socket",
                    "/tmp/leserpent.sock",
                    "--remote",
                    "https://localhost:9443",
                    "--remote-ca",
                    "/tmp/ca.pem",
                    "health",
                ]
                .into_iter()
                .map(str::to_string),
                None,
                None,
                None,
                None,
            )
            .is_err()
        );
        assert!(
            parse_args_with_remote(
                ["--remote", "https://localhost:9443", "health"]
                    .into_iter()
                    .map(str::to_string),
                None,
                None,
                None,
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

    #[test]
    fn refresh_plan_export_is_deterministic_and_uses_shared_lowering() {
        let options = parse_args(
            [
                "runtime",
                "refresh",
                "runtime-a",
                "--dry-run",
                "--expected-revision",
                "7",
                "--idempotency-key",
                "plan-a",
                "--export-plan",
            ]
            .into_iter()
            .map(str::to_string),
            None,
            Some("operator-a".into()),
        )
        .unwrap();
        let first = export_plan(&options).unwrap().unwrap();
        let second = export_plan(&options).unwrap().unwrap();
        assert_eq!(first, second);
        let plan = leselang_command::decode_plan(first.as_bytes()).unwrap();
        let PlannedOperation::Command(command) = plan.operation else {
            panic!("runtime refresh plan must contain a command");
        };
        assert_eq!(command.command_id.as_str(), "plan-a");
        assert_eq!(command.expected_revision, Some(Revision(7)));
        assert!(command.dry_run);
        assert_eq!(command.origin, CommandOrigin::Cli);
        assert!(matches!(
            command.command,
            Command::RuntimeRefresh { runtime_id } if runtime_id.as_str() == "runtime-a"
        ));
    }

    #[test]
    fn refresh_plan_export_requires_explicit_stable_identity() {
        let parsed = parse_args(
            [
                "runtime",
                "refresh",
                "runtime-a",
                "--dry-run",
                "--export-plan",
            ]
            .into_iter()
            .map(str::to_string),
            None,
            None,
        );
        assert_eq!(
            parsed,
            Err(CliError::Usage(
                "--export-plan requires --idempotency-key for deterministic output".into()
            ))
        );
    }

    #[test]
    fn read_queries_export_canonical_leselang_without_daemon_configuration() {
        let list = parse_args(
            [
                "runtime",
                "list",
                "--environment",
                " production ",
                "--export-leselang",
            ]
            .into_iter()
            .map(str::to_string),
            None,
            None,
        )
        .unwrap();
        assert_eq!(
            export_leselang(&list).as_deref(),
            Some(
                "fn main() = runtime.list(\n  environment: \"production\",\n  cluster: none,\n  role: none,\n)\n"
            )
        );

        let inspect = parse_args(
            ["runtime", "inspect", "runtime-a", "--export-leselang"]
                .into_iter()
                .map(str::to_string),
            None,
            None,
        )
        .unwrap();
        assert_eq!(
            export_leselang(&inspect).as_deref(),
            Some("fn main() = runtime.inspect(runtime_id: \"runtime-a\")\n")
        );
    }

    #[test]
    fn read_query_plan_export_is_valid_and_export_formats_are_exclusive() {
        let inspect = parse_args(
            ["runtime", "inspect", "runtime-a", "--export-plan"]
                .into_iter()
                .map(str::to_string),
            None,
            Some("operator-a".into()),
        )
        .unwrap();
        let encoded = export_plan(&inspect).unwrap().unwrap();
        let plan = leselang_command::decode_plan(encoded.as_bytes()).unwrap();
        assert!(matches!(
            plan.operation,
            PlannedOperation::Query(leserpent_domain::QueryEnvelope {
                query: Query::RuntimeInspect { runtime_id },
                ..
            }) if runtime_id.as_str() == "runtime-a"
        ));

        assert!(
            parse_args(
                ["runtime", "list", "--export-plan", "--export-leselang",]
                    .into_iter()
                    .map(str::to_string),
                None,
                None,
            )
            .is_err()
        );
    }
}
