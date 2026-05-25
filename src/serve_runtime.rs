use gewyvern::export::ExportBundle;
use std::net::TcpListener;

use crate::data_api::{
    ApiRenderedTarget, ApiState, start_api_service, update_api_snapshot_for_scan,
    update_api_snapshot_for_single,
};
use crate::diagnosis_runtime::external_sidecar_presence;

use super::{
    Cli, SocketTarget, UiLocale, analysis_snapshot, analysis_snapshot_json, annotate_export_trust,
    findings_json_with_analysis, findings_text, render_report_outputs, render_scan_outputs,
    run_binding_session, scan_report_html, scan_report_json_with_analyses,
    scan_report_text_with_analyses, scan_targets_for_cli, summary_json_with_analysis,
    summary_line_with_analysis,
};

pub(super) fn serve_socket_sessions(cli: &Cli, socket_target: &SocketTarget) {
    let api_state = cli.api_socket.as_deref().map(start_api_service);
    match socket_target {
        SocketTarget::Unix(path) => serve_unix_socket_sessions(cli, path, api_state),
        SocketTarget::Tcp(addr) => serve_tcp_socket_sessions(cli, addr, api_state),
    }
}

fn serve_unix_socket_sessions(cli: &Cli, path: &str, api_state: Option<ApiState>) {
    let locale = UiLocale::detect();
    let scan_targets = scan_targets_for_cli(cli).unwrap_or_else(|err| {
        eprintln!("{err}");
        std::process::exit(2);
    });
    #[cfg(target_family = "unix")]
    {
        super::remove_unix_socket_file(path).unwrap_or_else(|err| {
            eprintln!(
                "{}",
                locale.msgf("socket_service_failed", &format!("{err:?}"), None)
            );
            std::process::exit(1);
        });
        let listener = super::bind_unix_socket_listener(path).unwrap_or_else(|err| {
            eprintln!(
                "{}",
                locale.msgf("socket_service_failed", &format!("{err:?}"), None)
            );
            std::process::exit(1);
        });
        let max_sessions = cli.max_sessions.unwrap_or(usize::MAX);

        for _ in 0..max_sessions {
            if cli.scan_all {
                let facts = match super::collect_unix_socket_facts_on_listener(&listener) {
                    Ok(facts) => facts,
                    Err(err) => {
                        eprintln!(
                            "{}",
                            locale.msgf("socket_service_failed", &format!("{err:?}"), None)
                        );
                        continue;
                    }
                };
                let mut outputs = Vec::new();
                for target in &scan_targets {
                    let export = run_binding_session(target.binding(), &facts);
                    let export = annotate_export_trust(export, cli);
                    outputs.push((target.label(), export));
                }
                emit_scan_outputs(cli, &outputs, true, api_state.as_ref());
                continue;
            }

            let export = match if let Some(binding) = cli.dsl_binding() {
                super::run_unix_socket_session_on_listener_with_binding(&listener, binding)
            } else {
                super::run_unix_socket_session_on_listener(&listener, cli.template_mode.template())
            } {
                Ok(export) => export,
                Err(err) => {
                    eprintln!(
                        "{}",
                        locale.msgf("socket_service_failed", &format!("{err:?}"), None)
                    );
                    continue;
                }
            };
            let export = annotate_export_trust(export, cli);
            emit_rendered(cli, "socket_session", &export, true, api_state.as_ref());
        }

        super::remove_unix_socket_file(path).unwrap_or_else(|err| {
            eprintln!(
                "{}",
                locale.msgf("socket_service_failed", &format!("{err:?}"), None)
            );
            std::process::exit(1);
        });
        return;
    }

    #[cfg(not(target_family = "unix"))]
    {
        let _ = path;
        eprintln!("{}", locale.msg("unix_only"));
        std::process::exit(1);
    }
}

fn serve_tcp_socket_sessions(cli: &Cli, addr: &str, api_state: Option<ApiState>) {
    let locale = UiLocale::detect();
    let scan_targets = scan_targets_for_cli(cli).unwrap_or_else(|err| {
        eprintln!("{err}");
        std::process::exit(2);
    });
    let listener = TcpListener::bind(addr).unwrap_or_else(|err| {
        eprintln!(
            "{}",
            locale.msgf("socket_service_failed", &err.to_string(), None)
        );
        std::process::exit(1);
    });
    let max_sessions = cli.max_sessions.unwrap_or(usize::MAX);

    for _ in 0..max_sessions {
        if cli.scan_all {
            let facts = match super::collect_tcp_socket_facts_on_listener(&listener) {
                Ok(facts) => facts,
                Err(err) => {
                    eprintln!(
                        "{}",
                        locale.msgf("socket_service_failed", &format!("{err:?}"), None)
                    );
                    continue;
                }
            };
            let mut outputs = Vec::new();
            for target in &scan_targets {
                let export = run_binding_session(target.binding(), &facts);
                let export = annotate_export_trust(export, cli);
                outputs.push((target.label(), export));
            }
            emit_scan_outputs(cli, &outputs, true, api_state.as_ref());
            continue;
        }

        let export = match if let Some(binding) = cli.dsl_binding() {
            super::run_tcp_socket_session_on_listener_with_binding(&listener, binding)
        } else {
            super::run_tcp_socket_session_on_listener(&listener, cli.template_mode.template())
        } {
            Ok(export) => export,
            Err(err) => {
                eprintln!(
                    "{}",
                    locale.msgf("socket_service_failed", &format!("{err:?}"), None)
                );
                continue;
            }
        };
        let export = annotate_export_trust(export, cli);
        emit_rendered(cli, "socket_session", &export, true, api_state.as_ref());
    }
}

fn emit_rendered(
    cli: &Cli,
    name: &str,
    export: &ExportBundle,
    append: bool,
    api_state: Option<&ApiState>,
) {
    let single = vec![(name.to_string(), export.clone())];
    let analysis = analysis_snapshot(export);
    let summary_text = summary_line_with_analysis(name, export, &analysis);
    let summary_json_body = summary_json_with_analysis(name, export, &analysis);
    let findings_json_body = findings_json_with_analysis(name, export, &analysis);
    let analysis_json_body = analysis_snapshot_json(&analysis);
    let export_json_body = export.to_json();
    let report_json_body = scan_report_json_with_analyses(&single, std::slice::from_ref(&analysis));
    let report_html_body = scan_report_html(&single);
    let (
        has_external_sidecar_context,
        has_external_evidence_chain_enrichment,
        has_external_diagnostic_opinion,
    ) = external_sidecar_presence(&analysis);
    if let Some(state) = api_state {
        update_api_snapshot_for_single(
            state,
            ApiRenderedTarget {
                name: name.to_string(),
                summary_text: summary_text.clone(),
                summary_json: summary_json_body.clone(),
                findings_json: findings_json_body.clone(),
                analysis_json: analysis_json_body.clone(),
                has_external_sidecar_context,
                has_external_evidence_chain_enrichment,
                has_external_diagnostic_opinion,
                export_json: export_json_body.clone(),
                report_json: report_json_body.clone(),
                report_html: report_html_body.clone(),
            },
        );
    }
    let rendered = if cli.report_format.is_some() {
        render_report_outputs(cli, &single)
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
    let analyses = outputs
        .iter()
        .map(|(_, export)| analysis_snapshot(export))
        .collect::<Vec<_>>();
    let scan_summary_text = scan_report_text_with_analyses(outputs, &analyses);
    let scan_summary_json = scan_report_json_with_analyses(outputs, &analyses);
    let scan_analysis_json = format!(
        "[{}]",
        outputs
            .iter()
            .zip(analyses.iter())
            .map(|((name, _), analysis)| format!(
                "{{\"target\":\"{}\",\"analysis\":{}}}",
                name.replace('\\', "\\\\").replace('"', "\\\""),
                analysis_snapshot_json(analysis),
            ))
            .collect::<Vec<_>>()
            .join(",")
    );
    let scan_report_html_body = scan_report_html(outputs);
    if let Some(state) = api_state {
        let targets = outputs
            .iter()
            .zip(analyses.iter())
            .map(|((name, export), analysis)| {
                let (
                    has_external_sidecar_context,
                    has_external_evidence_chain_enrichment,
                    has_external_diagnostic_opinion,
                ) = external_sidecar_presence(analysis);
                ApiRenderedTarget {
                    name: name.clone(),
                    summary_text: summary_line_with_analysis(name, export, analysis),
                    summary_json: summary_json_with_analysis(name, export, analysis),
                    findings_json: findings_json_with_analysis(name, export, analysis),
                    analysis_json: analysis_snapshot_json(analysis),
                    has_external_sidecar_context,
                    has_external_evidence_chain_enrichment,
                    has_external_diagnostic_opinion,
                    export_json: export.to_json(),
                    report_json: scan_report_json_with_analyses(
                        &[(name.clone(), export.clone())],
                        std::slice::from_ref(analysis),
                    ),
                    report_html: scan_report_html(&[(name.clone(), export.clone())]),
                }
            })
            .collect::<Vec<_>>();
        update_api_snapshot_for_scan(
            state,
            targets,
            scan_summary_text.clone(),
            scan_summary_json.clone(),
            scan_analysis_json,
            scan_summary_json.clone(),
            scan_report_html_body.clone(),
        );
    }
    let rendered = render_scan_outputs(cli, outputs);
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
                eprintln!(
                    "{}",
                    locale.msgf("write_failed", path, Some(&err.to_string()))
                );
                std::process::exit(1);
            });
        } else {
            super::fs::write(path, format!("{rendered}\n")).unwrap_or_else(|err| {
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
