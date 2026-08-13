use crate::cli_validation::{CliValidationInput, validate_cli_options};
use crate::external_analysis::ExternalAnalysisConfig;
use crate::data_api::normalize_api_admin_token;
use crate::runtime_events::EVENT_DSL_COMPILE_FAILED;
use crate::runtime_logging::{LogLevel, LoggingConfig, log_error_event};
use crate::{UiLocale, usage};
use gewyvern::dsl::compile_file;
use gewyvern::protocol_profiles::{ResolvedProtocolProfile, protocol_dsl_path};
use gewyvern::template::{TemplateBinding, handshake_debug_template, udp_debug_template};

mod trusted_path;
use self::trusted_path::next_trusted_path_value;

const CLI_SOCKET_TARGET_MAX_LEN: usize = 4096;

#[derive(Debug)]
pub(crate) struct Cli {
    pub(crate) demo_mode: DemoMode,
    pub(crate) template_mode: TemplateMode,
    pub(crate) dsl_path: Option<String>,
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) protocol: Option<String>,
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) entry: Option<String>,
    pub(crate) scan_all: bool,
    pub(crate) protocol_set_path: Option<String>,
    pub(crate) list_protocols: bool,
    pub(crate) list_history: bool,
    pub(crate) list_entries: Option<String>,
    pub(crate) ingest_mode: IngestMode,
    pub(crate) pid: Option<u32>,
    pub(crate) diagnostics: bool,
    pub(crate) findings: bool,
    pub(crate) http_transactions: bool,
    pub(crate) debugger_console: bool,
    pub(crate) debug_session: bool,
    pub(crate) serve: bool,
    pub(crate) api_socket: Option<String>,
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) allow_remote_api: bool,
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) api_admin_token: Option<String>,
    pub(crate) max_sessions: Option<usize>,
    pub(crate) json: bool,
    pub(crate) report_format: Option<ReportFormat>,
    pub(crate) summary_only: bool,
    pub(crate) out_path: Option<String>,
    pub(crate) socket_target: Option<SocketTarget>,
    pub(crate) external_engine_bin: Option<String>,
    pub(crate) external_engine_worker: Option<String>,
    pub(crate) external_engine_python_bin: Option<String>,
    pub(crate) log_level: LogLevel,
    pub(crate) log_to_stderr: bool,
    pub(crate) log_file: Option<String>,
    pub(crate) log_max_bytes: usize,
    pub(crate) log_max_files: usize,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct CliDefaults {
    pub(crate) serve: Option<bool>,
    pub(crate) api_socket: Option<String>,
    pub(crate) allow_remote_api: Option<bool>,
    pub(crate) api_admin_token: Option<String>,
    pub(crate) max_sessions: Option<usize>,
    pub(crate) ingest_mode: Option<IngestMode>,
    pub(crate) socket_target: Option<SocketTarget>,
    pub(crate) external_engine_bin: Option<String>,
    pub(crate) external_engine_worker: Option<String>,
    pub(crate) external_engine_python_bin: Option<String>,
    pub(crate) log_level: Option<LogLevel>,
    pub(crate) log_to_stderr: Option<bool>,
    pub(crate) log_file: Option<String>,
    pub(crate) log_max_bytes: Option<usize>,
    pub(crate) log_max_files: Option<usize>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DemoMode {
    Tcp,
    Udp,
    Both,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TemplateMode {
    Tcp,
    Udp,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ReportFormat {
    Json,
    Html,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum IngestMode {
    LocalAdvisory,
    RemoteAdvisory,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum SocketTarget {
    Unix(String),
    Tcp(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ScanTarget {
    pub(crate) protocol: String,
    pub(crate) entry: String,
    pub(crate) dsl_path: String,
}

impl ScanTarget {
    pub(crate) fn from_resolved(profile: ResolvedProtocolProfile) -> Self {
        Self {
            protocol: profile.protocol.to_string(),
            entry: profile.entry.to_string(),
            dsl_path: profile.dsl_path.to_string(),
        }
    }

    pub(crate) fn label(&self) -> String {
        format!("scan:{}:{}", self.protocol, self.entry)
    }

    pub(crate) fn binding(&self) -> TemplateBinding {
        let locale = UiLocale::detect();
        compile_file(&self.dsl_path).unwrap_or_else(|err| {
            log_error_event(
                "dsl",
                EVENT_DSL_COMPILE_FAILED,
                &[
                    ("path", self.dsl_path.clone()),
                    ("error", format!("{err:?}")),
                ],
                "failed to compile dsl binding",
            );
            eprintln!(
                "{}",
                locale.msgf("dsl_compile_failed", &format!("{err:?}"), None)
            );
            std::process::exit(2);
        })
    }
}

impl DemoMode {
    pub(crate) fn from_str(value: &str) -> Result<Self, String> {
        let locale = UiLocale::detect();
        match value {
            "tcp" => Ok(Self::Tcp),
            "udp" => Ok(Self::Udp),
            "both" => Ok(Self::Both),
            other => Err(locale.msgf("unsupported_demo", other, None)),
        }
    }

    pub(crate) fn includes_tcp(self) -> bool {
        matches!(self, Self::Tcp | Self::Both)
    }

    pub(crate) fn includes_udp(self) -> bool {
        matches!(self, Self::Udp | Self::Both)
    }
}

impl TemplateMode {
    pub(crate) fn from_str(value: &str) -> Result<Self, String> {
        let locale = UiLocale::detect();
        match value {
            "tcp" => Ok(Self::Tcp),
            "udp" => Ok(Self::Udp),
            other => Err(locale.msgf("unsupported_template", other, None)),
        }
    }

    pub(crate) fn template(self) -> gewyvern::template::Template {
        match self {
            Self::Tcp => handshake_debug_template(),
            Self::Udp => udp_debug_template(),
        }
    }
}

impl IngestMode {
    pub(crate) fn from_str(value: &str) -> Result<Self, String> {
        let locale = UiLocale::detect();
        match value {
            "local-advisory" | "local" => Ok(Self::LocalAdvisory),
            "remote-advisory" | "remote" => Ok(Self::RemoteAdvisory),
            other => Err(locale.msgf("unsupported_ingest_mode", other, None)),
        }
    }
}

impl ReportFormat {
    pub(crate) fn from_str(value: &str) -> Result<Self, String> {
        match value {
            "json" => Ok(Self::Json),
            "html" => Ok(Self::Html),
            other => Err(format!("unsupported report format '{other}'")),
        }
    }
}

impl Cli {
    pub(crate) fn dsl_binding(&self) -> Option<TemplateBinding> {
        let locale = UiLocale::detect();
        self.dsl_path.as_deref().map(|path| {
            compile_file(path).unwrap_or_else(|err| {
                log_error_event(
                    "dsl",
                    EVENT_DSL_COMPILE_FAILED,
                    &[("path", path.to_string()), ("error", format!("{err:?}"))],
                    "failed to compile dsl binding",
                );
                eprintln!(
                    "{}",
                    locale.msgf("dsl_compile_failed", &format!("{err:?}"), None)
                );
                std::process::exit(2);
            })
        })
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn from_args<I>(args: I) -> Result<Self, String>
    where
        I: IntoIterator<Item = String>,
    {
        Self::from_args_with_defaults(args, CliDefaults::default())
    }

    pub(crate) fn from_args_with_defaults<I>(args: I, defaults: CliDefaults) -> Result<Self, String>
    where
        I: IntoIterator<Item = String>,
    {
        let locale = UiLocale::detect();
        let mut demo_mode = DemoMode::Both;
        let mut template_mode = TemplateMode::Tcp;
        let mut dsl_path = None;
        let mut protocol = None;
        let mut entry = None;
        let mut scan_all = false;
        let mut protocol_set_path = None;
        let mut list_protocols = false;
        let mut list_history = false;
        let mut list_entries = None;
        let mut ingest_mode = defaults.ingest_mode.unwrap_or(IngestMode::LocalAdvisory);
        let mut pid = None;
        let mut diagnostics = false;
        let mut findings = false;
        let mut http_transactions = false;
        let mut debugger_console = false;
        let mut debug_session = false;
        let mut serve = defaults.serve.unwrap_or(false);
        let mut api_socket = defaults
            .api_socket
            .map(|value| validate_cli_socket_target("api socket", value))
            .transpose()?;
        let mut allow_remote_api = defaults.allow_remote_api.unwrap_or(false);
        let mut api_admin_token = resolve_api_admin_token(
            defaults.api_admin_token,
            std::env::var("GEWY_API_ADMIN_TOKEN").ok(),
        );
        let mut max_sessions = defaults.max_sessions;
        let mut json = false;
        let mut report_format = None;
        let mut summary_only = false;
        let mut out_path = None;
        let mut socket_target = defaults
            .socket_target
            .map(validate_default_socket_target)
            .transpose()?;
        let mut external_engine_bin = defaults.external_engine_bin;
        let mut external_engine_worker = defaults.external_engine_worker;
        let mut external_engine_python_bin = defaults.external_engine_python_bin;
        let mut log_level = defaults.log_level.unwrap_or(LogLevel::Warn);
        let mut log_to_stderr = defaults.log_to_stderr.unwrap_or(true);
        let mut log_file = defaults.log_file;
        let log_defaults = LoggingConfig::default();
        let log_max_bytes = defaults.log_max_bytes.unwrap_or(log_defaults.max_bytes);
        let log_max_files = defaults.log_max_files.unwrap_or(log_defaults.max_files);
        let mut args = args.into_iter();

        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--demo" => {
                    let value = args
                        .next()
                        .ok_or_else(|| locale.msgf("missing_demo", "", None))?;
                    demo_mode = DemoMode::from_str(&value)?;
                }
                "--json" => json = true,
                "--report-format" => {
                    let value = args
                        .next()
                        .ok_or_else(|| "missing report format".to_string())?;
                    report_format = Some(ReportFormat::from_str(&value)?);
                }
                "--serve" => serve = true,
                "--allow-remote-api" => allow_remote_api = true,
                "--api-admin-token" => {
                    api_admin_token = Some(
                        args.next()
                            .ok_or_else(|| "missing --api-admin-token value".to_string())?,
                    );
                }
                "--api-socket" => {
                    let value = args
                        .next()
                        .ok_or_else(|| locale.msgf("missing_api_socket", "", None))?;
                    api_socket = Some(validate_cli_socket_target("api socket", value)?);
                }
                "--findings" => findings = true,
                "--http-transactions" => http_transactions = true,
                "--debugger-console" => debugger_console = true,
                "--debug-session" => debug_session = true,
                "--max-sessions" => {
                    let value = args
                        .next()
                        .ok_or_else(|| locale.msgf("missing_max_sessions", "", None))?;
                    max_sessions = Some(
                        value
                            .parse::<usize>()
                            .map_err(|_| locale.msgf("invalid_max_sessions", "", None))?,
                    );
                }
                "--summary-only" => summary_only = true,
                "--template" => {
                    let value = args
                        .next()
                        .ok_or_else(|| locale.msgf("missing_template", "", None))?;
                    template_mode = TemplateMode::from_str(&value)?;
                }
                "--dsl" => {
                    dsl_path = Some(
                        args.next()
                            .ok_or_else(|| locale.msgf("missing_dsl", "", None))?,
                    );
                }
                "--protocol" => {
                    protocol = Some(
                        args.next()
                            .ok_or_else(|| locale.msgf("missing_protocol", "", None))?,
                    );
                }
                "--scan-all" => scan_all = true,
                "--protocol-set" => {
                    protocol_set_path = Some(args.next().ok_or_else(|| {
                        "--protocol-set requires a protocol set file path".to_string()
                    })?);
                }
                "--entry" => {
                    entry = Some(
                        args.next()
                            .ok_or_else(|| locale.msgf("missing_entry", "", None))?,
                    );
                }
                "--list-protocols" => list_protocols = true,
                "--list-history" => list_history = true,
                "--allow-remote-socket" => ingest_mode = IngestMode::RemoteAdvisory,
                "--ingest-mode" => {
                    let value = args
                        .next()
                        .ok_or_else(|| locale.msgf("missing_ingest_mode", "", None))?;
                    ingest_mode = IngestMode::from_str(&value)?;
                }
                "--list-entries" => {
                    list_entries = Some(
                        args.next()
                            .ok_or_else(|| locale.msgf("missing_protocol", "", None))?,
                    );
                }
                "--socket-trust" => {
                    let value = args
                        .next()
                        .ok_or_else(|| locale.msgf("missing_socket_trust", "", None))?;
                    ingest_mode = match value.as_str() {
                        "trusted-local" | "local" => IngestMode::LocalAdvisory,
                        "unsafe-remote" | "remote" => IngestMode::RemoteAdvisory,
                        other => {
                            return Err(locale.msgf("unsupported_socket_trust", other, None));
                        }
                    };
                }
                "--pid" => {
                    let value = args
                        .next()
                        .ok_or_else(|| locale.msgf("missing_pid", "", None))?;
                    pid = Some(
                        value
                            .parse::<u32>()
                            .ok()
                            .filter(|pid| *pid > 0)
                            .ok_or_else(|| locale.msgf("invalid_pid", "", None))?,
                    );
                }
                "--diagnostics" => diagnostics = true,
                "--unix-socket" => {
                    let value = args
                        .next()
                        .ok_or_else(|| locale.msgf("missing_unix_socket", "", None))?;
                    let value = validate_cli_socket_target("unix socket path", value)?;
                    socket_target =
                        Some(SocketTarget::Unix(value));
                }
                "--tcp-socket" => {
                    let value = args
                        .next()
                        .ok_or_else(|| locale.msgf("missing_tcp_socket", "", None))?;
                    let value = validate_cli_socket_target("tcp socket target", value)?;
                    socket_target =
                        Some(SocketTarget::Tcp(value));
                }
                "--out" => {
                    out_path = Some(
                        args.next()
                            .ok_or_else(|| locale.msgf("missing_out", "", None))?,
                    );
                }
                "--external-engine-bin" => {
                    external_engine_bin =
                        Some(next_trusted_path_value(&mut args, "--external-engine-bin")?);
                }
                "--external-engine-worker" => {
                    external_engine_worker = Some(next_trusted_path_value(
                        &mut args,
                        "--external-engine-worker",
                    )?);
                }
                "--external-engine-python-bin" => {
                    external_engine_python_bin = Some(next_trusted_path_value(
                        &mut args,
                        "--external-engine-python-bin",
                    )?);
                }
                "--log-level" => {
                    let value = args
                        .next()
                        .ok_or_else(|| "missing value for --log-level".to_string())?;
                    log_level = LogLevel::from_str(&value)?;
                }
                "--log-file" => {
                    log_file = Some(
                        args.next()
                            .ok_or_else(|| "missing value for --log-file".to_string())?,
                    );
                }
                "--log-stderr" => log_to_stderr = true,
                "--no-log-stderr" => log_to_stderr = false,
                "--help" | "-h" => return Err(usage().into()),
                other => return Err(locale.msgf("unknown_argument", other, None)),
            }
        }

        validate_cli_options(CliValidationInput {
            summary_only,
            json,
            report_format,
            diagnostics,
            dsl_path: dsl_path.is_some(),
            socket_target: socket_target.as_ref(),
            serve,
            findings,
            http_transactions,
            debugger_console,
            debug_session,
            scan_all,
            protocol: protocol.is_some(),
            entry: entry.is_some(),
            protocol_set_path: protocol_set_path.is_some(),
            list_protocols,
            list_history,
            list_entries: list_entries.is_some(),
            pid: pid.is_some(),
            demo_mode,
            api_socket: api_socket.as_deref(),
            allow_remote_api,
            api_admin_token: api_admin_token.as_deref(),
            ingest_mode,
            external_engine_bin: external_engine_bin.is_some(),
            external_engine_worker: external_engine_worker.is_some(),
            external_engine_python_bin: external_engine_python_bin.is_some(),
            locale,
        })?;

        if let Some(protocol_name) = protocol.as_deref() {
            let built_in_path = protocol_dsl_path(protocol_name, entry.as_deref())
                .ok_or_else(|| locale.msgf("unsupported_protocol", protocol_name, None))?;
            dsl_path = Some(built_in_path.to_string());
        }

        Ok(Self {
            demo_mode,
            template_mode,
            dsl_path,
            protocol,
            entry,
            scan_all,
            protocol_set_path,
            list_protocols,
            list_history,
            list_entries,
            ingest_mode,
            pid,
            diagnostics,
            findings,
            http_transactions,
            debugger_console,
            debug_session,
            serve,
            api_socket,
            allow_remote_api,
            api_admin_token,
            max_sessions,
            json,
            report_format,
            summary_only,
            out_path,
            socket_target,
            external_engine_bin,
            external_engine_worker,
            external_engine_python_bin,
            log_level,
            log_to_stderr,
            log_file,
            log_max_bytes,
            log_max_files,
        })
    }

    pub(crate) fn external_analysis_config(&self) -> Option<ExternalAnalysisConfig> {
        self.external_engine_bin
            .as_ref()
            .map(|engine_bin| ExternalAnalysisConfig {
                engine_bin: engine_bin.clone(),
                python_worker: self.external_engine_worker.clone(),
                python_bin: self.external_engine_python_bin.clone(),
            })
    }

    pub(crate) fn logging_config(&self) -> LoggingConfig {
        LoggingConfig {
            level: self.log_level,
            log_to_stderr: self.log_to_stderr,
            log_file: self.log_file.as_ref().map(Into::into),
            max_bytes: self.log_max_bytes,
            max_files: self.log_max_files,
        }
    }
}

pub(crate) fn resolve_api_admin_token(
    configured: Option<String>,
    environment: Option<String>,
) -> Option<String> {
    configured.or_else(|| environment.map(|token| token.to_string())).and_then(|token| {
        let token = token.trim().to_string();
        if token.is_empty() {
            None
        } else {
            normalize_api_admin_token(&token)
        }
    })
}

fn validate_cli_socket_target(name: &str, value: String) -> Result<String, String> {
    if value.trim() != value {
        return Err(format!(
            "{name} must not contain leading or trailing whitespace"
        ));
    }
    if value.is_empty() {
        return Err(format!("{name} must not be empty"));
    }
    if value.len() > CLI_SOCKET_TARGET_MAX_LEN {
        return Err(format!("{name} is too long for a socket target"));
    }
    if value.bytes().any(|byte| byte.is_ascii_control()) {
        return Err(format!("{name} must not contain control characters"));
    }
    Ok(value)
}

fn validate_default_socket_target(target: SocketTarget) -> Result<SocketTarget, String> {
    match target {
        SocketTarget::Unix(path) => Ok(SocketTarget::Unix(validate_cli_socket_target(
            "unix socket path",
            path,
        )?)),
        SocketTarget::Tcp(addr) => Ok(SocketTarget::Tcp(validate_cli_socket_target(
            "tcp socket target",
            addr,
        )?)),
    }
}
