mod support;

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
fn rip_request_runtime_path_materializes_distance_vector_ir() {
    let export = run_rip_path(
        "rip_request_path.gewy",
        0x520001,
        &[(PacketDir::Egress, 1, 2, 1)],
    );

    assert_operation(&export, "rip_request");
    assert_stage(&export, "send_request");

    let ir = protocol_ir(&export, "rip_request");
    assert_surface(
        ir,
        "rip",
        "request",
        "exchange",
        "network-control-discovery",
    );
    assert_eq!(
        ir.semantics_category.as_deref(),
        Some("distance-vector-route-request")
    );
    assert_json_replay(&export);
}

#[test]
fn rip_response_runtime_path_materializes_route_update_ir() {
    let export = run_rip_path(
        "rip_response_path.gewy",
        0x520002,
        &[(PacketDir::Ingress, 2, 2, 1)],
    );

    assert_operation(&export, "rip_response");
    assert_stage(&export, "receive_response");

    let ir = protocol_ir(&export, "rip_response");
    assert_surface(
        ir,
        "rip",
        "response",
        "exchange",
        "network-control-discovery",
    );
    assert_eq!(
        ir.semantics_category.as_deref(),
        Some("distance-vector-route-update")
    );
    assert_json_replay(&export);
}

#[test]
fn rip_response_runtime_ir_does_not_materialize_for_request_command() {
    let export = run_rip_path(
        "rip_response_path.gewy",
        0x520003,
        &[(PacketDir::Ingress, 1, 2, 1)],
    );

    assert_no_stage(&export, "receive_response");
    assert_no_protocol_ir(&export, "rip_response");
    assert_json_replay(&export);
}

#[test]
fn rip_unreachable_runtime_path_materializes_withdrawal_ir() {
    let export = run_rip_path(
        "rip_unreachable_path.gewy",
        0x520004,
        &[(PacketDir::Ingress, 2, 2, 16)],
    );

    assert_operation(&export, "rip_unreachable");
    assert_stage(&export, "receive_metric16");

    let ir = protocol_ir(&export, "rip_unreachable");
    assert_surface(
        ir,
        "rip",
        "unreachable",
        "failure",
        "network-control-discovery",
    );
    assert_eq!(
        ir.semantics_category.as_deref(),
        Some("distance-vector-route-withdrawal")
    );
    assert_json_replay(&export);
}

fn run_rip_path(fixture: &str, cookie: u64, packets: &[(PacketDir, u8, u8, u8)]) -> ExportBundle {
    let binding = compile_file(&dsl_fixture_path(fixture)).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, cookie, 520, "ripd"));
    session.ingest(route_fact(2, cookie, 7));
    for (index, (dir, command, version, metric)) in packets.iter().enumerate() {
        session.ingest(udp_payload_fact(
            3 + index as u64,
            cookie,
            *dir,
            *command,
            *version,
            *metric,
        ));
    }
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(100));
    session.export_bundle()
}

fn udp_payload_fact(
    id: u64,
    cookie: u64,
    dir: PacketDir,
    command: u8,
    version: u8,
    metric: u8,
) -> FactEnvelope {
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
            remote_port: Some(520),
            payload_byte0: Some(command),
            payload_byte1: Some(version),
            payload_prefix2: Some(u16::from_be_bytes([command, version])),
            payload_prefix4: Some(u32::from_be_bytes([command, version, 0, 0])),
            payload_byte4: None,
            payload_byte5: None,
            payload_byte9: None,
            payload_byte10: None,
            payload_byte13: None,
            payload_bytes: BTreeMap::from([(0, command), (1, version), (23, metric)]),
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
