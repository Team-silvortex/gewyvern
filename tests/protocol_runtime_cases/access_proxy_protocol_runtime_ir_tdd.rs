use crate::support;

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
fn socks5_denied_runtime_path_materializes_connect_failure_ir() {
    let export = run_socks5_path(
        "socks5_denied_path.gewy",
        0x5051,
        &[
            (PacketDir::Egress, bytes(&[0x05, 0x01])),
            (PacketDir::Ingress, bytes(&[0x05, 0x00])),
            (PacketDir::Egress, bytes(&[0x05, 0x01, 0x00, 0x03])),
            (PacketDir::Ingress, bytes(&[0x05, 0x05, 0x00, 0x01])),
        ],
    );

    assert_operation(&export, "socks5_denied");
    assert_stage(&export, "receive_connect_denied");

    let ir = protocol_ir(&export, "socks5_denied");
    assert_surface(
        ir,
        "socks5",
        "denied",
        "denied",
        "web-proxy-request-response",
    );
    assert_eq!(ir.semantics_category.as_deref(), Some("failure-path"));
    assert_json_replay(&export);
}

#[test]
fn socks5_denied_runtime_ir_does_not_materialize_for_connect_success() {
    let export = run_socks5_path(
        "socks5_denied_path.gewy",
        0x5052,
        &[
            (PacketDir::Egress, bytes(&[0x05, 0x01])),
            (PacketDir::Ingress, bytes(&[0x05, 0x00])),
            (PacketDir::Egress, bytes(&[0x05, 0x01, 0x00, 0x03])),
            (PacketDir::Ingress, bytes(&[0x05, 0x00, 0x00, 0x01])),
        ],
    );

    assert_operation(&export, "socks5_denied");
    assert_stage(&export, "send_connect_request");
    assert_no_stage(&export, "receive_connect_denied");
    assert_no_protocol_ir(&export, "socks5_denied");
    assert_json_replay(&export);
}

#[test]
fn socks5_auth_denied_runtime_path_materializes_auth_failure_ir() {
    let export = run_socks5_path(
        "socks5_auth_denied_path.gewy",
        0x5053,
        &[
            (PacketDir::Egress, bytes(&[0x05, 0x01, 0x02])),
            (PacketDir::Ingress, bytes(&[0x05, 0x02])),
            (PacketDir::Egress, bytes(&[0x01, 0x01])),
            (PacketDir::Ingress, bytes(&[0x01, 0x01])),
        ],
    );

    assert_operation(&export, "socks5_auth_denied");
    assert_stage(&export, "receive_auth_denied");

    let ir = protocol_ir(&export, "socks5_auth_denied");
    assert_surface(
        ir,
        "socks5",
        "auth-denied",
        "auth",
        "web-proxy-request-response",
    );
    assert_eq!(ir.semantics_category.as_deref(), Some("failure-path"));
    assert_json_replay(&export);
}

#[test]
fn socks5_auth_denied_runtime_ir_does_not_materialize_for_auth_success() {
    let export = run_socks5_path(
        "socks5_auth_denied_path.gewy",
        0x5054,
        &[
            (PacketDir::Egress, bytes(&[0x05, 0x01, 0x02])),
            (PacketDir::Ingress, bytes(&[0x05, 0x02])),
            (PacketDir::Egress, bytes(&[0x01, 0x01])),
            (PacketDir::Ingress, bytes(&[0x01, 0x00])),
        ],
    );

    assert_operation(&export, "socks5_auth_denied");
    assert_stage(&export, "send_auth_request");
    assert_no_stage(&export, "receive_auth_denied");
    assert_no_protocol_ir(&export, "socks5_auth_denied");
    assert_json_replay(&export);
}

#[test]
fn socks5_auth_connect_denied_runtime_path_materializes_connect_failure_ir() {
    let export = run_socks5_path(
        "socks5_auth_connect_denied_path.gewy",
        0x5055,
        &[
            (PacketDir::Egress, bytes(&[0x05, 0x01, 0x02])),
            (PacketDir::Ingress, bytes(&[0x05, 0x02])),
            (PacketDir::Egress, bytes(&[0x01, 0x01])),
            (PacketDir::Ingress, bytes(&[0x01, 0x00])),
            (PacketDir::Egress, bytes(&[0x05, 0x01, 0x00, 0x03])),
            (PacketDir::Ingress, bytes(&[0x05, 0x05, 0x00, 0x01])),
        ],
    );

    assert_operation(&export, "socks5_auth_connect_denied");
    assert_stage(&export, "receive_auth_ok");
    assert_stage(&export, "receive_connect_denied");

    let ir = protocol_ir(&export, "socks5_auth_connect_denied");
    assert_surface(
        ir,
        "socks5",
        "auth-connect-denied",
        "denied",
        "web-proxy-request-response",
    );
    assert_eq!(ir.semantics_category.as_deref(), Some("failure-path"));
    assert_json_replay(&export);
}

#[test]
fn socks5_auth_connect_denied_runtime_ir_does_not_materialize_for_connect_success() {
    let export = run_socks5_path(
        "socks5_auth_connect_denied_path.gewy",
        0x5056,
        &[
            (PacketDir::Egress, bytes(&[0x05, 0x01, 0x02])),
            (PacketDir::Ingress, bytes(&[0x05, 0x02])),
            (PacketDir::Egress, bytes(&[0x01, 0x01])),
            (PacketDir::Ingress, bytes(&[0x01, 0x00])),
            (PacketDir::Egress, bytes(&[0x05, 0x01, 0x00, 0x03])),
            (PacketDir::Ingress, bytes(&[0x05, 0x00, 0x00, 0x01])),
        ],
    );

    assert_operation(&export, "socks5_auth_connect_denied");
    assert_stage(&export, "receive_auth_ok");
    assert_no_stage(&export, "receive_connect_denied");
    assert_no_protocol_ir(&export, "socks5_auth_connect_denied");
    assert_json_replay(&export);
}

fn run_socks5_path(
    fixture: &str,
    cookie: u64,
    packets: &[(PacketDir, &[(u16, u8)])],
) -> ExportBundle {
    let binding = compile_file(&dsl_fixture_path(fixture)).unwrap();
    let config = SessionConfig::for_template(binding.template).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, cookie, 8100, "curl"));
    session.ingest(tcp_state_fact_with_ports(2, cookie, 1, 2, 47000, 1080));
    session.ingest(tcp_state_fact_with_ports(3, cookie, 2, 3, 47000, 1080));
    session.ingest(route_fact(4, cookie, 6));

    for (index, (dir, payload)) in packets.iter().enumerate() {
        session.ingest(packet_fact_with_dir_and_payload_bytes(
            5 + index as u64,
            cookie,
            0x18,
            *dir,
            Some(47000),
            Some(1080),
            payload,
        ));
    }

    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(160));
    session.export_bundle()
}

fn bytes(values: &'static [u8]) -> &'static [(u16, u8)] {
    Box::leak(
        values
            .iter()
            .copied()
            .enumerate()
            .map(|(offset, value)| (offset as u16, value))
            .collect::<Vec<_>>()
            .into_boxed_slice(),
    )
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
