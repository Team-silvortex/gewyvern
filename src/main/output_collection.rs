use gewyvern::export::ExportBundle;
use gewyvern::ledger::{
    CpuId, FactEnvelope, FactId, FactKind, PacketDir, PacketMetaFact, SessionId, TcpStateFact,
};
use gewyvern::socket_input::{
    collect_tcp_socket_facts, collect_unix_socket_facts, run_tcp_socket_session,
    run_tcp_socket_session_with_binding, run_unix_socket_session,
    run_unix_socket_session_with_binding,
};
use gewyvern::template::{handshake_debug_template, udp_debug_template};
use std::time::{Duration, SystemTime};

use crate::runtime_events::{EVENT_SOCKET_SESSION_COLLECT_FAILED, EVENT_SOCKET_SESSION_RUN_FAILED};
use crate::runtime_logging::log_error_event;
use crate::serve_runtime::serve_socket_sessions;
use crate::{
    Cli, ScanTarget, SocketTarget, UiLocale, annotate_export_trust, filter_export_by_pid,
    route_fact, run_binding_demo, run_binding_session, run_session,
};

pub(crate) fn collect_cli_outputs(
    cli: &Cli,
    base: SystemTime,
    scan_targets: &[ScanTarget],
    locale: UiLocale,
) -> Vec<(String, ExportBundle)> {
    let mut outputs: Vec<(String, ExportBundle)> = Vec::new();
    if let Some(socket_target) = cli.socket_target.as_ref() {
        collect_socket_cli_outputs(&mut outputs, cli, socket_target, scan_targets, locale);
    } else {
        collect_non_socket_cli_outputs(&mut outputs, cli, base, scan_targets);
    }
    outputs
}

fn collect_socket_cli_outputs(
    outputs: &mut Vec<(String, ExportBundle)>,
    cli: &Cli,
    socket_target: &SocketTarget,
    scan_targets: &[ScanTarget],
    locale: UiLocale,
) {
    if cli.serve {
        serve_socket_sessions(cli, socket_target);
        return;
    }

    if cli.scan_all {
        let facts = match socket_target {
            SocketTarget::Unix(path) => collect_unix_socket_facts(path),
            SocketTarget::Tcp(addr) => collect_tcp_socket_facts(addr),
        }
        .unwrap_or_else(|err| {
            let endpoint = socket_target_endpoint(socket_target);
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
        for target in scan_targets {
            push_filtered_output(
                outputs,
                cli,
                target.label(),
                run_binding_session(target.binding(), &facts),
            );
        }
        return;
    }

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
        let endpoint = socket_target_endpoint(socket_target);
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
    push_filtered_output(outputs, cli, "socket_session".to_string(), export);
}

fn collect_non_socket_cli_outputs(
    outputs: &mut Vec<(String, ExportBundle)>,
    cli: &Cli,
    base: SystemTime,
    scan_targets: &[ScanTarget],
) {
    if cli.scan_all {
        for target in scan_targets {
            push_filtered_output(
                outputs,
                cli,
                target.label(),
                run_binding_demo(target.binding()),
            );
        }
        return;
    }

    if let Some(binding) = cli.dsl_binding() {
        push_filtered_output(
            outputs,
            cli,
            "dsl_demo".to_string(),
            run_binding_demo(binding),
        );
        return;
    }

    if cli.demo_mode.includes_tcp() {
        push_filtered_output(outputs, cli, "tcp_demo".to_string(), tcp_demo_export(base));
    }

    if cli.demo_mode.includes_udp() {
        push_filtered_output(outputs, cli, "udp_demo".to_string(), udp_demo_export(base));
    }
}

fn push_filtered_output(
    outputs: &mut Vec<(String, ExportBundle)>,
    cli: &Cli,
    label: String,
    export: ExportBundle,
) {
    let export = cli
        .pid
        .map(|pid| filter_export_by_pid(&export, pid))
        .unwrap_or(export);
    outputs.push((label, annotate_export_trust(export, cli)));
}

fn socket_target_endpoint(socket_target: &SocketTarget) -> String {
    match socket_target {
        SocketTarget::Unix(path) => format!("unix:{path}"),
        SocketTarget::Tcp(addr) => format!("tcp:{addr}"),
    }
}

fn tcp_demo_export(base: SystemTime) -> ExportBundle {
    run_session(
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
    )
}

fn udp_demo_export(base: SystemTime) -> ExportBundle {
    run_session(
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
    )
}
