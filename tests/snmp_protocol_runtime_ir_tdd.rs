mod support;

use gewyvern::dsl::compile_file;
use gewyvern::export::{ExportBundle, ProtocolIr};
use gewyvern::flow::ProgramOperation;
use gewyvern::ledger::{
    CpuId, FactEnvelope, FactId, FactKind, PacketDir, PacketMetaFact, SessionId,
};
use gewyvern::runtime::{RuntimeSession, SessionConfig};
use std::time::{Duration, SystemTime};
use support::{
    route_fact, sock_lineage_fact,
    udp_packet_fact_with_dir_and_ports_and_payload_prefix4_and_byte13,
};

fn dsl_fixture_path(name: &str) -> String {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("dsl")
        .join(name)
        .to_string_lossy()
        .into_owned()
}

#[test]
fn snmp_bulk_runtime_path_materializes_management_read_ir() {
    let export = run_snmp_path(
        "snmp_bulk_path.gewy",
        0x5a01,
        "snmpbulkget",
        &[
            snmp_pdu(PacketDir::Egress, 54000, 161, 108, 0x302c, 0x302c0201, 0xa5),
            snmp_pdu(
                PacketDir::Ingress,
                54000,
                161,
                128,
                0x3040,
                0x30400201,
                0xa2,
            ),
        ],
    );

    assert_operation(&export, "snmp_bulk");
    assert_stage(&export, "send_bulk_request");
    assert_stage(&export, "receive_bulk_response");

    let ir = protocol_ir(&export, "snmp_bulk");
    assert_surface(ir, "snmp", "bulk", "read", "network-control-discovery");
    assert_eq!(
        ir.semantics_category.as_deref(),
        Some("snmp-bulk-read-path")
    );
    assert_json_replay(&export);
}

#[test]
fn snmp_inform_runtime_path_materializes_acknowledged_notify_ir() {
    let export = run_snmp_path(
        "snmp_inform_path.gewy",
        0x5a02,
        "snmpinform",
        &[
            snmp_pdu(PacketDir::Egress, 54004, 161, 112, 0x3030, 0x30300201, 0xa6),
            snmp_pdu(
                PacketDir::Ingress,
                54004,
                161,
                120,
                0x3032,
                0x30320201,
                0xa2,
            ),
        ],
    );

    assert_operation(&export, "snmp_inform");
    assert_stage(&export, "send_inform_notification");
    assert_stage(&export, "receive_inform_response");

    let ir = protocol_ir(&export, "snmp_inform");
    assert_surface(ir, "snmp", "inform", "notify", "network-control-discovery");
    assert_eq!(
        ir.semantics_category.as_deref(),
        Some("snmp-acknowledged-notification-path")
    );
    assert_json_replay(&export);
}

#[test]
fn snmp_v3_priv_runtime_path_materializes_private_security_ir() {
    let export = run_snmp_path(
        "snmp_v3_priv_path.gewy",
        0x5a03,
        "snmpget",
        &[
            snmp_v3_pdu(PacketDir::Egress, 54006, 161, 176, 0x3060, 0x30600201, 0x03),
            snmp_v3_pdu(
                PacketDir::Ingress,
                54006,
                161,
                192,
                0x3070,
                0x30700201,
                0x03,
            ),
        ],
    );

    assert_operation(&export, "snmp_v3_priv");
    assert_stage(&export, "send_v3_priv_request");
    assert_stage(&export, "receive_v3_priv_response");

    let ir = protocol_ir(&export, "snmp_v3_priv");
    assert_surface(
        ir,
        "snmp",
        "v3-priv",
        "security",
        "network-control-discovery",
    );
    assert_eq!(
        ir.semantics_category.as_deref(),
        Some("snmpv3-private-path")
    );
    assert_json_replay(&export);
}

fn run_snmp_path(
    fixture: &str,
    cookie: u64,
    process_name: &str,
    packets: &[SnmpPdu],
) -> ExportBundle {
    let binding = compile_file(&dsl_fixture_path(fixture)).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, cookie, 5400, process_name));
    session.ingest(route_fact(2, cookie, 7));

    for (index, pdu) in packets.iter().enumerate() {
        session.ingest(pdu.to_fact(3 + index as u64, cookie));
    }

    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(80));
    session.export_bundle()
}

#[derive(Clone, Copy)]
struct SnmpPdu {
    dir: PacketDir,
    local_port: u16,
    remote_port: u16,
    tot_len: u32,
    payload_prefix2: u16,
    payload_prefix4: u32,
    payload_byte13: u8,
    payload_byte18: Option<u8>,
}

impl SnmpPdu {
    fn to_fact(self, id: u64, cookie: u64) -> FactEnvelope {
        if let Some(payload_byte18) = self.payload_byte18 {
            return snmp_udp_payload_fact(
                id,
                cookie,
                self.tot_len,
                self.dir,
                self.local_port,
                self.remote_port,
                &[
                    (0, 0x30),
                    (1, (self.payload_prefix2 & 0xff) as u8),
                    (2, 0x02),
                    (3, 0x01),
                    (4, self.payload_byte13),
                    (18, payload_byte18),
                ],
            );
        }

        udp_packet_fact_with_dir_and_ports_and_payload_prefix4_and_byte13(
            id,
            cookie,
            self.tot_len,
            self.dir,
            Some(self.local_port),
            Some(self.remote_port),
            Some(0x30),
            Some(self.payload_prefix2),
            Some(self.payload_prefix4),
            Some(self.payload_byte13),
        )
    }
}

fn snmp_pdu(
    dir: PacketDir,
    local_port: u16,
    remote_port: u16,
    tot_len: u32,
    payload_prefix2: u16,
    payload_prefix4: u32,
    payload_byte13: u8,
) -> SnmpPdu {
    SnmpPdu {
        dir,
        local_port,
        remote_port,
        tot_len,
        payload_prefix2,
        payload_prefix4,
        payload_byte13,
        payload_byte18: None,
    }
}

fn snmp_v3_pdu(
    dir: PacketDir,
    local_port: u16,
    remote_port: u16,
    tot_len: u32,
    payload_prefix2: u16,
    payload_prefix4: u32,
    payload_byte18: u8,
) -> SnmpPdu {
    SnmpPdu {
        dir,
        local_port,
        remote_port,
        tot_len,
        payload_prefix2,
        payload_prefix4,
        payload_byte13: 0x03,
        payload_byte18: Some(payload_byte18),
    }
}

fn snmp_udp_payload_fact(
    id: u64,
    cookie: u64,
    tot_len: u32,
    dir: PacketDir,
    local_port: u16,
    remote_port: u16,
    payload_bytes: &[(u16, u8)],
) -> FactEnvelope {
    let byte_at = |target: u16| {
        payload_bytes
            .iter()
            .find_map(|(offset, value)| (*offset == target).then_some(*value))
    };
    let payload_byte0 = byte_at(0);
    let payload_byte1 = byte_at(1);
    let payload_byte2 = byte_at(2);
    let payload_byte3 = byte_at(3);
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
            local_port: Some(local_port),
            remote_port: Some(remote_port),
            payload_byte0,
            payload_byte1,
            payload_prefix2: payload_byte0
                .zip(payload_byte1)
                .map(|(b0, b1)| u16::from_be_bytes([b0, b1])),
            payload_prefix4: payload_byte0
                .zip(payload_byte1)
                .zip(payload_byte2)
                .zip(payload_byte3)
                .map(|(((b0, b1), b2), b3)| u32::from_be_bytes([b0, b1, b2, b3])),
            payload_byte4: byte_at(4),
            payload_byte5: byte_at(5),
            payload_byte9: byte_at(9),
            payload_byte10: byte_at(10),
            payload_byte13: byte_at(13),
            payload_bytes: payload_bytes.iter().copied().collect(),
            l3_proto: 0x0800,
            l4_proto: 17,
            tot_len,
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
