#[path = "main/binding_demo.rs"]
mod binding_demo;
#[path = "main/certificate_state_cli.rs"]
mod certificate_state_cli;
#[path = "main/cli.rs"]
mod cli;
#[path = "main/cli_validation.rs"]
mod cli_validation;
mod data_api;
mod diagnosis_runtime;
mod external_analysis;
#[path = "main/helpers.rs"]
mod helpers;
#[path = "main/history_catalog_delta.rs"]
mod history_catalog_delta;
#[path = "main/history_view.rs"]
mod history_view;
mod render_utils;
mod report_runtime;
#[path = "main/runtime_config.rs"]
mod runtime_config;
#[path = "main/runtime_events.rs"]
mod runtime_events;
#[path = "main/runtime_logging.rs"]
mod runtime_logging;
#[path = "main/runtime_migration.rs"]
mod runtime_migration;
mod serve_runtime;
#[path = "main/socket_resilience.rs"]
mod socket_resilience;
#[path = "main/startup.rs"]
mod startup;
#[cfg(test)]
#[path = "main/tests.rs"]
mod tests;
#[path = "main/ui_locale.rs"]
mod ui_locale;

use gewyvern::dsl::compile_file;
use gewyvern::export::ExportBundle;
use gewyvern::flow::{FlowId, ProcessView, ProgramFlowId};
use gewyvern::gewyc::{RenderFormat, compile_diagnostics_report_file, render_diagnostics_report};
use gewyvern::http::compose_http_transactions;
use gewyvern::ledger::{
    CpuId, FactEnvelope, FactId, FactKind, PacketDir, PacketMetaFact, QuicFrameType,
    QuicPacketType, RouteDecisionFact, SessionId, SockLineageFact, TcpStateFact,
};
use gewyvern::protocol_profiles::{
    ResolvedProtocolProfile, default_protocol_scan_set, default_protocol_scan_set_from_dir,
    protocol_dsl_path, protocol_summaries, protocol_summary, resolve_built_in_dsl_path,
    resolve_protocol_profile,
};
use gewyvern::runtime::{RuntimeSession, SessionConfig};
use gewyvern::socket_input::{
    bind_unix_socket_listener, collect_tcp_socket_facts, collect_tcp_socket_facts_on_listener,
    collect_unix_socket_facts, collect_unix_socket_facts_on_listener, remove_unix_socket_file,
    run_tcp_socket_session, run_tcp_socket_session_on_listener,
    run_tcp_socket_session_on_listener_with_binding, run_tcp_socket_session_with_binding,
    run_unix_socket_session, run_unix_socket_session_on_listener,
    run_unix_socket_session_on_listener_with_binding, run_unix_socket_session_with_binding,
};
use gewyvern::template::{TemplateBinding, handshake_debug_template, udp_debug_template};
use std::collections::HashSet;
use std::env;
use std::fs;
use std::net::ToSocketAddrs;
use std::path::Path;
use std::time::{Duration, SystemTime};

use crate::diagnosis_runtime::*;
use crate::external_analysis::ExternalAnalysisConfig;
use crate::history_view::render_history_index;
use crate::report_runtime::{
    findings_json, findings_json_with_analysis, findings_text, http_transactions_json,
    http_transactions_text, render_debug_session_outputs, render_debugger_console_outputs,
    render_report_outputs, render_scan_outputs, scan_report_html, scan_report_json_with_analyses,
    scan_report_text_with_analyses, summary_json, summary_json_with_analysis, summary_line,
    summary_line_with_analysis, training_example_json_array, training_example_json_with_analysis,
};
#[cfg(test)]
use crate::report_runtime::{scan_report_json, scan_report_text, training_example_json};
use crate::runtime_events::{
    EVENT_DIAGNOSTICS_COMPILE_FAILED, EVENT_DIAGNOSTICS_REQUIRES_DSL, EVENT_HISTORY_RENDER_FAILED,
    EVENT_SCAN_TARGET_RESOLVE_FAILED, EVENT_SOCKET_SESSION_COLLECT_FAILED,
    EVENT_SOCKET_SESSION_RUN_FAILED, EVENT_WRITE_FAILED,
};
use crate::runtime_logging::log_error_event;
use crate::serve_runtime::serve_socket_sessions;
use crate::startup::bootstrap_cli;

pub(crate) use self::binding_demo::run_binding_demo;
pub(crate) use self::ui_locale::UiLocale;

fn main() {
    let locale = UiLocale::detect();
    let args = env::args().skip(1).collect::<Vec<_>>();
    let cli = bootstrap_cli(args);

    if cli.list_protocols {
        let rendered = if cli.json {
            list_protocols_json()
        } else {
            list_protocols_text()
        };
        write_or_print(&rendered, cli.out_path.as_deref(), locale);
        return;
    }

    if cli.list_history {
        let rendered = render_history_index(cli.json).unwrap_or_else(|message| {
            log_error_event(
                "history",
                EVENT_HISTORY_RENDER_FAILED,
                &[("error", message.clone())],
                "failed to render history index",
            );
            eprintln!("{message}");
            std::process::exit(2);
        });
        write_or_print(&rendered, cli.out_path.as_deref(), locale);
        return;
    }

    if let Some(protocol) = cli.list_entries.as_deref() {
        let rendered = if cli.json {
            list_entries_json(protocol).unwrap_or_else(|| {
                eprintln!("{}", locale.msgf("unsupported_protocol", protocol, None));
                std::process::exit(2);
            })
        } else {
            list_entries_text(protocol).unwrap_or_else(|| {
                eprintln!("{}", locale.msgf("unsupported_protocol", protocol, None));
                std::process::exit(2);
            })
        };
        write_or_print(&rendered, cli.out_path.as_deref(), locale);
        return;
    }

    let base = SystemTime::UNIX_EPOCH + Duration::from_secs(1_710_000_000);
    let scan_targets = scan_targets_for_cli(&cli).unwrap_or_else(|err| {
        log_error_event(
            "runtime",
            EVENT_SCAN_TARGET_RESOLVE_FAILED,
            &[("error", err.clone())],
            "failed to resolve scan targets",
        );
        eprintln!("{err}");
        std::process::exit(2);
    });
    let mut outputs: Vec<(String, ExportBundle)> = Vec::new();

    if cli.diagnostics {
        let path = cli.dsl_path.as_deref().unwrap_or_else(|| {
            log_error_event(
                "diagnostics",
                EVENT_DIAGNOSTICS_REQUIRES_DSL,
                &[],
                "diagnostics mode requires a dsl path",
            );
            eprintln!("{}", locale.msg("diagnostics_requires_dsl"));
            std::process::exit(2);
        });
        let report = compile_diagnostics_report_file(path).unwrap_or_else(|err| {
            log_error_event(
                "diagnostics",
                EVENT_DIAGNOSTICS_COMPILE_FAILED,
                &[("path", path.to_string()), ("error", format!("{err:?}"))],
                "failed to compile diagnostics report",
            );
            eprintln!(
                "{}",
                locale.msgf("binding_diagnostics_failed", &format!("{err:?}"), None)
            );
            std::process::exit(2);
        });
        let rendered = if cli.json {
            render_diagnostics_report(&report, RenderFormat::Json)
        } else {
            render_diagnostics_report(&report, RenderFormat::Text)
        };
        write_or_print(&rendered, cli.out_path.as_deref(), locale);
        return;
    }

    if let Some(socket_target) = cli.socket_target.as_ref() {
        if cli.serve {
            serve_socket_sessions(&cli, socket_target);
            return;
        }

        if cli.scan_all {
            let facts = match socket_target {
                SocketTarget::Unix(path) => collect_unix_socket_facts(path),
                SocketTarget::Tcp(addr) => collect_tcp_socket_facts(addr),
            }
            .unwrap_or_else(|err| {
                let endpoint = match socket_target {
                    SocketTarget::Unix(path) => format!("unix:{path}"),
                    SocketTarget::Tcp(addr) => format!("tcp:{addr}"),
                };
                log_error_event(
                    "runtime",
                    EVENT_SOCKET_SESSION_COLLECT_FAILED,
                    &[("endpoint", endpoint), ("error", format!("{err:?}"))],
                    "failed to collect socket session facts",
                );
                eprintln!(
                    "{}",
                    locale.msgf("socket_session_failed", &format!("{err:?}"), None)
                );
                std::process::exit(1);
            });
            for target in &scan_targets {
                let export = run_binding_session(target.binding(), &facts);
                let export = cli
                    .pid
                    .map(|pid| filter_export_by_pid(&export, pid))
                    .unwrap_or(export);
                outputs.push((target.label(), annotate_export_trust(export, &cli)));
            }
        } else {
            let export = match (socket_target, cli.dsl_binding()) {
                (SocketTarget::Unix(path), Some(binding)) => {
                    run_unix_socket_session_with_binding(path, binding)
                }
                (SocketTarget::Tcp(addr), Some(binding)) => {
                    run_tcp_socket_session_with_binding(addr, binding)
                }
                (SocketTarget::Unix(path), None) => {
                    run_unix_socket_session(path, cli.template_mode.template())
                }
                (SocketTarget::Tcp(addr), None) => {
                    run_tcp_socket_session(addr, cli.template_mode.template())
                }
            }
            .unwrap_or_else(|err| {
                let endpoint = match socket_target {
                    SocketTarget::Unix(path) => format!("unix:{path}"),
                    SocketTarget::Tcp(addr) => format!("tcp:{addr}"),
                };
                log_error_event(
                    "runtime",
                    EVENT_SOCKET_SESSION_RUN_FAILED,
                    &[("endpoint", endpoint), ("error", format!("{err:?}"))],
                    "failed to run socket session",
                );
                eprintln!(
                    "{}",
                    locale.msgf("socket_session_failed", &format!("{err:?}"), None)
                );
                std::process::exit(1);
            });
            let export = cli
                .pid
                .map(|pid| filter_export_by_pid(&export, pid))
                .unwrap_or(export);
            outputs.push((
                "socket_session".to_string(),
                annotate_export_trust(export, &cli),
            ));
        }
    } else {
        if cli.scan_all {
            for target in &scan_targets {
                let export = run_binding_demo(target.binding());
                let export = cli
                    .pid
                    .map(|pid| filter_export_by_pid(&export, pid))
                    .unwrap_or(export);
                outputs.push((target.label(), annotate_export_trust(export, &cli)));
            }
        } else if let Some(binding) = cli.dsl_binding() {
            let export = run_binding_demo(binding);
            let export = cli
                .pid
                .map(|pid| filter_export_by_pid(&export, pid))
                .unwrap_or(export);
            outputs.push(("dsl_demo".to_string(), annotate_export_trust(export, &cli)));
        } else {
            if cli.demo_mode.includes_tcp() {
                let tcp_export = run_session(
                    handshake_debug_template(),
                    vec![
                        FactEnvelope {
                            id: FactId(1),
                            ts: base,
                            cpu: CpuId(0),
                            ifindex: Some(2),
                            session: SessionId(1),
                            fragment_id: "tcp_state_fragment".into(),
                            kind: FactKind::TcpState(TcpStateFact {
                                netns: 1,
                                sk_cookie: 42,
                                saddr: [0; 16],
                                daddr: [0; 16],
                                sport: 42310,
                                dport: 443,
                                family: 2,
                                old: 1,
                                new: 2,
                            }),
                        },
                        FactEnvelope {
                            id: FactId(2),
                            ts: base + Duration::from_millis(10),
                            cpu: CpuId(0),
                            ifindex: Some(2),
                            session: SessionId(1),
                            fragment_id: "tcp_packet_meta_fragment".into(),
                            kind: FactKind::PacketMeta(PacketMetaFact {
                                netns: 1,
                                sk_cookie: Some(42),
                                dir: PacketDir::Egress,
                                local_port: None,
                                remote_port: None,
                                payload_byte0: None,
                                payload_byte1: None,
                                payload_prefix2: None,
                                payload_prefix4: None,
                                payload_byte4: None,
                                payload_byte5: None,
                                payload_byte9: None,
                                payload_byte10: None,
                                payload_byte13: None,
                                payload_bytes: std::collections::BTreeMap::new(),
                                l3_proto: 0x0800,
                                l4_proto: 6,
                                tot_len: 60,
                                tcp_flags: 0x02,
                                seq: Some(1),
                                ack: None,
                                window: Some(65535),
                            }),
                        },
                        route_fact(3, base + Duration::from_millis(20), 42, 2, SessionId(1)),
                    ],
                );

                let tcp_export = cli
                    .pid
                    .map(|pid| filter_export_by_pid(&tcp_export, pid))
                    .unwrap_or(tcp_export);
                outputs.push((
                    "tcp_demo".to_string(),
                    annotate_export_trust(tcp_export, &cli),
                ));
            }

            if cli.demo_mode.includes_udp() {
                let udp_export = run_session(
                    udp_debug_template(),
                    vec![
                        FactEnvelope {
                            id: FactId(1),
                            ts: base,
                            cpu: CpuId(0),
                            ifindex: Some(3),
                            session: SessionId(2),
                            fragment_id: "udp_packet_meta_fragment".into(),
                            kind: FactKind::PacketMeta(PacketMetaFact {
                                netns: 1,
                                sk_cookie: Some(99),
                                dir: PacketDir::Egress,
                                local_port: None,
                                remote_port: None,
                                payload_byte0: None,
                                payload_byte1: None,
                                payload_prefix2: None,
                                payload_prefix4: None,
                                payload_byte4: None,
                                payload_byte5: None,
                                payload_byte9: None,
                                payload_byte10: None,
                                payload_byte13: None,
                                payload_bytes: std::collections::BTreeMap::new(),
                                l3_proto: 0x0800,
                                l4_proto: 17,
                                tot_len: 72,
                                tcp_flags: 0,
                                seq: None,
                                ack: None,
                                window: None,
                            }),
                        },
                        route_fact(2, base + Duration::from_millis(10), 99, 3, SessionId(2)),
                    ],
                );

                let udp_export = cli
                    .pid
                    .map(|pid| filter_export_by_pid(&udp_export, pid))
                    .unwrap_or(udp_export);
                outputs.push((
                    "udp_demo".to_string(),
                    annotate_export_trust(udp_export, &cli),
                ));
            }
        }
    }

    let rendered = if cli.http_transactions {
        let transactions = if cli.dsl_path.is_some() {
            let mut composed_exports = Vec::new();
            composed_exports.extend(outputs.iter().map(|(_, export)| export.clone()));
            if outputs
                .iter()
                .any(|(_, export)| export_has_operation(export, "http_request"))
            {
                let dns_path = resolve_built_in_dsl_path(
                    "/Users/Shared/chroot/dev/gewyvern/dsl/dns_udp_process.gewy",
                );
                composed_exports.push(run_binding_demo(
                    compile_file(&dns_path).expect("dns dsl should compile"),
                ));
                let http_response_path = resolve_built_in_dsl_path(
                    "/Users/Shared/chroot/dev/gewyvern/dsl/http_server_response_path.gewy",
                );
                composed_exports.push(run_binding_demo(
                    compile_file(&http_response_path).expect("http server dsl should compile"),
                ));
            }
            if outputs
                .iter()
                .any(|(_, export)| export_has_operation(export, "http3_request"))
            {
                let http3_response_path = resolve_built_in_dsl_path(
                    "/Users/Shared/chroot/dev/gewyvern/dsl/http3_server_response_path.gewy",
                );
                composed_exports.push(run_binding_demo(
                    compile_file(&http3_response_path).expect("http3 server dsl should compile"),
                ));
            }
            compose_http_transactions(&composed_exports)
        } else {
            compose_http_transactions(
                &outputs
                    .iter()
                    .map(|(_, export)| export.clone())
                    .collect::<Vec<_>>(),
            )
        };

        if cli.json {
            http_transactions_json(&transactions)
        } else {
            http_transactions_text(&transactions)
        }
    } else if cli.debugger_console {
        render_debugger_console_outputs(&cli, &outputs)
    } else if cli.debug_session {
        render_debug_session_outputs(&cli, &outputs)
    } else if cli.findings {
        if cli.scan_all {
            render_scan_outputs(&cli, &outputs)
        } else if cli.report_format.is_some() {
            render_report_outputs(&cli, &outputs)
        } else {
            outputs
                .into_iter()
                .map(|(name, export)| {
                    if cli.json {
                        findings_json(&name, &export)
                    } else {
                        findings_text(&name, &export)
                    }
                })
                .collect::<Vec<_>>()
                .join("\n")
        }
    } else if cli.json {
        if cli.scan_all && cli.summary_only {
            render_scan_outputs(&cli, &outputs)
        } else if cli.report_format.is_some() {
            render_report_outputs(&cli, &outputs)
        } else {
            outputs
                .into_iter()
                .map(|(name, export)| {
                    if cli.summary_only {
                        summary_json(&name, &export)
                    } else {
                        export.to_json()
                    }
                })
                .collect::<Vec<_>>()
                .join("\n")
        }
    } else {
        if cli.scan_all {
            render_scan_outputs(&cli, &outputs)
        } else if cli.report_format.is_some() {
            render_report_outputs(&cli, &outputs)
        } else {
            outputs
                .into_iter()
                .map(|(name, export)| summary_line(&name, &export))
                .collect::<Vec<_>>()
                .join("\n")
        }
    };
    if let Some(path) = cli.out_path.as_deref() {
        fs::write(path, format!("{rendered}\n")).unwrap_or_else(|err| {
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
    } else {
        println!("{rendered}");
    }
}

pub(crate) use self::cli::{Cli, IngestMode, ReportFormat, ScanTarget, SocketTarget};

pub(crate) use self::helpers::{
    annotate_export_trust, api_socket_addr_is_local, export_has_operation, filter_export_by_pid,
    ingest_mode_for_export, ingest_mode_note_for_export, list_entries_json, list_entries_text,
    list_protocols_json, list_protocols_text, pid_attribution_note_for_export,
    pid_attribution_status_for_export, route_fact, run_binding_session, run_session,
    scan_targets_for_cli, socket_target_is_local, write_or_print,
};
fn usage() -> &'static str {
    UiLocale::detect().usage()
}
