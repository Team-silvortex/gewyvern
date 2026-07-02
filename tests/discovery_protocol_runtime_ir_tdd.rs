mod support;

use gewyvern::dsl::compile_file;
use gewyvern::export::{ExportBundle, ProtocolIr};
use gewyvern::flow::ProgramOperation;
use gewyvern::ledger::PacketDir;
use gewyvern::runtime::{RuntimeSession, SessionConfig};
use std::time::{Duration, SystemTime};
use support::{
    route_fact, sock_lineage_fact, udp_packet_fact_with_dir_and_ports_and_payload,
    udp_packet_fact_with_dir_and_ports_and_payload_prefix4,
};

fn dsl_fixture_path(name: &str) -> String {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("dsl")
        .join(name)
        .to_string_lossy()
        .into_owned()
}

#[test]
fn mdns_query_runtime_path_materializes_discovery_ir() {
    let export = run_udp_path(
        "mdns_query_path.gewy",
        0x5353,
        5353,
        "avahi-daemon",
        &[
            UdpPacket::prefix4(PacketDir::Egress, 64, 0x00, 0x0000, 0x00000000),
            UdpPacket::prefix4(PacketDir::Ingress, 96, 0x00, 0x0000, 0x00008400),
        ],
    );

    assert_operation(&export, "mdns_query");
    assert_stage(&export, "send_query");
    assert_stage(&export, "receive_response");

    let ir = protocol_ir(&export, "mdns_query");
    assert_surface(ir, "mdns", "query", "query", "network-control-discovery");
    assert_eq!(ir.semantics_category.as_deref(), Some("discovery-path"));
    assert_eq!(ir.typical_signal.as_deref(), Some("query flags 0x0000"));
    assert_json_replay(&export);
}

#[test]
fn ssdp_discovery_runtime_path_materializes_discovery_ir() {
    let export = run_udp_path(
        "ssdp_discovery_path.gewy",
        0x1900,
        1900,
        "ssdp-client",
        &[
            UdpPacket::prefix4(PacketDir::Egress, 96, 0x4d, 0x4d2d, 0x4d2d5345),
            UdpPacket::prefix4(PacketDir::Ingress, 180, 0x48, 0x4854, 0x48545450),
        ],
    );

    assert_operation(&export, "ssdp_discovery");
    assert_stage(&export, "send_search");
    assert_stage(&export, "receive_response");

    let ir = protocol_ir(&export, "ssdp_discovery");
    assert_surface(
        ir,
        "ssdp",
        "discovery",
        "discovery",
        "network-control-discovery",
    );
    assert_eq!(ir.semantics_category.as_deref(), Some("discovery-path"));
    assert_eq!(
        ir.typical_signal.as_deref(),
        Some("M-SEARCH / HTTP response")
    );
    assert_json_replay(&export);
}

#[test]
fn gtpu_echo_runtime_path_materializes_tunnel_liveness_ir() {
    let export = run_udp_path(
        "gtpu_echo_path.gewy",
        0x2152,
        2152,
        "upf-agent",
        &[
            UdpPacket::prefix2(PacketDir::Egress, 64, 0x30, 0x3001),
            UdpPacket::prefix2(PacketDir::Ingress, 64, 0x30, 0x3002),
        ],
    );

    assert_operation(&export, "gtpu_echo");
    assert_stage(&export, "send_echo_request");
    assert_stage(&export, "receive_echo_response");

    let ir = protocol_ir(&export, "gtpu_echo");
    assert_surface(ir, "gtpu", "echo", "echo", "network-control-discovery");
    assert_eq!(
        ir.semantics_category.as_deref(),
        Some("tunnel-liveness-path")
    );
    assert_json_replay(&export);
}

struct UdpPacket {
    dir: PacketDir,
    tot_len: u32,
    byte0: u8,
    prefix2: u16,
    prefix4: Option<u32>,
}

impl UdpPacket {
    fn prefix2(dir: PacketDir, tot_len: u32, byte0: u8, prefix2: u16) -> Self {
        Self {
            dir,
            tot_len,
            byte0,
            prefix2,
            prefix4: None,
        }
    }

    fn prefix4(dir: PacketDir, tot_len: u32, byte0: u8, prefix2: u16, prefix4: u32) -> Self {
        Self {
            dir,
            tot_len,
            byte0,
            prefix2,
            prefix4: Some(prefix4),
        }
    }
}

fn run_udp_path(
    fixture: &str,
    cookie: u64,
    port: u16,
    process_name: &str,
    packets: &[UdpPacket],
) -> ExportBundle {
    let binding = compile_file(&dsl_fixture_path(fixture)).unwrap();
    let config = SessionConfig::for_template(binding.template).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, cookie, 8200, process_name));
    session.ingest(route_fact(2, cookie, 7));

    for (index, packet) in packets.iter().enumerate() {
        let id = 3 + index as u64;
        let fact = match packet.prefix4 {
            Some(prefix4) => udp_packet_fact_with_dir_and_ports_and_payload_prefix4(
                id,
                cookie,
                packet.tot_len,
                packet.dir,
                Some(port),
                Some(port),
                Some(packet.byte0),
                Some(packet.prefix2),
                Some(prefix4),
            ),
            None => udp_packet_fact_with_dir_and_ports_and_payload(
                id,
                cookie,
                packet.tot_len,
                packet.dir,
                Some(port),
                Some(port),
                Some(packet.byte0),
                Some(packet.prefix2),
            ),
        };
        session.ingest(fact);
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
