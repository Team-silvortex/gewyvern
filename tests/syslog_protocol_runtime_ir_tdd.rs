mod support;

use gewyvern::dsl::compile_file;
use gewyvern::export::{ExportBundle, ProtocolIr};
use gewyvern::flow::ProgramOperation;
use gewyvern::ledger::PacketDir;
use gewyvern::runtime::{RuntimeSession, SessionConfig};
use std::time::{Duration, SystemTime};
use support::{
    packet_fact_with_dir_and_payload_bytes, route_fact, sock_lineage_fact,
    tcp_state_fact_with_ports, udp_packet_fact_with_dir_and_ports_and_payload,
};

fn dsl_fixture_path(name: &str) -> String {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("dsl")
        .join(name)
        .to_string_lossy()
        .into_owned()
}

#[test]
fn syslog_udp_runtime_path_materializes_log_ingest_ir() {
    let export = run_udp_syslog_path(0x5140, 0x3c);

    assert_operation(&export, "syslog_udp_message");
    assert_stage(&export, "send_syslog_message");

    let ir = protocol_ir(&export, "syslog_udp_message");
    assert_surface(ir, "udp", "log-ingest");
    assert_eq!(
        ir.semantics_category.as_deref(),
        Some("syslog-udp-message-path")
    );
    assert_json_replay(&export);
}

#[test]
fn syslog_udp_runtime_ir_does_not_materialize_without_pri_marker() {
    let export = run_udp_syslog_path(0x5141, 0x41);

    assert_no_stage(&export, "send_syslog_message");
    assert_no_protocol_ir(&export, "syslog_udp_message");
    assert_json_replay(&export);
}

#[test]
fn syslog_tcp_runtime_path_materializes_log_stream_ir() {
    let export = run_tcp_syslog_path(
        "syslog_tcp_message_path.gewy",
        0x5142,
        514,
        "syslog_tcp_message",
        "send_syslog_frame",
        &[(0, 0x3c), (1, 0x31), (2, 0x33), (3, 0x3e)],
    );

    assert_operation(&export, "syslog_tcp_message");
    assert_stage(&export, "send_syslog_frame");

    let ir = protocol_ir(&export, "syslog_tcp_message");
    assert_surface(ir, "tcp", "log-ingest");
    assert_eq!(
        ir.semantics_category.as_deref(),
        Some("syslog-tcp-message-path")
    );
    assert_json_replay(&export);
}

#[test]
fn syslog_tls_runtime_path_materializes_secure_transport_ir() {
    let export = run_tcp_syslog_path(
        "syslog_tls_transport_path.gewy",
        0x5143,
        6514,
        "syslog_tls_transport",
        "send_tls_client_hello",
        &[(0, 0x16), (1, 0x03), (2, 0x03), (3, 0x00)],
    );

    assert_operation(&export, "syslog_tls_transport");
    assert_stage(&export, "send_tls_client_hello");

    let ir = protocol_ir(&export, "syslog_tls_transport");
    assert_surface(ir, "tls", "secure-transport");
    assert_eq!(
        ir.semantics_category.as_deref(),
        Some("syslog-tls-transport-path")
    );
    assert_json_replay(&export);
}

fn run_udp_syslog_path(cookie: u64, byte0: u8) -> ExportBundle {
    let binding = compile_file(&dsl_fixture_path("syslog_udp_message_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, cookie, 5140, "logger"));
    session.ingest(route_fact(2, cookie, 7));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        3,
        cookie,
        120,
        PacketDir::Egress,
        Some(56000),
        Some(514),
        Some(byte0),
        None,
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(100));
    session.export_bundle()
}

fn run_tcp_syslog_path(
    fixture: &str,
    cookie: u64,
    server_port: u16,
    operation: &str,
    phase: &str,
    payload: &[(u16, u8)],
) -> ExportBundle {
    let binding = compile_file(&dsl_fixture_path(fixture)).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, cookie, 5141, "logger"));
    session.ingest(tcp_state_fact_with_ports(
        2,
        cookie,
        1,
        2,
        56001,
        server_port,
    ));
    session.ingest(route_fact(3, cookie, 6));
    session.ingest(packet_fact_with_dir_and_payload_bytes(
        4,
        cookie,
        0x18,
        PacketDir::Egress,
        Some(56001),
        Some(server_port),
        payload,
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(100));

    let export = session.export_bundle();
    assert_operation(&export, operation);
    assert_stage(&export, phase);
    export
}

fn assert_operation(export: &ExportBundle, operation: &str) {
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom(operation.into())
    );
}

fn assert_surface(ir: &ProtocolIr, entry: &str, shelf: &str) {
    assert_eq!(ir.protocol, "syslog");
    assert_eq!(ir.entry, entry);
    assert_eq!(ir.shelf_key.as_deref(), Some(shelf));
    assert_eq!(
        ir.cluster_key.as_deref(),
        Some("web-proxy-request-response")
    );
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

fn assert_no_stage(export: &ExportBundle, phase: &str) {
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .all(|stage| stage.phase.as_deref() != Some(phase)),
        "unexpected stage {phase}"
    );
}

fn assert_json_replay(export: &ExportBundle) {
    let replayed = ExportBundle::from_json(&export.to_json()).expect("export json should replay");
    assert_eq!(replayed.protocol_ir, export.protocol_ir);
}

fn assert_no_protocol_ir(export: &ExportBundle, operation: &str) {
    assert!(
        export
            .protocol_ir
            .iter()
            .all(|item| item.operation != operation),
        "unexpected protocol IR for {operation}"
    );
}

fn protocol_ir<'a>(export: &'a ExportBundle, operation: &str) -> &'a ProtocolIr {
    export
        .protocol_ir
        .iter()
        .find(|item| item.operation == operation)
        .expect("protocol IR should materialize")
}
