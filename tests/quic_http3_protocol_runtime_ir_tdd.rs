mod support;

use gewyvern::dsl::compile_file;
use gewyvern::export::{ExportBundle, ProtocolIr};
use gewyvern::flow::ProgramOperation;
use gewyvern::ir::{QuicFrameType, QuicPacketType};
use gewyvern::ledger::PacketDir;
use gewyvern::runtime::{RuntimeSession, SessionConfig};
use std::time::{Duration, SystemTime};
use support::{
    route_fact, sock_lineage_fact, udp_packet_fact_with_dir_and_ports_and_payload,
    udp_quic_meta_fact,
};

fn dsl_fixture_path(name: &str) -> String {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("dsl")
        .join(name)
        .to_string_lossy()
        .into_owned()
}

#[test]
fn quic_retry_runtime_path_materializes_retry_ir_from_legacy_operation() {
    let export = run_quic_retry_path(true);

    assert_operation(&export, "quic_retry_validation");
    assert_stage(&export, "receive_retry");

    let ir = protocol_ir(&export, "quic_retry_validation");
    assert_surface(ir, "quic", "retry", "retry", "secure-transport-session");
    assert_eq!(ir.semantics_category.as_deref(), Some("continuation-path"));
    assert_json_replay(&export);
}

#[test]
fn quic_retry_runtime_ir_does_not_materialize_without_retry_packet() {
    let export = run_quic_retry_path(false);

    assert_no_stage(&export, "receive_retry");
    assert_no_protocol_ir(&export, "quic_retry_validation");
    assert_json_replay(&export);
}

#[test]
fn quic_close_runtime_path_materializes_close_ir_from_observation_operation() {
    let export = run_close_like_path("quic_close_path.gewy", 0x7155, "quic-client", true);

    assert_operation(&export, "quic_close_observation");
    assert_stage(&export, "receive_close");

    let ir = protocol_ir(&export, "quic_close_observation");
    assert_surface(ir, "quic", "close", "close", "secure-transport-session");
    assert_eq!(ir.semantics_category.as_deref(), Some("failure-path"));
    assert_json_replay(&export);
}

#[test]
fn quic_close_runtime_ir_does_not_materialize_without_close_frame() {
    let export = run_close_like_path("quic_close_path.gewy", 0x7156, "quic-client", false);

    assert_no_stage(&export, "receive_close");
    assert_no_protocol_ir(&export, "quic_close_observation");
    assert_json_replay(&export);
}

#[test]
fn http3_close_runtime_path_materializes_close_ir_from_observation_operation() {
    let export = run_http3_close_path(true);

    assert_operation(&export, "http3_close_observation");
    assert_stage(&export, "send_request_stream");
    assert_stage(&export, "receive_close");

    let ir = protocol_ir(&export, "http3_close_observation");
    assert_surface(ir, "http3", "close", "close", "web-proxy-request-response");
    assert_eq!(ir.semantics_category.as_deref(), Some("failure-path"));
    assert_json_replay(&export);
}

#[test]
fn http3_close_runtime_ir_does_not_materialize_without_close_frame() {
    let export = run_http3_close_path(false);

    assert_stage(&export, "send_request_stream");
    assert_no_stage(&export, "receive_close");
    assert_no_protocol_ir(&export, "http3_close_observation");
    assert_json_replay(&export);
}

#[test]
fn http3_server_close_runtime_path_materializes_server_close_ir() {
    let export = run_http3_server_close_path(true);

    assert_operation(&export, "http3_server_close_observation");
    assert_stage(&export, "send_response_stream");
    assert_stage(&export, "send_close");

    let ir = protocol_ir(&export, "http3_server_close_observation");
    assert_surface(
        ir,
        "http3",
        "server-close",
        "server-close",
        "web-proxy-request-response",
    );
    assert_eq!(ir.semantics_category.as_deref(), Some("failure-path"));
    assert_json_replay(&export);
}

#[test]
fn http3_server_close_runtime_ir_does_not_materialize_without_close_frame() {
    let export = run_http3_server_close_path(false);

    assert_stage(&export, "send_response_stream");
    assert_no_stage(&export, "send_close");
    assert_no_protocol_ir(&export, "http3_server_close_observation");
    assert_json_replay(&export);
}

fn run_quic_retry_path(include_retry: bool) -> ExportBundle {
    let binding = compile_file(&dsl_fixture_path("quic_retry_path.gewy")).unwrap();
    let config = SessionConfig::for_template(binding.template).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    let cookie = 0x7172;
    session.ingest(sock_lineage_fact(1, cookie, 4433, "quic-client"));
    session.ingest(route_fact(2, cookie, 7));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        3,
        cookie,
        1280,
        PacketDir::Egress,
        Some(53000),
        Some(443),
        Some(0xc3),
        Some(0xc300),
    ));
    if include_retry {
        session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
            4,
            cookie,
            160,
            PacketDir::Ingress,
            Some(53000),
            Some(443),
            Some(0xf0),
            None,
        ));
    }
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(70));
    session.export_bundle()
}

fn run_close_like_path(
    fixture: &str,
    cookie: u64,
    process_name: &str,
    include_peer_close: bool,
) -> ExportBundle {
    let binding = compile_file(&dsl_fixture_path(fixture)).unwrap();
    let config = SessionConfig::for_template(binding.template).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, cookie, 4433, process_name));
    session.ingest(route_fact(2, cookie, 7));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        3,
        cookie,
        1280,
        PacketDir::Egress,
        Some(53001),
        Some(443),
        Some(0xc3),
        Some(0xc300),
    ));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        4,
        cookie,
        220,
        PacketDir::Ingress,
        Some(53001),
        Some(443),
        Some(0xe0),
        None,
    ));
    ingest_quic_frame(
        &mut session,
        5,
        cookie,
        PacketDir::Egress,
        true,
        Some(QuicPacketType::Initial),
        QuicFrameType::Crypto,
    );
    ingest_quic_frame(
        &mut session,
        6,
        cookie,
        PacketDir::Ingress,
        true,
        Some(QuicPacketType::Handshake),
        QuicFrameType::Crypto,
    );
    if include_peer_close {
        ingest_quic_frame(
            &mut session,
            7,
            cookie,
            PacketDir::Ingress,
            false,
            None,
            QuicFrameType::ConnectionClose,
        );
    }
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(90));
    session.export_bundle()
}

fn run_http3_close_path(include_peer_close: bool) -> ExportBundle {
    let binding = compile_file(&dsl_fixture_path("http3_close_path.gewy")).unwrap();
    let config = SessionConfig::for_template(binding.template).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    let cookie = 0x6833;
    session.ingest(sock_lineage_fact(1, cookie, 4433, "curl"));
    session.ingest(route_fact(2, cookie, 7));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        3,
        cookie,
        1280,
        PacketDir::Egress,
        Some(53110),
        Some(443),
        Some(0xc3),
        Some(0xc300),
    ));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        4,
        cookie,
        220,
        PacketDir::Ingress,
        Some(53110),
        Some(443),
        Some(0xe0),
        None,
    ));
    ingest_quic_frame_with_ports(
        &mut session,
        5,
        cookie,
        PacketDir::Egress,
        Some(53110),
        Some(443),
        true,
        Some(QuicPacketType::Initial),
        QuicFrameType::Crypto,
    );
    ingest_quic_frame_with_ports(
        &mut session,
        6,
        cookie,
        PacketDir::Ingress,
        Some(53110),
        Some(443),
        true,
        Some(QuicPacketType::Handshake),
        QuicFrameType::Crypto,
    );
    ingest_quic_frame_with_ports(
        &mut session,
        7,
        cookie,
        PacketDir::Egress,
        Some(53110),
        Some(443),
        false,
        None,
        QuicFrameType::Stream,
    );
    if include_peer_close {
        ingest_quic_frame_with_ports(
            &mut session,
            8,
            cookie,
            PacketDir::Ingress,
            Some(53110),
            Some(443),
            false,
            None,
            QuicFrameType::ConnectionClose,
        );
    }
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(100));
    session.export_bundle()
}

fn run_http3_server_close_path(include_local_close: bool) -> ExportBundle {
    let binding = compile_file(&dsl_fixture_path("http3_server_close_path.gewy")).unwrap();
    let config = SessionConfig::for_template(binding.template).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    let cookie = 0x6834;
    session.ingest(sock_lineage_fact(1, cookie, 8443, "nginx"));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        2,
        cookie,
        1280,
        PacketDir::Ingress,
        Some(443),
        Some(53112),
        Some(0xc3),
        Some(0xc300),
    ));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        3,
        cookie,
        220,
        PacketDir::Egress,
        Some(443),
        Some(53112),
        Some(0xe0),
        None,
    ));
    ingest_quic_frame_with_ports(
        &mut session,
        4,
        cookie,
        PacketDir::Ingress,
        Some(443),
        Some(53112),
        true,
        Some(QuicPacketType::Initial),
        QuicFrameType::Crypto,
    );
    ingest_quic_frame_with_ports(
        &mut session,
        5,
        cookie,
        PacketDir::Egress,
        Some(443),
        Some(53112),
        true,
        Some(QuicPacketType::Handshake),
        QuicFrameType::Crypto,
    );
    ingest_quic_frame_with_ports(
        &mut session,
        6,
        cookie,
        PacketDir::Ingress,
        Some(443),
        Some(53112),
        false,
        None,
        QuicFrameType::Stream,
    );
    ingest_quic_frame_with_ports(
        &mut session,
        7,
        cookie,
        PacketDir::Egress,
        Some(443),
        Some(53112),
        false,
        None,
        QuicFrameType::Stream,
    );
    if include_local_close {
        ingest_quic_frame_with_ports(
            &mut session,
            8,
            cookie,
            PacketDir::Egress,
            Some(443),
            Some(53112),
            false,
            None,
            QuicFrameType::ConnectionClose,
        );
    }
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(110));
    session.export_bundle()
}

fn ingest_quic_frame(
    session: &mut RuntimeSession,
    id: u64,
    cookie: u64,
    dir: PacketDir,
    long_header: bool,
    packet_type: Option<QuicPacketType>,
    frame_type: QuicFrameType,
) {
    ingest_quic_frame_with_ports(
        session,
        id,
        cookie,
        dir,
        Some(53001),
        Some(443),
        long_header,
        packet_type,
        frame_type,
    );
}

fn ingest_quic_frame_with_ports(
    session: &mut RuntimeSession,
    id: u64,
    cookie: u64,
    dir: PacketDir,
    local_port: Option<u16>,
    remote_port: Option<u16>,
    long_header: bool,
    packet_type: Option<QuicPacketType>,
    frame_type: QuicFrameType,
) {
    session.ingest(udp_quic_meta_fact(
        id,
        cookie,
        dir,
        local_port,
        remote_port,
        long_header,
        packet_type,
        vec![frame_type],
    ));
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
