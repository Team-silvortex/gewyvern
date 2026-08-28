use crate::support;

use gewyvern::dsl::compile_file;
use gewyvern::export::{ExportBundle, ProtocolIr};
use gewyvern::flow::ProgramOperation;
use gewyvern::ledger::{
    CpuId, FactEnvelope, FactId, FactKind, PacketDir, PacketMetaFact, SessionId,
};
use gewyvern::runtime::{RuntimeSession, SessionConfig};
use std::collections::BTreeMap;
use std::time::{Duration, SystemTime};
use support::{route_fact, sock_lineage_fact};

fn dsl_fixture_path(name: &str) -> String {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("dsl")
        .join(name)
        .to_string_lossy()
        .into_owned()
}

#[test]
fn llmnr_query_runtime_path_materializes_local_name_ir() {
    let export = run_llmnr_path(
        "llmnr_query_path.gewy",
        0x535501,
        &[(PacketDir::Egress, 0, 0)],
    );

    assert_operation(&export, "llmnr_query");
    assert_stage(&export, "send_query");

    let ir = protocol_ir(&export, "llmnr_query");
    assert_surface(ir, "llmnr", "query", "query", "network-control-discovery");
    assert_eq!(
        ir.semantics_category.as_deref(),
        Some("local-name-query-path")
    );
    assert_json_replay(&export);
}

#[test]
fn llmnr_response_runtime_path_materializes_local_name_answer_ir() {
    let export = run_llmnr_path(
        "llmnr_response_path.gewy",
        0x535502,
        &[(PacketDir::Ingress, 0x80, 0)],
    );

    assert_operation(&export, "llmnr_response");
    assert_stage(&export, "receive_response");

    let ir = protocol_ir(&export, "llmnr_response");
    assert_surface(
        ir,
        "llmnr",
        "response",
        "response",
        "network-control-discovery",
    );
    assert_eq!(
        ir.semantics_category.as_deref(),
        Some("local-name-response-path")
    );
    assert_json_replay(&export);
}

#[test]
fn llmnr_response_runtime_ir_does_not_materialize_for_query_flags() {
    let export = run_llmnr_path(
        "llmnr_response_path.gewy",
        0x535503,
        &[(PacketDir::Ingress, 0x00, 0)],
    );

    assert_no_stage(&export, "receive_response");
    assert_no_protocol_ir(&export, "llmnr_response");
    assert_json_replay(&export);
}

#[test]
fn llmnr_error_runtime_path_materializes_local_name_error_ir() {
    let export = run_llmnr_path(
        "llmnr_error_path.gewy",
        0x535504,
        &[(PacketDir::Ingress, 0x80, 3)],
    );

    assert_operation(&export, "llmnr_error");
    assert_stage(&export, "receive_nxdomain");

    let ir = protocol_ir(&export, "llmnr_error");
    assert_surface(ir, "llmnr", "error", "error", "network-control-discovery");
    assert_eq!(
        ir.semantics_category.as_deref(),
        Some("local-name-error-path")
    );
    assert_json_replay(&export);
}

fn run_llmnr_path(fixture: &str, cookie: u64, packets: &[(PacketDir, u8, u8)]) -> ExportBundle {
    let binding = compile_file(&dsl_fixture_path(fixture)).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, cookie, 5355, "systemd-resolve"));
    session.ingest(route_fact(2, cookie, 7));
    for (index, (dir, flags_hi, rcode)) in packets.iter().enumerate() {
        session.ingest(udp_payload_fact(
            3 + index as u64,
            cookie,
            *dir,
            *flags_hi,
            *rcode,
        ));
    }
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(100));
    session.export_bundle()
}

fn udp_payload_fact(id: u64, cookie: u64, dir: PacketDir, flags_hi: u8, rcode: u8) -> FactEnvelope {
    FactEnvelope {
        id: FactId(id),
        ts: SystemTime::UNIX_EPOCH + Duration::from_millis(id * 10),
        cpu: CpuId(0),
        ifindex: Some(2),
        session: SessionId(1),
        fragment_id: "udp_packet_meta_fragment".into(),
        kind: FactKind::PacketMeta(PacketMetaFact {
            netns: 1,
            sk_cookie: Some(cookie),
            dir,
            local_port: Some(49152),
            remote_port: Some(5355),
            payload_byte0: Some(0x12),
            payload_byte1: Some(0x34),
            payload_prefix2: Some(0x1234),
            payload_prefix4: Some(u32::from_be_bytes([0x12, 0x34, flags_hi, rcode])),
            payload_byte4: None,
            payload_byte5: None,
            payload_byte9: None,
            payload_byte10: None,
            payload_byte13: None,
            payload_bytes: BTreeMap::from([(2, flags_hi), (3, rcode)]),
            l3_proto: 0x0800,
            l4_proto: 17,
            tot_len: 180,
            tcp_flags: 0,
            seq: None,
            ack: None,
            window: None,
        }),
    }
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
