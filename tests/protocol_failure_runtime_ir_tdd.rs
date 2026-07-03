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
fn amqp_auth_denied_runtime_path_materializes_broker_failure_ir() {
    let export = run_tcp_path(
        "amqp_auth_denied_path.gewy",
        0xa11a,
        5672,
        "amqp-client",
        &[
            (
                PacketDir::Egress,
                &[(0, 0x41), (1, 0x4d), (2, 0x51), (3, 0x50)][..],
            ),
            (PacketDir::Ingress, &[(0, 0x01), (10, 0x0a)][..]),
            (PacketDir::Egress, &[(0, 0x01), (10, 0x0b)][..]),
            (PacketDir::Ingress, &[(0, 0x01), (10, 0x32)][..]),
        ],
    );

    assert_operation(&export, "amqp_auth_denied");
    assert_stage(&export, "send_start_ok");
    assert_stage(&export, "receive_connection_close");

    let ir = protocol_ir(&export, "amqp_auth_denied");
    assert_surface(
        ir,
        "amqp",
        "auth-denied",
        "start-negotiation",
        "cache-queue-stream",
    );
    assert_eq!(ir.semantics_category.as_deref(), Some("failure-path"));
    assert_json_replay(&export);
}

#[test]
fn amqp_auth_denied_runtime_ir_does_not_materialize_without_close() {
    let export = run_tcp_path(
        "amqp_auth_denied_path.gewy",
        0xa11b,
        5672,
        "amqp-client",
        &[
            (
                PacketDir::Egress,
                &[(0, 0x41), (1, 0x4d), (2, 0x51), (3, 0x50)][..],
            ),
            (PacketDir::Ingress, &[(0, 0x01), (10, 0x0a)][..]),
            (PacketDir::Egress, &[(0, 0x01), (10, 0x0b)][..]),
            (PacketDir::Ingress, &[(0, 0x01), (10, 0x14)][..]),
        ],
    );

    assert_operation(&export, "amqp_auth_denied");
    assert_stage(&export, "send_start_ok");
    assert_no_stage(&export, "receive_connection_close");
    assert_no_protocol_ir(&export, "amqp_auth_denied");
    assert_json_replay(&export);
}

#[test]
fn kerberos_as_error_runtime_path_materializes_auth_failure_ir() {
    let export = run_udp_path(
        "kerberos_as_error_path.gewy",
        0x6b01,
        88,
        "kinit",
        &[
            (PacketDir::Egress, 0x6a, None),
            (PacketDir::Ingress, 0x7e, None),
        ],
    );

    assert_operation(&export, "kerberos_as_error");
    assert_stage(&export, "send_as_request");
    assert_stage(&export, "receive_error");

    let ir = protocol_ir(&export, "kerberos_as_error");
    assert_surface(
        ir,
        "kerberos",
        "as-error",
        "as",
        "identity-directory-access",
    );
    assert_eq!(ir.semantics_category.as_deref(), Some("failure-path"));
    assert_json_replay(&export);
}

#[test]
fn kerberos_as_error_runtime_ir_does_not_materialize_for_as_reply() {
    let export = run_udp_path(
        "kerberos_as_error_path.gewy",
        0x6b02,
        88,
        "kinit",
        &[
            (PacketDir::Egress, 0x6a, None),
            (PacketDir::Ingress, 0x6b, None),
        ],
    );

    assert_operation(&export, "kerberos_as_error");
    assert_stage(&export, "send_as_request");
    assert_no_stage(&export, "receive_error");
    assert_no_protocol_ir(&export, "kerberos_as_error");
    assert_json_replay(&export);
}

#[test]
fn stun_binding_error_runtime_path_materializes_binding_failure_ir() {
    let export = run_udp_path(
        "stun_binding_error_path.gewy",
        0x5701,
        3478,
        "stun-client",
        &[
            (PacketDir::Egress, 0x00, Some(0x0001)),
            (PacketDir::Ingress, 0x01, Some(0x0111)),
        ],
    );

    assert_operation(&export, "stun_binding_error");
    assert_stage(&export, "send_request");
    assert_stage(&export, "receive_error_response");

    let ir = protocol_ir(&export, "stun_binding_error");
    assert_surface(
        ir,
        "stun",
        "binding-error",
        "binding",
        "network-control-discovery",
    );
    assert_eq!(ir.semantics_category.as_deref(), Some("failure-path"));
    assert_json_replay(&export);
}

#[test]
fn stun_binding_error_runtime_ir_does_not_materialize_for_success_response() {
    let export = run_udp_path(
        "stun_binding_error_path.gewy",
        0x5702,
        3478,
        "stun-client",
        &[
            (PacketDir::Egress, 0x00, Some(0x0001)),
            (PacketDir::Ingress, 0x01, Some(0x0101)),
        ],
    );

    assert_operation(&export, "stun_binding_error");
    assert_stage(&export, "send_request");
    assert_no_stage(&export, "receive_error_response");
    assert_no_protocol_ir(&export, "stun_binding_error");
    assert_json_replay(&export);
}

#[test]
fn otlp_export_error_runtime_path_materializes_collector_failure_ir() {
    let export = run_tcp_path(
        "otlp_export_error_path.gewy",
        0x071f,
        443,
        "otelcol",
        &[(PacketDir::Ingress, &[(3, 0x01), (4, 0x05)][..])],
    );

    assert_operation(&export, "otlp_export_error");
    assert_stage(&export, "receive_error_headers");
    assert_stage(&export, "receive_error_status");

    let ir = protocol_ir(&export, "otlp_export_error");
    assert_surface(
        ir,
        "otlp",
        "export-error",
        "collector-response",
        "web-proxy-request-response",
    );
    assert_eq!(
        ir.semantics_category.as_deref(),
        Some("otlp-export-error-path")
    );
    assert_json_replay(&export);
}

#[test]
fn otlp_export_error_runtime_ir_does_not_materialize_for_success_status() {
    let export = run_tcp_path(
        "otlp_export_error_path.gewy",
        0x0720,
        443,
        "otelcol",
        &[(PacketDir::Ingress, &[(3, 0x00), (4, 0x00)][..])],
    );

    assert_operation(&export, "otlp_export_error");
    assert_no_stage(&export, "receive_error_headers");
    assert_no_stage(&export, "receive_error_status");
    assert_no_protocol_ir(&export, "otlp_export_error");
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
    session.ingest(sock_lineage_fact(1, cookie, 9100, process_name));
    session.ingest(tcp_state_fact_with_ports(
        2,
        cookie,
        1,
        2,
        43125,
        server_port,
    ));
    session.ingest(tcp_state_fact_with_ports(
        3,
        cookie,
        2,
        3,
        43125,
        server_port,
    ));
    session.ingest(route_fact(4, cookie, 10));

    for (index, (dir, payload_bytes)) in packets.iter().enumerate() {
        session.ingest(packet_fact_with_dir_and_payload_bytes(
            5 + index as u64,
            cookie,
            0,
            *dir,
            Some(43125),
            Some(server_port),
            payload_bytes,
        ));
    }

    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(120));
    session.export_bundle()
}

fn run_udp_path(
    fixture: &str,
    cookie: u64,
    server_port: u16,
    process_name: &str,
    packets: &[(PacketDir, u8, Option<u16>)],
) -> ExportBundle {
    let binding = compile_file(&dsl_fixture_path(fixture)).unwrap();
    let config = SessionConfig::for_template(binding.template).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, cookie, 9200, process_name));
    session.ingest(route_fact(2, cookie, 10));

    for (index, (dir, byte0, prefix2)) in packets.iter().enumerate() {
        session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
            3 + index as u64,
            cookie,
            72,
            *dir,
            Some(53001),
            Some(server_port),
            Some(*byte0),
            *prefix2,
        ));
    }

    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(100));
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

fn assert_no_stage(export: &ExportBundle, phase: &str) {
    assert!(
        !export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some(phase)),
        "unexpected stage {phase}"
    );
}

fn assert_no_protocol_ir(export: &ExportBundle, operation: &str) {
    assert!(
        !export
            .protocol_ir
            .iter()
            .any(|item| item.operation == operation),
        "unexpected protocol IR for {operation}"
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
