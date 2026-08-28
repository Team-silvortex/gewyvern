use crate::support;

use gewyvern::dsl::compile_file;
use gewyvern::export::{ExportBundle, ProtocolIr};
use gewyvern::flow::ProgramOperation;
use gewyvern::ledger::PacketDir;
use gewyvern::runtime::{RuntimeSession, SessionConfig};
use std::time::{Duration, SystemTime};
use support::{route_fact, sock_lineage_fact, udp_packet_fact_with_dir_and_ports_and_payload};

fn dsl_fixture_path(name: &str) -> String {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("dsl")
        .join(name)
        .to_string_lossy()
        .into_owned()
}

#[test]
fn tftp_read_runtime_path_materializes_transfer_ir() {
    let export = run_tftp_path(
        "tftp_read_path.gewy",
        0x7466,
        &[(PacketDir::Egress, 0x0001), (PacketDir::Ingress, 0x0003)],
    );

    assert_operation(&export, "tftp_read");
    assert_stage(&export, "send_read_request");
    assert_stage(&export, "receive_data");

    let ir = protocol_ir(&export, "tftp_read");
    assert_surface(ir, "read", "transfer");
    assert_eq!(ir.semantics_category.as_deref(), Some("tftp-read-path"));
    assert_json_replay(&export);
}

#[test]
fn tftp_read_runtime_ir_does_not_materialize_when_error_packet_arrives() {
    let export = run_tftp_path(
        "tftp_read_path.gewy",
        0x7467,
        &[(PacketDir::Egress, 0x0001), (PacketDir::Ingress, 0x0005)],
    );

    assert_no_stage(&export, "receive_data");
    assert_no_protocol_ir(&export, "tftp_read");
    assert_json_replay(&export);
}

#[test]
fn tftp_write_runtime_path_materializes_transfer_ir() {
    let export = run_tftp_path(
        "tftp_write_path.gewy",
        0x7468,
        &[(PacketDir::Egress, 0x0002), (PacketDir::Ingress, 0x0004)],
    );

    assert_operation(&export, "tftp_write");
    assert_stage(&export, "send_write_request");
    assert_stage(&export, "receive_ack");

    let ir = protocol_ir(&export, "tftp_write");
    assert_surface(ir, "write", "transfer");
    assert_eq!(ir.typical_signal.as_deref(), Some("WRQ + ACK"));
    assert_json_replay(&export);
}

#[test]
fn tftp_error_runtime_path_materializes_failure_ir() {
    let export = run_tftp_path(
        "tftp_error_path.gewy",
        0x7469,
        &[(PacketDir::Egress, 0x0001), (PacketDir::Ingress, 0x0005)],
    );

    assert_operation(&export, "tftp_error");
    assert_stage(&export, "send_read_request");
    assert_stage(&export, "receive_error");

    let ir = protocol_ir(&export, "tftp_error");
    assert_surface(ir, "error", "failure");
    assert_eq!(ir.semantics_category.as_deref(), Some("failure-path"));
    assert_json_replay(&export);
}

#[test]
fn tftp_error_runtime_ir_does_not_materialize_when_data_packet_arrives() {
    let export = run_tftp_path(
        "tftp_error_path.gewy",
        0x7470,
        &[(PacketDir::Egress, 0x0001), (PacketDir::Ingress, 0x0003)],
    );

    assert_no_stage(&export, "receive_error");
    assert_no_protocol_ir(&export, "tftp_error");
    assert_json_replay(&export);
}

fn run_tftp_path(fixture: &str, cookie: u64, packets: &[(PacketDir, u16)]) -> ExportBundle {
    let binding = compile_file(&dsl_fixture_path(fixture)).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, cookie, 6900, "tftp-client"));
    session.ingest(route_fact(2, cookie, 7));

    for (index, (dir, opcode)) in packets.iter().enumerate() {
        session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
            3 + index as u64,
            cookie,
            120,
            *dir,
            Some(58000),
            Some(69),
            Some((opcode >> 8) as u8),
            Some(*opcode),
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

fn assert_surface(ir: &ProtocolIr, entry: &str, shelf: &str) {
    assert_eq!(ir.protocol, "tftp");
    assert_eq!(ir.entry, entry);
    assert_eq!(ir.shelf_key.as_deref(), Some(shelf));
    assert_eq!(ir.cluster_key.as_deref(), Some("network-control-discovery"));
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
