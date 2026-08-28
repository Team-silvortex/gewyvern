use crate::support;

use gewyvern::dsl::compile_file;
use gewyvern::export::{ExportBundle, ProtocolIr};
use gewyvern::flow::ProgramOperation;
use gewyvern::ledger::PacketDir;
use gewyvern::runtime::{RuntimeSession, SessionConfig};
use std::time::{Duration, SystemTime};
use support::{packet_fact_with_dir_and_payload_bytes, route_fact, tcp_state_fact_with_ports};

fn dsl_fixture_path(name: &str) -> String {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("dsl")
        .join(name)
        .to_string_lossy()
        .into_owned()
}

#[test]
fn nats_error_runtime_path_materializes_server_error_ir() {
    let export = run_nats_path(&[(PacketDir::Ingress, prefix4(*b"-ERR"))]);

    assert_operation(&export, "nats_error");
    assert_stage(&export, "receive_error");

    let ir = protocol_ir(&export, "nats_error");
    assert_surface(ir, "nats", "error", "error", "cache-queue-stream");
    assert_eq!(ir.semantics_category.as_deref(), Some("failure-path"));
    assert_json_replay(&export);
}

#[test]
fn nats_error_runtime_ir_does_not_materialize_for_ok_response() {
    let export = run_nats_path(&[(PacketDir::Ingress, prefix4(*b"+OK "))]);

    assert_operation(&export, "nats_error");
    assert_no_stage(&export, "receive_error");
    assert_no_protocol_ir(&export, "nats_error");
    assert_json_replay(&export);
}

fn run_nats_path(packets: &[(PacketDir, &[(u16, u8)])]) -> ExportBundle {
    let binding = compile_file(&dsl_fixture_path("nats_error_path.gewy")).unwrap();
    let config = SessionConfig::for_template(binding.template).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    let cookie = 0x5151;
    session.ingest(route_fact(1, cookie, 2));
    session.ingest(tcp_state_fact_with_ports(2, cookie, 1, 2, 45000, 4222));
    session.ingest(tcp_state_fact_with_ports(3, cookie, 2, 3, 45000, 4222));

    for (index, (dir, payload)) in packets.iter().enumerate() {
        session.ingest(packet_fact_with_dir_and_payload_bytes(
            4 + index as u64,
            cookie,
            0x18,
            *dir,
            Some(45000),
            Some(4222),
            payload,
        ));
    }

    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(100));
    session.export_bundle()
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
