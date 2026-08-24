use gewyvern::export::ExportBundle;
use gewyvern::protocol_profiles::protocol_target_name_for_template_id;
use std::net::TcpListener;

use crate::data_api::{
    ApiRenderedTarget, ApiService, ApiState, persist_api_snapshot, start_api_service,
    start_api_service_with_admin_token, update_api_snapshot_for_scan_with_protocol_surfaces,
    update_api_snapshot_for_single_with_protocol_surface,
};
use crate::diagnosis_runtime::{
    external_capability_summary, external_sidecar_consumption_mode, external_sidecar_presence,
    external_sidecar_trust_level,
};
use crate::report_runtime::{
    collect_analyses, collect_protocol_surfaces, protocol_surface_for_target,
    scan_report_html_with_analyses_and_surfaces, scan_report_json_with_analyses_and_surfaces,
    scan_report_text_with_analyses_and_surfaces,
    single_target_report_html_with_analysis_and_surface,
    single_target_report_json_with_analysis_and_surface,
};
use crate::runtime_events::{
    EVENT_API_SERVICE_START, EVENT_APPEND_FAILED, EVENT_SNAPSHOT_PERSIST_FAILED,
    EVENT_SOCKET_LISTENER_BIND_FAILED, EVENT_SOCKET_LISTENER_CLEANUP_FAILED,
    EVENT_SOCKET_SESSION_COLLECT_FAILED, EVENT_SOCKET_SESSION_IDLE_TIMEOUT,
    EVENT_SOCKET_SESSION_RUN_FAILED, EVENT_SOCKET_STALE_CLEANUP_FAILED, EVENT_TCP_SERVICE_START,
    EVENT_UNIX_SERVICE_START, EVENT_WRITE_FAILED,
};
use crate::runtime_logging::{log_error_event, log_info_event, log_warn_event};
use crate::socket_resilience::{
    SocketLoopHealth, apply_socket_failure_backoff, log_socket_loop_recovered,
    log_socket_session_failure,
};

use super::{
    Cli, ReportFormat, SocketTarget, UiLocale, analysis_snapshot, analysis_snapshot_json,
    annotate_export_trust, findings_json_with_analysis, findings_text, run_binding_session,
    scan_analysis_json_array, scan_targets_for_cli, summary_json_with_analysis,
    summary_line_with_analysis, training_example_json_array, training_example_json_with_analysis,
};

pub(crate) const SOCKET_SESSION_TARGET_NAME: &str = "socket_session";

pub(super) fn serve_socket_sessions(cli: &Cli, socket_target: &SocketTarget) {
    let api_service = cli.api_socket.as_deref().map(|addr| {
        log_info_event(
            "api",
            EVENT_API_SERVICE_START,
            &[("socket", addr.to_string())],
            "starting api service",
        );
        match cli.api_admin_token.as_deref() {
            Some(token) => {
                start_api_service_with_admin_token(addr, cli.allow_remote_api, Some(token))
            }
            None => start_api_service(addr, cli.allow_remote_api),
        }
    });
    match socket_target {
        SocketTarget::Unix(path) => serve_unix_socket_sessions(cli, path, api_service),
        SocketTarget::Tcp(addr) => serve_tcp_socket_sessions(cli, addr, api_service),
    }
}

fn log_socket_service_failure(event: &str, transport: &str, endpoint: &str, error: &str) {
    log_error_event(
        "serve",
        event,
        &[
            ("transport", transport.to_string()),
            ("endpoint", endpoint.to_string()),
            ("error", error.to_string()),
        ],
        "socket service failure",
    );
}

fn log_socket_idle_timeout(transport: &str, endpoint: &str, idle_polls: usize, error: &str) {
    log_info_event(
        "serve",
        EVENT_SOCKET_SESSION_IDLE_TIMEOUT,
        &[
            ("transport", transport.to_string()),
            ("endpoint", endpoint.to_string()),
            ("idle_polls", idle_polls.to_string()),
            ("error", error.to_string()),
        ],
        "socket service idle; waiting for the next client",
    );
}

fn serve_unix_socket_sessions(cli: &Cli, path: &str, api_service: Option<ApiService>) {
    let locale = UiLocale::detect();
    let api_state = api_service.as_ref().map(ApiService::state);
    log_info_event(
        "serve",
        EVENT_UNIX_SERVICE_START,
        &[
            ("socket", path.to_string()),
            ("max_sessions", max_sessions_label(cli)),
        ],
        "starting unix socket service",
    );
    let scan_targets = scan_targets_for_cli(cli).unwrap_or_else(|err| {
        eprintln!("{err}");
        std::process::exit(2);
    });
    #[cfg(target_family = "unix")]
    {
        super::remove_unix_socket_file(path).unwrap_or_else(|err| {
            log_socket_service_failure(
                EVENT_SOCKET_STALE_CLEANUP_FAILED,
                "unix",
                path,
                &format!("{err:?}"),
            );
            eprintln!(
                "{}",
                locale.msgf("socket_service_failed", &format!("{err:?}"), None)
            );
            std::process::exit(1);
        });
        let listener = super::bind_unix_socket_listener(path).unwrap_or_else(|err| {
            log_socket_service_failure(
                EVENT_SOCKET_LISTENER_BIND_FAILED,
                "unix",
                path,
                &format!("{err:?}"),
            );
            eprintln!(
                "{}",
                locale.msgf("socket_service_failed", &format!("{err:?}"), None)
            );
            std::process::exit(1);
        });
        let max_sessions = cli.max_sessions.unwrap_or(usize::MAX);
        let mut handled_sessions = 0usize;
        let mut loop_health = SocketLoopHealth::default();
        while handled_sessions < max_sessions {
            if cli.scan_all {
                let facts = match super::collect_unix_socket_facts_on_listener(&listener) {
                    Ok(facts) => facts,
                    Err(err) => {
                        if err.is_accept_timeout() {
                            let idle_polls = loop_health.record_idle_timeout();
                            if idle_polls == 1 || idle_polls % 12 == 0 {
                                log_socket_idle_timeout(
                                    "unix",
                                    path,
                                    idle_polls,
                                    &format!("{err:?}"),
                                );
                            }
                            continue;
                        }
                        handled_sessions += 1;
                        let report = loop_health.record_failure();
                        log_socket_session_failure(
                            EVENT_SOCKET_SESSION_COLLECT_FAILED,
                            "unix",
                            path,
                            &format!("{err:?}"),
                            report,
                        );
                        apply_socket_failure_backoff(report);
                        eprintln!(
                            "{}",
                            locale.msgf("socket_service_failed", &format!("{err:?}"), None)
                        );
                        continue;
                    }
                };
                if let Some(recovered) = loop_health.record_success() {
                    log_socket_loop_recovered("unix", path, recovered);
                }
                handled_sessions += 1;
                let mut outputs = Vec::new();
                for target in &scan_targets {
                    let export = run_binding_session(target.binding(), &facts);
                    let export = annotate_export_trust(export, cli);
                    outputs.push((target.label(), export));
                }
                emit_scan_outputs(cli, &outputs, true, api_state);
                continue;
            }

            let export = match if let Some(binding) = cli.dsl_binding() {
                super::run_unix_socket_session_on_listener_with_binding(&listener, binding)
            } else {
                super::run_unix_socket_session_on_listener(&listener, cli.template_mode.template())
            } {
                Ok(export) => export,
                Err(err) => {
                    if err.is_accept_timeout() {
                        let idle_polls = loop_health.record_idle_timeout();
                        if idle_polls == 1 || idle_polls % 12 == 0 {
                            log_socket_idle_timeout("unix", path, idle_polls, &format!("{err:?}"));
                        }
                        continue;
                    }
                    handled_sessions += 1;
                    let report = loop_health.record_failure();
                    log_socket_session_failure(
                        EVENT_SOCKET_SESSION_RUN_FAILED,
                        "unix",
                        path,
                        &format!("{err:?}"),
                        report,
                    );
                    apply_socket_failure_backoff(report);
                    eprintln!(
                        "{}",
                        locale.msgf("socket_service_failed", &format!("{err:?}"), None)
                    );
                    continue;
                }
            };
            if let Some(recovered) = loop_health.record_success() {
                log_socket_loop_recovered("unix", path, recovered);
            }
            handled_sessions += 1;
            let export = annotate_export_trust(export, cli);
            let target_name = single_runtime_target_name(&export);
            emit_rendered(cli, &target_name, &export, true, api_state);
        }

        super::remove_unix_socket_file(path).unwrap_or_else(|err| {
            log_socket_service_failure(
                EVENT_SOCKET_LISTENER_CLEANUP_FAILED,
                "unix",
                path,
                &format!("{err:?}"),
            );
            eprintln!(
                "{}",
                locale.msgf("socket_service_failed", &format!("{err:?}"), None)
            );
            std::process::exit(1);
        });
    }

    #[cfg(not(target_family = "unix"))]
    {
        let _ = path;
        eprintln!("{}", locale.msg("unix_only"));
        std::process::exit(1);
    }
}

fn serve_tcp_socket_sessions(cli: &Cli, addr: &str, api_service: Option<ApiService>) {
    let locale = UiLocale::detect();
    let api_state = api_service.as_ref().map(ApiService::state);
    log_info_event(
        "serve",
        EVENT_TCP_SERVICE_START,
        &[
            ("socket", addr.to_string()),
            ("max_sessions", max_sessions_label(cli)),
        ],
        "starting tcp socket service",
    );
    let scan_targets = scan_targets_for_cli(cli).unwrap_or_else(|err| {
        eprintln!("{err}");
        std::process::exit(2);
    });
    let listener = TcpListener::bind(addr).unwrap_or_else(|err| {
        log_socket_service_failure(
            EVENT_SOCKET_LISTENER_BIND_FAILED,
            "tcp",
            addr,
            &err.to_string(),
        );
        eprintln!(
            "{}",
            locale.msgf("socket_service_failed", &err.to_string(), None)
        );
        std::process::exit(1);
    });
    let max_sessions = cli.max_sessions.unwrap_or(usize::MAX);
    let mut handled_sessions = 0usize;
    let mut loop_health = SocketLoopHealth::default();
    while handled_sessions < max_sessions {
        if cli.scan_all {
            let facts = match super::collect_tcp_socket_facts_on_listener(&listener) {
                Ok(facts) => facts,
                Err(err) => {
                    if err.is_accept_timeout() {
                        let idle_polls = loop_health.record_idle_timeout();
                        if idle_polls == 1 || idle_polls % 12 == 0 {
                            log_socket_idle_timeout("tcp", addr, idle_polls, &format!("{err:?}"));
                        }
                        continue;
                    }
                    handled_sessions += 1;
                    let report = loop_health.record_failure();
                    log_socket_session_failure(
                        EVENT_SOCKET_SESSION_COLLECT_FAILED,
                        "tcp",
                        addr,
                        &format!("{err:?}"),
                        report,
                    );
                    apply_socket_failure_backoff(report);
                    eprintln!(
                        "{}",
                        locale.msgf("socket_service_failed", &format!("{err:?}"), None)
                    );
                    continue;
                }
            };
            if let Some(recovered) = loop_health.record_success() {
                log_socket_loop_recovered("tcp", addr, recovered);
            }
            handled_sessions += 1;
            let mut outputs = Vec::new();
            for target in &scan_targets {
                let export = run_binding_session(target.binding(), &facts);
                let export = annotate_export_trust(export, cli);
                outputs.push((target.label(), export));
            }
            emit_scan_outputs(cli, &outputs, true, api_state);
            continue;
        }

        let export = match if let Some(binding) = cli.dsl_binding() {
            super::run_tcp_socket_session_on_listener_with_binding(&listener, binding)
        } else {
            super::run_tcp_socket_session_on_listener(&listener, cli.template_mode.template())
        } {
            Ok(export) => export,
            Err(err) => {
                if err.is_accept_timeout() {
                    let idle_polls = loop_health.record_idle_timeout();
                    if idle_polls == 1 || idle_polls % 12 == 0 {
                        log_socket_idle_timeout("tcp", addr, idle_polls, &format!("{err:?}"));
                    }
                    continue;
                }
                handled_sessions += 1;
                let report = loop_health.record_failure();
                log_socket_session_failure(
                    EVENT_SOCKET_SESSION_RUN_FAILED,
                    "tcp",
                    addr,
                    &format!("{err:?}"),
                    report,
                );
                apply_socket_failure_backoff(report);
                eprintln!(
                    "{}",
                    locale.msgf("socket_service_failed", &format!("{err:?}"), None)
                );
                continue;
            }
        };
        if let Some(recovered) = loop_health.record_success() {
            log_socket_loop_recovered("tcp", addr, recovered);
        }
        handled_sessions += 1;
        let export = annotate_export_trust(export, cli);
        let target_name = single_runtime_target_name(&export);
        emit_rendered(cli, &target_name, &export, true, api_state);
    }
}

pub(crate) fn single_runtime_target_name(export: &ExportBundle) -> String {
    protocol_target_name_for_template_id(&export.template_id)
        .unwrap_or_else(|| SOCKET_SESSION_TARGET_NAME.to_string())
}

fn max_sessions_label(cli: &Cli) -> String {
    cli.max_sessions
        .map(|value| value.to_string())
        .unwrap_or_else(|| "unbounded".to_string())
}

fn emit_rendered(
    cli: &Cli,
    name: &str,
    export: &ExportBundle,
    append: bool,
    api_state: Option<&ApiState>,
) {
    let analysis = analysis_snapshot(export);
    let protocol_surface = protocol_surface_for_target(name);
    let summary_text = summary_line_with_analysis(name, export, &analysis);
    let summary_json_body = summary_json_with_analysis(name, export, &analysis);
    let findings_json_body = findings_json_with_analysis(name, export, &analysis);
    let analysis_json_body = analysis_snapshot_json(&analysis);
    let training_example_json_body = training_example_json_with_analysis(name, export, &analysis);
    let export_json_body = export.to_json();
    let report_json_body = single_target_report_json_with_analysis_and_surface(
        name,
        export,
        &analysis,
        protocol_surface.as_ref(),
    );
    let report_html_body = single_target_report_html_with_analysis_and_surface(
        name,
        export,
        &analysis,
        protocol_surface.as_ref(),
    );
    let (
        has_external_sidecar_context,
        has_external_evidence_chain_enrichment,
        has_external_diagnostic_opinion,
    ) = external_sidecar_presence(&analysis);
    let (
        has_external_capability_profile,
        external_capability_status,
        external_hint_status,
        external_context_status,
    ) = external_capability_summary(&analysis);
    let external_sidecar_consumption_mode = external_sidecar_consumption_mode(&analysis);
    let external_sidecar_trust_level = external_sidecar_trust_level(&analysis);
    if let Some(state) = api_state {
        update_api_snapshot_for_single_with_protocol_surface(
            state,
            ApiRenderedTarget {
                name: name.to_string(),
                primary_module_family: analysis.primary_module_family.clone(),
                evidence_posture: analysis.evidence_posture.clone(),
                automation_outcome: analysis.automation_outcome.clone(),
                summary_text: summary_text.clone(),
                summary_json: summary_json_body.clone(),
                findings_json: findings_json_body.clone(),
                analysis_json: analysis_json_body.clone(),
                training_example_json: training_example_json_body.clone(),
                has_external_sidecar_context,
                has_external_evidence_chain_enrichment,
                has_external_diagnostic_opinion,
                has_external_capability_profile,
                external_capability_status,
                external_hint_status,
                external_context_status,
                external_sidecar_trust_level,
                external_sidecar_consumption_mode,
                export_json: export_json_body.clone(),
                report_json: report_json_body.clone(),
                report_html: report_html_body.clone(),
            },
            protocol_surface,
        );
        if let Err(err) = persist_api_snapshot(state) {
            log_warn_event(
                "api_snapshot",
                EVENT_SNAPSHOT_PERSIST_FAILED,
                &[("error", err.to_string())],
                "failed to persist latest api snapshot",
            );
        }
    }
    let rendered = if let Some(report_format) = cli.report_format {
        match report_format {
            ReportFormat::Json => report_json_body,
            ReportFormat::Html => report_html_body,
        }
    } else if cli.findings {
        if cli.json {
            findings_json_body
        } else {
            findings_text(name, export)
        }
    } else if cli.json {
        if cli.summary_only {
            summary_json_body
        } else {
            export_json_body
        }
    } else {
        summary_text
    };

    write_rendered_output(cli, &rendered, append);
}

fn emit_scan_outputs(
    cli: &Cli,
    outputs: &[(String, ExportBundle)],
    append: bool,
    api_state: Option<&ApiState>,
) {
    let analyses = collect_analyses(outputs);
    let protocol_surfaces = collect_protocol_surfaces(outputs);
    let scan_summary_text =
        scan_report_text_with_analyses_and_surfaces(outputs, &analyses, &protocol_surfaces);
    let scan_summary_json =
        scan_report_json_with_analyses_and_surfaces(outputs, &analyses, &protocol_surfaces);
    let scan_analysis_json = scan_analysis_json_array(outputs, &analyses);
    let scan_training_example_json = training_example_json_array(outputs, &analyses);
    let scan_report_html_body =
        scan_report_html_with_analyses_and_surfaces(outputs, &analyses, &protocol_surfaces);
    if let Some(state) = api_state {
        let targets = outputs
            .iter()
            .zip(analyses.iter())
            .zip(protocol_surfaces.iter())
            .map(|(((name, export), analysis), protocol_surface)| {
                let (
                    has_external_sidecar_context,
                    has_external_evidence_chain_enrichment,
                    has_external_diagnostic_opinion,
                ) = external_sidecar_presence(analysis);
                let (
                    has_external_capability_profile,
                    external_capability_status,
                    external_hint_status,
                    external_context_status,
                ) = external_capability_summary(analysis);
                let external_sidecar_consumption_mode = external_sidecar_consumption_mode(analysis);
                let external_sidecar_trust_level = external_sidecar_trust_level(analysis);
                ApiRenderedTarget {
                    name: name.clone(),
                    primary_module_family: analysis.primary_module_family.clone(),
                    evidence_posture: analysis.evidence_posture.clone(),
                    automation_outcome: analysis.automation_outcome.clone(),
                    summary_text: summary_line_with_analysis(name, export, analysis),
                    summary_json: summary_json_with_analysis(name, export, analysis),
                    findings_json: findings_json_with_analysis(name, export, analysis),
                    analysis_json: analysis_snapshot_json(analysis),
                    training_example_json: training_example_json_with_analysis(
                        name, export, analysis,
                    ),
                    has_external_sidecar_context,
                    has_external_evidence_chain_enrichment,
                    has_external_diagnostic_opinion,
                    has_external_capability_profile,
                    external_capability_status,
                    external_hint_status,
                    external_context_status,
                    external_sidecar_trust_level,
                    external_sidecar_consumption_mode,
                    export_json: export.to_json(),
                    report_json: single_target_report_json_with_analysis_and_surface(
                        name,
                        export,
                        analysis,
                        protocol_surface.as_ref(),
                    ),
                    report_html: single_target_report_html_with_analysis_and_surface(
                        name,
                        export,
                        analysis,
                        protocol_surface.as_ref(),
                    ),
                }
            })
            .collect::<Vec<_>>();
        update_api_snapshot_for_scan_with_protocol_surfaces(
            state,
            targets,
            protocol_surfaces,
            scan_summary_text.clone(),
            scan_summary_json.clone(),
            scan_analysis_json,
            scan_training_example_json,
            scan_summary_json.clone(),
            scan_report_html_body.clone(),
        );
        if let Err(err) = persist_api_snapshot(state) {
            log_warn_event(
                "api_snapshot",
                EVENT_SNAPSHOT_PERSIST_FAILED,
                &[("error", err.to_string())],
                "failed to persist latest api snapshot",
            );
        }
    }
    let rendered = match cli.report_format {
        Some(ReportFormat::Html) => scan_report_html_body,
        Some(ReportFormat::Json) => scan_summary_json,
        None if cli.json => scan_summary_json,
        None => scan_summary_text,
    };
    write_rendered_output(cli, &rendered, append);
}

fn write_rendered_output(cli: &Cli, rendered: &str, append: bool) {
    let locale = UiLocale::detect();
    if let Some(path) = cli.out_path.as_deref() {
        if append {
            let mut existing = super::fs::read_to_string(path).unwrap_or_default();
            existing.push_str(rendered);
            existing.push('\n');
            super::fs::write(path, existing).unwrap_or_else(|err| {
                log_error_event(
                    "output",
                    EVENT_APPEND_FAILED,
                    &[("path", path.to_string()), ("error", err.to_string())],
                    "failed to append rendered output",
                );
                eprintln!(
                    "{}",
                    locale.msgf("write_failed", path, Some(&err.to_string()))
                );
                std::process::exit(1);
            });
        } else {
            super::fs::write(path, format!("{rendered}\n")).unwrap_or_else(|err| {
                log_error_event(
                    "output",
                    EVENT_WRITE_FAILED,
                    &[("path", path.to_string()), ("error", err.to_string())],
                    "failed to write rendered output",
                );
                eprintln!(
                    "{}",
                    locale.msgf("write_failed", path, Some(&err.to_string()))
                );
                std::process::exit(1);
            });
        }
    } else {
        println!("{rendered}");
    }
}
