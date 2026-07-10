use crate::cli::DemoMode;
use crate::{
    IngestMode, ReportFormat, SocketTarget, UiLocale, api_socket_addr_is_local,
    socket_target_is_local,
};

pub(crate) struct CliValidationInput<'a> {
    pub(crate) summary_only: bool,
    pub(crate) json: bool,
    pub(crate) report_format: Option<ReportFormat>,
    pub(crate) diagnostics: bool,
    pub(crate) dsl_path: bool,
    pub(crate) socket_target: Option<&'a SocketTarget>,
    pub(crate) serve: bool,
    pub(crate) findings: bool,
    pub(crate) http_transactions: bool,
    pub(crate) debugger_console: bool,
    pub(crate) debug_session: bool,
    pub(crate) scan_all: bool,
    pub(crate) protocol: bool,
    pub(crate) entry: bool,
    pub(crate) protocol_set_path: bool,
    pub(crate) list_protocols: bool,
    pub(crate) list_history: bool,
    pub(crate) list_entries: bool,
    pub(crate) pid: bool,
    pub(crate) demo_mode: DemoMode,
    pub(crate) api_socket: Option<&'a str>,
    pub(crate) allow_remote_api: bool,
    pub(crate) ingest_mode: IngestMode,
    pub(crate) external_engine_bin: bool,
    pub(crate) external_engine_worker: bool,
    pub(crate) external_engine_python_bin: bool,
    pub(crate) locale: UiLocale,
}

pub(crate) fn validate_cli_options(input: CliValidationInput<'_>) -> Result<(), String> {
    macro_rules! reject {
        ($condition:expr, $message:expr) => {
            if $condition {
                return Err($message.into());
            }
        };
    }
    reject!(
        input.summary_only && !input.json && input.report_format.is_none(),
        input.locale.msg("summary_only_requires_json")
    );
    reject!(
        input.diagnostics && !input.dsl_path,
        input.locale.msg("diagnostics_requires_dsl")
    );
    reject!(
        input.diagnostics && input.socket_target.is_some(),
        input.locale.msg("diagnostics_socket_conflict")
    );
    reject!(
        input.diagnostics && input.serve,
        input.locale.msg("diagnostics_serve_conflict")
    );
    reject!(
        input.diagnostics && input.findings,
        input.locale.msg("findings_diagnostics_conflict")
    );
    reject!(
        input.diagnostics && input.http_transactions,
        input.locale.msg("findings_diagnostics_conflict")
    );

    let debugger_mode = input.debugger_console || input.debug_session;
    let debugger_flag = if input.debug_session {
        "--debug-session"
    } else {
        "--debugger-console"
    };
    reject!(
        debugger_mode && input.diagnostics,
        format!("{debugger_flag} cannot be combined with --diagnostics")
    );
    reject!(
        debugger_mode && input.findings,
        format!("{debugger_flag} cannot be combined with --findings")
    );
    reject!(
        debugger_mode && input.http_transactions,
        format!("{debugger_flag} cannot be combined with --http-transactions")
    );
    reject!(
        input.debugger_console && input.debug_session,
        "--debugger-console cannot be combined with --debug-session"
    );
    reject!(
        input.report_format.is_some() && input.diagnostics,
        "--report-format cannot be combined with --diagnostics"
    );
    reject!(
        input.report_format.is_some() && input.http_transactions,
        "--report-format cannot be combined with --http-transactions"
    );
    reject!(
        input.report_format.is_some() && debugger_mode,
        format!("--report-format cannot be combined with {debugger_flag}")
    );

    reject!(
        input.scan_all && input.dsl_path,
        "--scan-all cannot be combined with --dsl"
    );
    reject!(
        input.scan_all && input.protocol,
        "--scan-all cannot be combined with --protocol"
    );
    reject!(
        input.scan_all && input.entry,
        "--scan-all cannot be combined with --entry"
    );
    reject!(
        input.protocol_set_path && !input.scan_all,
        "--protocol-set requires --scan-all"
    );
    reject!(
        input.dsl_path && input.protocol,
        input.locale.msg("dsl_protocol_conflict")
    );
    reject!(
        input.dsl_path && input.entry,
        input.locale.msg("dsl_entry_conflict")
    );
    reject!(
        input.list_protocols && input.list_entries,
        input.locale.msg("list_conflict")
    );
    reject!(
        input.list_history && input.list_protocols,
        "--list-history cannot be combined with --list-protocols"
    );
    reject!(
        input.list_history && input.list_entries,
        "--list-history cannot be combined with --list-entries"
    );
    reject!(
        input.socket_target.is_some() && input.pid,
        input.locale.msg("pid_socket_conflict")
    );
    reject!(
        input.entry && !input.protocol,
        input.locale.msg("entry_requires_protocol")
    );
    reject!(
        input.dsl_path && input.demo_mode != DemoMode::Both,
        input.locale.msg("dsl_demo_conflict")
    );
    reject!(
        input.socket_target.is_some() && input.demo_mode != DemoMode::Both,
        input.locale.msg("demo_socket_conflict")
    );
    reject!(
        input.serve && input.socket_target.is_none(),
        input.locale.msg("serve_requires_socket")
    );
    reject!(
        input.api_socket.is_some() && !input.serve,
        input.locale.msg("api_requires_serve")
    );

    reject!(
        input
            .api_socket
            .is_some_and(|addr| !input.allow_remote_api && !api_socket_addr_is_local(addr)),
        input.locale.msg("remote_api_requires_flag")
    );
    reject!(
        remote_socket_requires_flag(input.socket_target, input.ingest_mode),
        input.locale.msg("remote_socket_requires_flag")
    );
    reject!(
        input.external_engine_worker && !input.external_engine_bin,
        "--external-engine-worker requires --external-engine-bin"
    );
    reject!(
        input.external_engine_python_bin && !input.external_engine_worker,
        "--external-engine-python-bin requires --external-engine-worker"
    );
    Ok(())
}

fn remote_socket_requires_flag(target: Option<&SocketTarget>, ingest_mode: IngestMode) -> bool {
    matches!(target, Some(SocketTarget::Tcp(_)))
        && ingest_mode != IngestMode::RemoteAdvisory
        && target.is_some_and(|target| !socket_target_is_local(target))
}
