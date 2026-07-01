mod support;

use gewyvern::dsl::compile_file;
use gewyvern::export::{ExportBundle, ProtocolIr};
use gewyvern::flow::ProgramOperation;
use gewyvern::ledger::PacketDir;
use gewyvern::runtime::{RuntimeSession, SessionConfig};
use std::time::{Duration, SystemTime};
use support::{
    packet_fact_with_dir_and_payload_bytes, route_fact, sock_lineage_fact,
    tcp_state_fact_with_ports,
};

fn dsl_fixture_path(name: &str) -> String {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("dsl")
        .join(name)
        .to_string_lossy()
        .into_owned()
}

#[test]
fn dns_tcp_runtime_path_materializes_query_ir_from_legacy_operation() {
    let export = run_tcp_path(
        "dns_tcp_query_path.gewy",
        0xd053,
        53,
        "dig",
        &[
            (PacketDir::Egress, &[(4, 0x00)][..]),
            (PacketDir::Ingress, &[(4, 0x80)][..]),
        ],
    );

    assert_operation(&export, "dns_tcp_query");
    assert_stage(&export, "send_query");
    assert_stage(&export, "receive_response");

    let ir = protocol_ir(&export, "dns_tcp_query");
    assert_surface(ir, "dns", "tcp", "tcp", "network-control-discovery");
    assert_eq!(
        ir.semantics_category.as_deref(),
        Some("name-resolution-path")
    );
    assert_json_replay(&export);
}

#[test]
fn tls_client_runtime_path_materializes_client_ir() {
    let export = run_tcp_path(
        "tls_client_path.gewy",
        0x7151,
        443,
        "curl",
        &[(PacketDir::Egress, &[(0, 0x16)][..])],
    );

    assert_operation(&export, "tls_client");
    assert_stage(&export, "send_client_hello");

    let ir = protocol_ir(&export, "tls_client");
    assert_surface(ir, "tls", "client", "client", "secure-transport-session");
    assert_eq!(ir.semantics_category.as_deref(), Some("tls-client-path"));
    assert_json_replay(&export);
}

#[test]
fn tls_alert_runtime_path_materializes_failure_ir() {
    let export = run_tcp_path(
        "tls_alert_path.gewy",
        0x7152,
        443,
        "curl",
        &[(PacketDir::Ingress, &[(0, 0x15)][..])],
    );

    assert_operation(&export, "tls_alert");
    assert_stage(&export, "receive_alert");

    let ir = protocol_ir(&export, "tls_alert");
    assert_surface(
        ir,
        "tls",
        "alert",
        "handshake-signal",
        "secure-transport-session",
    );
    assert_eq!(ir.semantics_category.as_deref(), Some("failure-path"));
    assert_json_replay(&export);
}

#[test]
fn grpc_call_runtime_path_materializes_rpc_ir() {
    let export = run_tcp_path(
        "grpc_call_path.gewy",
        0x6771,
        443,
        "grpcurl",
        &[
            (PacketDir::Egress, &[(3, 0x01)][..]),
            (PacketDir::Egress, &[(3, 0x00)][..]),
            (PacketDir::Ingress, &[(3, 0x00)][..]),
        ],
    );

    assert_operation(&export, "grpc_call");
    assert_stage(&export, "send_headers");
    assert_stage(&export, "send_message");
    assert_stage(&export, "receive_message");

    let ir = protocol_ir(&export, "grpc_call");
    assert_surface(ir, "grpc", "call", "call", "web-proxy-request-response");
    assert_eq!(ir.semantics_category.as_deref(), Some("rpc-call-path"));
    assert_json_replay(&export);
}

#[test]
fn websocket_upgrade_runtime_path_materializes_upgrade_ir() {
    let export = run_tcp_path(
        "websocket_upgrade_path.gewy",
        0x7751,
        80,
        "browser",
        &[
            (
                PacketDir::Egress,
                &[(0, 0x47), (1, 0x45), (2, 0x54), (3, 0x20)][..],
            ),
            (
                PacketDir::Ingress,
                &[
                    (0, 0x48),
                    (1, 0x54),
                    (2, 0x54),
                    (3, 0x50),
                    (9, 0x31),
                    (10, 0x30),
                    (11, 0x31),
                ][..],
            ),
        ],
    );

    assert_operation(&export, "websocket_upgrade");
    assert_stage(&export, "send_upgrade_request");
    assert_stage(&export, "receive_switching_protocols");

    let ir = protocol_ir(&export, "websocket_upgrade");
    assert_surface(
        ir,
        "websocket",
        "upgrade",
        "upgrade",
        "web-proxy-request-response",
    );
    assert_eq!(
        ir.semantics_category.as_deref(),
        Some("websocket-upgrade-path")
    );
    assert_json_replay(&export);
}

#[test]
fn websocket_close_runtime_path_materializes_close_ir() {
    let export = run_tcp_path(
        "websocket_close_path.gewy",
        0x7752,
        80,
        "browser",
        &[(PacketDir::Egress, &[(0, 0x08)][..])],
    );

    assert_operation(&export, "websocket_close");
    assert_stage(&export, "send_close");

    let ir = protocol_ir(&export, "websocket_close");
    assert_surface(
        ir,
        "websocket",
        "close",
        "close",
        "web-proxy-request-response",
    );
    assert_eq!(ir.semantics_category.as_deref(), Some("failure-path"));
    assert_json_replay(&export);
}

fn run_tcp_path(
    fixture: &str,
    cookie: u64,
    server_port: u16,
    process_name: &str,
    packets: &[(PacketDir, &[(u16, u8)])],
) -> ExportBundle {
    let binding = compile_file(&dsl_fixture_path(fixture)).unwrap();
    let config = SessionConfig::for_template(binding.template).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, cookie, 8200, process_name));
    session.ingest(tcp_state_fact_with_ports(
        2,
        cookie,
        1,
        2,
        47000,
        server_port,
    ));
    session.ingest(tcp_state_fact_with_ports(
        3,
        cookie,
        2,
        3,
        47000,
        server_port,
    ));
    session.ingest(route_fact(4, cookie, 6));

    for (index, (dir, payload_bytes)) in packets.iter().enumerate() {
        session.ingest(packet_fact_with_dir_and_payload_bytes(
            5 + index as u64,
            cookie,
            0x18,
            *dir,
            Some(47000),
            Some(server_port),
            payload_bytes,
        ));
    }

    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(110));
    session.export_bundle()
}

fn assert_operation(export: &ExportBundle, operation: &str) {
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom(operation.into())
    );
}

fn assert_surface(ir: &ProtocolIr, protocol: &str, entry: &str, shelf: &str, cluster: &str) {
    assert_eq!(ir.protocol, protocol);
    assert_eq!(ir.entry, entry);
    assert_eq!(ir.shelf_key.as_deref(), Some(shelf));
    assert_eq!(ir.cluster_key.as_deref(), Some(cluster));
}

fn assert_stage(export: &ExportBundle, phase: &str) {
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some(phase)),
        "missing stage {phase}"
    );
}

fn assert_json_replay(export: &ExportBundle) {
    let replayed = ExportBundle::from_json(&export.to_json()).expect("export json should replay");
    assert_eq!(replayed.protocol_ir, export.protocol_ir);
}

fn protocol_ir<'a>(export: &'a ExportBundle, operation: &str) -> &'a ProtocolIr {
    export
        .protocol_ir
        .iter()
        .find(|item| item.operation == operation)
        .expect("protocol IR should materialize")
}
