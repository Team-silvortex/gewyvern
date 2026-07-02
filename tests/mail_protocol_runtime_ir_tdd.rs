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
fn smtp_data_runtime_path_materializes_message_submit_ir() {
    let export = run_tcp_mail_path(
        "smtp_data_path.gewy",
        0x5a25,
        25,
        "msmtp",
        &[
            (PacketDir::Ingress, prefix4(*b"220 ")),
            (PacketDir::Egress, prefix4(*b"EHLO")),
            (PacketDir::Ingress, prefix4(*b"250 ")),
            (PacketDir::Egress, prefix4(*b"AUTH")),
            (PacketDir::Ingress, prefix4(*b"235 ")),
            (PacketDir::Egress, prefix4(*b"MAIL")),
            (
                PacketDir::Ingress,
                bytes(&[b'2', b'5', b'0', b' ', b'2', b'.', b'1', b'.']),
            ),
            (PacketDir::Egress, prefix4(*b"RCPT")),
            (
                PacketDir::Ingress,
                bytes(&[b'2', b'5', b'0', b' ', b'2', b'.', b'1', b'.', b'5']),
            ),
            (PacketDir::Egress, prefix4(*b"DATA")),
            (PacketDir::Ingress, prefix4(*b"354 ")),
            (PacketDir::Egress, bytes(&[0x0d, 0x0a, b'.', 0x0d, 0x0a])),
            (
                PacketDir::Ingress,
                bytes(&[b'2', b'5', b'0', b' ', b'2', b'.', b'0', b'.', b'0']),
            ),
        ],
    );

    assert_operation(&export, "smtp_data");
    assert_stage(&export, "send_data");
    assert_stage(&export, "receive_message_queued");

    let ir = protocol_ir(&export, "smtp_data");
    assert_surface(ir, "smtp", "data", "data", "mail-delivery-mailbox");
    assert_eq!(
        ir.semantics_category.as_deref(),
        Some("mail-data-submit-path")
    );
    assert_json_replay(&export);
}

#[test]
fn smtp_rcpt_denied_runtime_path_materializes_envelope_failure_ir() {
    let export = run_tcp_mail_path(
        "smtp_rcpt_denied_path.gewy",
        0x5a26,
        25,
        "msmtp",
        &[
            (PacketDir::Ingress, prefix4(*b"220 ")),
            (PacketDir::Egress, prefix4(*b"EHLO")),
            (PacketDir::Ingress, prefix4(*b"250 ")),
            (PacketDir::Egress, prefix4(*b"AUTH")),
            (PacketDir::Ingress, prefix4(*b"235 ")),
            (PacketDir::Egress, prefix4(*b"MAIL")),
            (
                PacketDir::Ingress,
                bytes(&[b'2', b'5', b'0', b' ', b'2', b'.', b'1', b'.']),
            ),
            (PacketDir::Egress, prefix4(*b"RCPT")),
            (PacketDir::Ingress, prefix4(*b"550 ")),
        ],
    );

    assert_operation(&export, "smtp_rcpt_denied");
    assert_stage(&export, "send_rcpt_to");
    assert_stage(&export, "receive_rcpt_denied");

    let ir = protocol_ir(&export, "smtp_rcpt_denied");
    assert_surface(
        ir,
        "smtp",
        "rcpt-denied",
        "envelope",
        "mail-delivery-mailbox",
    );
    assert_eq!(ir.semantics_category.as_deref(), Some("failure-path"));
    assert_json_replay(&export);
}

#[test]
fn imap_select_runtime_path_materializes_mailbox_select_ir() {
    let export = run_tcp_mail_path(
        "imap_select_path.gewy",
        0x1a9a,
        143,
        "imap-client",
        &[
            (PacketDir::Ingress, prefix4(*b"* OK")),
            (
                PacketDir::Egress,
                bytes(&[b'A', b'0', b'0', b'1', b' ', b'L', b'O', b'G', b'I', b'N']),
            ),
            (
                PacketDir::Ingress,
                bytes(&[b'A', b'0', b'0', b'1', b' ', b'O', b'K']),
            ),
            (
                PacketDir::Egress,
                bytes(&[
                    b'A', b'0', b'0', b'2', b' ', b'S', b'E', b'L', b'E', b'C', b'T',
                ]),
            ),
            (
                PacketDir::Ingress,
                bytes(&[b'A', b'0', b'0', b'2', b' ', b'O', b'K']),
            ),
        ],
    );

    assert_operation(&export, "imap_select");
    assert_stage(&export, "send_select");
    assert_stage(&export, "receive_mailbox_selected");

    let ir = protocol_ir(&export, "imap_select");
    assert_surface(ir, "imap", "select", "select", "mail-delivery-mailbox");
    assert_eq!(
        ir.semantics_category.as_deref(),
        Some("mailbox-select-path")
    );
    assert_json_replay(&export);
}

#[test]
fn pop3_list_runtime_path_materializes_mailbox_list_ir() {
    let export = run_tcp_mail_path(
        "pop3_list_path.gewy",
        0x9033,
        110,
        "pop3-client",
        &[
            (
                PacketDir::Ingress,
                bytes(&[b'+', b'O', b'K', b' ', b'P', b'O', b'P', b'3']),
            ),
            (PacketDir::Egress, prefix4(*b"USER")),
            (
                PacketDir::Ingress,
                bytes(&[b'+', b'O', b'K', b' ', b' ', b'U', b's', b'e', b'r']),
            ),
            (PacketDir::Egress, prefix4(*b"PASS")),
            (
                PacketDir::Ingress,
                bytes(&[b'+', b'O', b'K', b' ', b' ', b'M', b'a', b'i', b'l']),
            ),
            (PacketDir::Egress, prefix4(*b"LIST")),
            (
                PacketDir::Ingress,
                bytes(&[b'+', b'O', b'K', b' ', b' ', b'm', b'e', b's', b's']),
            ),
        ],
    );

    assert_operation(&export, "pop3_list");
    assert_stage(&export, "send_list");
    assert_stage(&export, "receive_list_ready");

    let ir = protocol_ir(&export, "pop3_list");
    assert_surface(ir, "pop3", "list", "list", "mail-delivery-mailbox");
    assert_eq!(ir.semantics_category.as_deref(), Some("mailbox-list-path"));
    assert_json_replay(&export);
}

fn run_tcp_mail_path(
    fixture: &str,
    cookie: u64,
    server_port: u16,
    process_name: &str,
    packets: &[(PacketDir, &[(u16, u8)])],
) -> ExportBundle {
    let binding = compile_file(&dsl_fixture_path(fixture)).unwrap();
    let config = SessionConfig::for_template(binding.template).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, cookie, 8800, process_name));
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

    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(220));
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

fn prefix4(values: [u8; 4]) -> &'static [(u16, u8)] {
    Box::leak(
        values
            .into_iter()
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
