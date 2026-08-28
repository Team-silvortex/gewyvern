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
use support::{
    packet_fact_with_dir_and_payload_bytes, route_fact, udp_packet_fact_with_dir_and_ports_and_byte,
};

fn dsl_fixture_path(name: &str) -> String {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("dsl")
        .join(name)
        .to_string_lossy()
        .into_owned()
}

#[test]
fn vxlan_vni_runtime_path_materializes_overlay_ir() {
    let export = run_udp_overlay_path("vxlan_vni_path.gewy", 4789, Some(0x08));

    assert_operation(&export, "vxlan_vni");
    assert_stage(&export, "send_vni_marked_packet");
    assert_stage(&export, "receive_vni_marked_packet");

    let ir = protocol_ir(&export, "vxlan_vni");
    assert_surface(ir, "vxlan", "vni", "overlay", "network-control-discovery");
    assert_eq!(
        ir.semantics_category.as_deref(),
        Some("overlay-tenant-path")
    );
    assert_json_replay(&export);
}

#[test]
fn geneve_options_runtime_path_materializes_overlay_ir() {
    let export = run_udp_overlay_path("geneve_options_path.gewy", 6081, Some(0x04));

    assert_operation(&export, "geneve_options");
    assert_stage(&export, "send_optioned_packet");
    assert_stage(&export, "receive_optioned_packet");

    let ir = protocol_ir(&export, "geneve_options");
    assert_surface(
        ir,
        "geneve",
        "options",
        "overlay",
        "network-control-discovery",
    );
    assert_eq!(
        ir.semantics_category.as_deref(),
        Some("overlay-option-path")
    );
    assert_json_replay(&export);
}

#[test]
fn l2tp_control_runtime_path_materializes_tunnel_ir() {
    let export = run_udp_overlay_path("l2tp_control_path.gewy", 1701, Some(0xc8));

    assert_operation(&export, "l2tp_control");
    assert_stage(&export, "send_control_message");
    assert_stage(&export, "receive_control_message");

    let ir = protocol_ir(&export, "l2tp_control");
    assert_surface(ir, "l2tp", "control", "tunnel", "network-control-discovery");
    assert_eq!(
        ir.semantics_category.as_deref(),
        Some("tunnel-control-path")
    );
    assert_json_replay(&export);
}

#[test]
fn pptp_control_runtime_path_materializes_tunnel_ir() {
    let binding = compile_file(&dsl_fixture_path("pptp_control_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(route_fact(1, 0x7070, 2));
    session.ingest(pptp_control_packet(2, PacketDir::Egress));
    session.ingest(pptp_control_packet(3, PacketDir::Ingress));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(80));

    let export = session.export_bundle();
    assert_operation(&export, "pptp_control");
    assert_stage(&export, "send_control_message");
    assert_stage(&export, "receive_control_message");

    let ir = protocol_ir(&export, "pptp_control");
    assert_surface(ir, "pptp", "control", "tunnel", "network-control-discovery");
    assert_eq!(
        ir.semantics_category.as_deref(),
        Some("tunnel-control-path")
    );
    assert_json_replay(&export);
}

#[test]
fn gre_encap_runtime_path_materializes_tunnel_ir() {
    let binding = compile_file(&dsl_fixture_path("gre_encap_path.gewy")).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(gre_packet_fact(1, PacketDir::Egress, 0x2000));
    session.ingest(gre_packet_fact(2, PacketDir::Ingress, 0x2000));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(60));

    let export = session.export_bundle();
    assert_operation(&export, "gre_encap");
    assert_stage(&export, "send_encapsulated_packet");
    assert_stage(&export, "receive_encapsulated_packet");

    let ir = protocol_ir(&export, "gre_encap");
    assert_surface(ir, "gre", "encap", "tunnel", "network-control-discovery");
    assert_eq!(
        ir.semantics_category.as_deref(),
        Some("tunnel-encapsulation-path")
    );
    assert_json_replay(&export);
}

fn run_udp_overlay_path(
    fixture: &str,
    overlay_port: u16,
    payload_byte0: Option<u8>,
) -> ExportBundle {
    let binding = compile_file(&dsl_fixture_path(fixture)).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(route_fact(1, 0x7701, 2));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_byte(
        2,
        0x7701,
        128,
        PacketDir::Egress,
        Some(41000),
        Some(overlay_port),
        payload_byte0,
    ));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_byte(
        3,
        0x7701,
        128,
        PacketDir::Ingress,
        Some(41000),
        Some(overlay_port),
        payload_byte0,
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(80));
    session.export_bundle()
}

fn pptp_control_packet(id: u64, dir: PacketDir) -> FactEnvelope {
    packet_fact_with_dir_and_payload_bytes(
        id,
        0x7070,
        0x18,
        dir,
        Some(43000),
        Some(1723),
        &[(4, 0x1a), (5, 0x2b), (6, 0x3c), (7, 0x4d)],
    )
}

fn gre_packet_fact(id: u64, dir: PacketDir, prefix2: u16) -> FactEnvelope {
    let [byte0, byte1] = prefix2.to_be_bytes();
    FactEnvelope {
        id: FactId(id),
        ts: SystemTime::UNIX_EPOCH + Duration::from_millis(id * 10),
        cpu: CpuId(0),
        ifindex: Some(2),
        session: SessionId(1),
        fragment_id: "udp_packet_meta_fragment".into(),
        kind: FactKind::PacketMeta(PacketMetaFact {
            netns: 1,
            sk_cookie: None,
            dir,
            local_port: None,
            remote_port: None,
            payload_byte0: Some(byte0),
            payload_byte1: Some(byte1),
            payload_prefix2: Some(prefix2),
            payload_prefix4: None,
            payload_byte4: None,
            payload_byte5: None,
            payload_byte9: None,
            payload_byte10: None,
            payload_byte13: None,
            payload_bytes: BTreeMap::from([(0, byte0), (1, byte1)]),
            l3_proto: 0x0800,
            l4_proto: 47,
            tot_len: 64,
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
