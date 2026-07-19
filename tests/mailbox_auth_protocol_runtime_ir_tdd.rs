// Expanded byte fixtures make protocol offsets explicit in these tests.
#![allow(clippy::byte_char_slices)]

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
fn imap_auth_denied_runtime_path_materializes_auth_failure_ir() {
    let export = run_mailbox_path(
        "imap_auth_denied_path.gewy",
        0x1a01,
        143,
        "imap-client",
        &[
            (PacketDir::Ingress, bytes(&[b'*', b' ', b'O', b'K'])),
            (
                PacketDir::Egress,
                bytes(&[b'A', b'0', b'0', b'1', b' ', b'L', b'O', b'G', b'I', b'N']),
            ),
            (
                PacketDir::Ingress,
                bytes(&[b'A', b'0', b'0', b'1', b' ', b'N', b'O']),
            ),
        ],
    );

    assert_operation(&export, "imap_auth_denied");
    assert_stage(&export, "send_auth_request");
    assert_stage(&export, "receive_auth_denied");

    let ir = protocol_ir(&export, "imap_auth_denied");
    assert_surface(ir, "imap", "auth-denied", "auth", "mail-delivery-mailbox");
    assert_eq!(ir.semantics_category.as_deref(), Some("failure-path"));
    assert_json_replay(&export);
}

#[test]
fn imap_auth_denied_runtime_ir_does_not_materialize_for_auth_ok() {
    let export = run_mailbox_path(
        "imap_auth_denied_path.gewy",
        0x1a02,
        143,
        "imap-client",
        &[
            (PacketDir::Ingress, bytes(&[b'*', b' ', b'O', b'K'])),
            (
                PacketDir::Egress,
                bytes(&[b'A', b'0', b'0', b'1', b' ', b'L', b'O', b'G', b'I', b'N']),
            ),
            (
                PacketDir::Ingress,
                bytes(&[b'A', b'0', b'0', b'1', b' ', b'O', b'K']),
            ),
        ],
    );

    assert_operation(&export, "imap_auth_denied");
    assert_stage(&export, "send_auth_request");
    assert_no_stage(&export, "receive_auth_denied");
    assert_no_protocol_ir(&export, "imap_auth_denied");
    assert_json_replay(&export);
}

#[test]
fn pop3_auth_denied_runtime_path_materializes_auth_failure_ir() {
    let export = run_mailbox_path(
        "pop3_auth_denied_path.gewy",
        0x3a01,
        110,
        "pop3-client",
        &[
            (
                PacketDir::Ingress,
                bytes(&[b'+', b'O', b'K', b' ', b' ', b'P', b'O', b'P', b'3']),
            ),
            (PacketDir::Egress, bytes(&[b'U', b'S', b'E', b'R'])),
            (
                PacketDir::Ingress,
                bytes(&[b'+', b'O', b'K', b' ', b' ', b'U', b's', b'e', b'r']),
            ),
            (PacketDir::Egress, bytes(&[b'P', b'A', b'S', b'S'])),
            (
                PacketDir::Ingress,
                bytes(&[b'-', b'E', b'R', b'R', b' ', b'a', b'u', b't', b'h']),
            ),
        ],
    );

    assert_operation(&export, "pop3_auth_denied");
    assert_stage(&export, "send_auth_pass");
    assert_stage(&export, "receive_auth_denied");

    let ir = protocol_ir(&export, "pop3_auth_denied");
    assert_surface(ir, "pop3", "auth-denied", "auth", "mail-delivery-mailbox");
    assert_eq!(ir.semantics_category.as_deref(), Some("failure-path"));
    assert_json_replay(&export);
}

#[test]
fn pop3_auth_denied_runtime_ir_does_not_materialize_for_auth_ok() {
    let export = run_mailbox_path(
        "pop3_auth_denied_path.gewy",
        0x3a02,
        110,
        "pop3-client",
        &[
            (
                PacketDir::Ingress,
                bytes(&[b'+', b'O', b'K', b' ', b' ', b'P', b'O', b'P', b'3']),
            ),
            (PacketDir::Egress, bytes(&[b'U', b'S', b'E', b'R'])),
            (
                PacketDir::Ingress,
                bytes(&[b'+', b'O', b'K', b' ', b' ', b'U', b's', b'e', b'r']),
            ),
            (PacketDir::Egress, bytes(&[b'P', b'A', b'S', b'S'])),
            (
                PacketDir::Ingress,
                bytes(&[b'+', b'O', b'K', b' ', b' ', b'M', b'a', b'i', b'l']),
            ),
        ],
    );

    assert_operation(&export, "pop3_auth_denied");
    assert_stage(&export, "send_auth_pass");
    assert_no_stage(&export, "receive_auth_denied");
    assert_no_protocol_ir(&export, "pop3_auth_denied");
    assert_json_replay(&export);
}

fn run_mailbox_path(
    fixture: &str,
    cookie: u64,
    server_port: u16,
    process_name: &str,
    packets: &[(PacketDir, &[(u16, u8)])],
) -> ExportBundle {
    let binding = compile_file(&dsl_fixture_path(fixture)).unwrap();
    let config = SessionConfig::for_template(binding.template).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, cookie, 8700, process_name));
    session.ingest(route_fact(2, cookie, 2));
    session.ingest(tcp_state_fact_with_ports(
        3,
        cookie,
        1,
        2,
        45200,
        server_port,
    ));
    session.ingest(tcp_state_fact_with_ports(
        4,
        cookie,
        2,
        3,
        45200,
        server_port,
    ));

    for (index, (dir, payload)) in packets.iter().enumerate() {
        session.ingest(packet_fact_with_dir_and_payload_bytes(
            5 + index as u64,
            cookie,
            0x18,
            *dir,
            Some(45200),
            Some(server_port),
            payload,
        ));
    }

    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(180));
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
