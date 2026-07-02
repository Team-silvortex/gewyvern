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
fn rtsp_play_runtime_path_materializes_media_session_ir() {
    let export = run_tcp_path(
        "rtsp_play_path.gewy",
        0x2757,
        554,
        "ffplay",
        &[
            (
                PacketDir::Egress,
                &[(0, b'O'), (1, b'P'), (2, b'T'), (3, b'I')][..],
            ),
            (PacketDir::Ingress, rtsp_response_with_header(*b"Publ")),
            (
                PacketDir::Egress,
                &[(0, b'D'), (1, b'E'), (2, b'S'), (3, b'C')][..],
            ),
            (PacketDir::Ingress, rtsp_response_with_header(*b"Cont")),
            (
                PacketDir::Egress,
                &[(0, b'S'), (1, b'E'), (2, b'T'), (3, b'U')][..],
            ),
            (PacketDir::Ingress, rtsp_response_with_header(*b"Sess")),
            (
                PacketDir::Egress,
                &[(0, b'P'), (1, b'L'), (2, b'A'), (3, b'Y')][..],
            ),
            (PacketDir::Ingress, rtsp_response_with_header(*b"Rang")),
        ],
    );

    assert_operation(&export, "rtsp_play");
    assert_stage(&export, "send_play");
    assert_stage(&export, "receive_play_ok");

    let ir = protocol_ir(&export, "rtsp_play");
    assert_surface(ir, "rtsp", "play", "play", "session-control-media-transfer");
    assert_eq!(ir.semantics_category.as_deref(), Some("rtsp-play-path"));
    assert_json_replay(&export);
}

#[test]
fn ftp_retr_runtime_path_materializes_passive_transfer_ir() {
    let export = run_tcp_path(
        "ftp_retr_path.gewy",
        0x3750,
        21,
        "curl",
        &[
            (PacketDir::Ingress, prefix4(*b"220 ")),
            (PacketDir::Egress, prefix4(*b"USER")),
            (PacketDir::Ingress, prefix4(*b"331 ")),
            (PacketDir::Egress, prefix4(*b"PASS")),
            (PacketDir::Ingress, prefix4(*b"230 ")),
            (PacketDir::Egress, prefix4(*b"PASV")),
            (PacketDir::Ingress, prefix4(*b"227 ")),
            (PacketDir::Egress, prefix4(*b"RETR")),
            (PacketDir::Ingress, prefix4(*b"150 ")),
            (PacketDir::Ingress, prefix4(*b"226 ")),
        ],
    );

    assert_operation(&export, "ftp_retr");
    assert_stage(&export, "send_retr");
    assert_stage(&export, "receive_transfer_complete");

    let ir = protocol_ir(&export, "ftp_retr");
    assert_surface(
        ir,
        "ftp",
        "retr",
        "passive",
        "session-control-media-transfer",
    );
    assert_eq!(
        ir.semantics_category.as_deref(),
        Some("ftp-passive-download-path")
    );
    assert_json_replay(&export);
}

#[test]
fn ftp_active_retr_runtime_path_materializes_active_transfer_ir() {
    let export = run_tcp_path(
        "ftp_active_retr_path.gewy",
        0x3751,
        21,
        "curl",
        &[
            (PacketDir::Ingress, prefix4(*b"220 ")),
            (PacketDir::Egress, prefix4(*b"USER")),
            (PacketDir::Ingress, prefix4(*b"331 ")),
            (PacketDir::Egress, prefix4(*b"PASS")),
            (PacketDir::Ingress, prefix4(*b"230 ")),
            (PacketDir::Egress, prefix4(*b"PORT")),
            (PacketDir::Ingress, prefix4(*b"200 ")),
            (PacketDir::Egress, prefix4(*b"RETR")),
            (PacketDir::Ingress, prefix4(*b"150 ")),
            (PacketDir::Ingress, prefix4(*b"226 ")),
        ],
    );

    assert_operation(&export, "ftp_active_retr");
    assert_stage(&export, "send_port");
    assert_stage(&export, "receive_transfer_complete");

    let ir = protocol_ir(&export, "ftp_active_retr");
    assert_surface(
        ir,
        "ftp",
        "active-retr",
        "active",
        "session-control-media-transfer",
    );
    assert_eq!(
        ir.semantics_category.as_deref(),
        Some("ftp-active-download-path")
    );
    assert_json_replay(&export);
}

#[test]
fn ftp_denied_runtime_path_materializes_login_failure_ir() {
    let export = run_tcp_path(
        "ftp_denied_path.gewy",
        0x3752,
        21,
        "curl",
        &[
            (PacketDir::Ingress, prefix4(*b"220 ")),
            (PacketDir::Egress, prefix4(*b"USER")),
            (PacketDir::Ingress, prefix4(*b"331 ")),
            (PacketDir::Egress, prefix4(*b"PASS")),
            (PacketDir::Ingress, prefix4(*b"530 ")),
        ],
    );

    assert_operation(&export, "ftp_denied");
    assert_stage(&export, "receive_auth_denied");

    let ir = protocol_ir(&export, "ftp_denied");
    assert_surface(
        ir,
        "ftp",
        "denied",
        "session",
        "session-control-media-transfer",
    );
    assert_eq!(ir.semantics_category.as_deref(), Some("failure-path"));
    assert_json_replay(&export);
}

#[test]
fn rtsp_setup_runtime_path_materializes_media_setup_ir() {
    let export = run_tcp_path(
        "rtsp_setup_path.gewy",
        0x2758,
        554,
        "ffplay",
        &[
            (
                PacketDir::Egress,
                &[(0, b'O'), (1, b'P'), (2, b'T'), (3, b'I')][..],
            ),
            (PacketDir::Ingress, rtsp_response_with_header(*b"Publ")),
            (
                PacketDir::Egress,
                &[(0, b'D'), (1, b'E'), (2, b'S'), (3, b'C')][..],
            ),
            (PacketDir::Ingress, rtsp_response_with_header(*b"Cont")),
            (
                PacketDir::Egress,
                &[(0, b'S'), (1, b'E'), (2, b'T'), (3, b'U')][..],
            ),
            (PacketDir::Ingress, rtsp_response_with_header(*b"Sess")),
        ],
    );

    assert_operation(&export, "rtsp_setup");
    assert_stage(&export, "send_setup");
    assert_stage(&export, "receive_setup_ok");

    let ir = protocol_ir(&export, "rtsp_setup");
    assert_surface(
        ir,
        "rtsp",
        "setup",
        "setup",
        "session-control-media-transfer",
    );
    assert_eq!(ir.semantics_category.as_deref(), Some("rtsp-setup-path"));
    assert_json_replay(&export);
}

#[test]
fn sip_denied_runtime_path_materializes_session_failure_ir() {
    let export = run_udp_path(
        "sip_denied_path.gewy",
        0x5150,
        "softphone",
        &[(PacketDir::Ingress, &[(8, b'4')][..])],
    );

    assert_operation(&export, "sip_denied");
    assert_stage(&export, "receive_4xx");

    let ir = protocol_ir(&export, "sip_denied");
    assert_surface(
        ir,
        "sip",
        "denied",
        "denied",
        "session-control-media-transfer",
    );
    assert_eq!(ir.semantics_category.as_deref(), Some("failure-path"));
    assert_json_replay(&export);
}

#[test]
fn sip_invite_runtime_path_materializes_call_setup_ir() {
    let export = run_udp_path(
        "sip_invite_path.gewy",
        0x5152,
        "softphone",
        &[
            (PacketDir::Egress, prefix4(*b"INVI")),
            (PacketDir::Ingress, prefix4(*b"SIP/")),
        ],
    );

    assert_operation(&export, "sip_invite");
    assert_stage(&export, "send_invite");
    assert_stage(&export, "receive_ok");

    let ir = protocol_ir(&export, "sip_invite");
    assert_surface(
        ir,
        "sip",
        "invite",
        "invite",
        "session-control-media-transfer",
    );
    assert_eq!(ir.semantics_category.as_deref(), Some("sip-invite-path"));
    assert_json_replay(&export);
}

#[test]
fn sip_register_runtime_path_materializes_registration_ir() {
    let export = run_udp_path(
        "sip_register_path.gewy",
        0x5151,
        "softphone",
        &[
            (PacketDir::Egress, prefix4(*b"REGI")),
            (PacketDir::Ingress, prefix4(*b"SIP/")),
        ],
    );

    assert_operation(&export, "sip_register");
    assert_stage(&export, "send_register");
    assert_stage(&export, "receive_ok");

    let ir = protocol_ir(&export, "sip_register");
    assert_surface(
        ir,
        "sip",
        "register",
        "register",
        "session-control-media-transfer",
    );
    assert_eq!(ir.semantics_category.as_deref(), Some("sip-register-path"));
    assert_json_replay(&export);
}

fn run_tcp_path(
    fixture: &str,
    cookie: u64,
    server_port: u16,
    process_name: &str,
    packets: &[(PacketDir, &[(u16, u8)])],
) -> ExportBundle {
    let binding = compile_file(&dsl_fixture_path(fixture)).unwrap();
    let config = SessionConfig::for_template(binding.template).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, cookie, 8600, process_name));
    session.ingest(tcp_state_fact_with_ports(
        2,
        cookie,
        1,
        2,
        50100,
        server_port,
    ));
    session.ingest(tcp_state_fact_with_ports(
        3,
        cookie,
        2,
        3,
        50100,
        server_port,
    ));
    session.ingest(route_fact(4, cookie, 9));

    for (index, (dir, payload_bytes)) in packets.iter().enumerate() {
        session.ingest(packet_fact_with_dir_and_payload_bytes(
            5 + index as u64,
            cookie,
            0x18,
            *dir,
            Some(50100),
            Some(server_port),
            payload_bytes,
        ));
    }

    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(180));
    session.export_bundle()
}

fn run_udp_path(
    fixture: &str,
    cookie: u64,
    process_name: &str,
    packets: &[(PacketDir, &[(u16, u8)])],
) -> ExportBundle {
    let binding = compile_file(&dsl_fixture_path(fixture)).unwrap();
    let config = SessionConfig::for_template(binding.template).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, cookie, 8610, process_name));
    session.ingest(route_fact(2, cookie, 9));

    for (index, (dir, payload_bytes)) in packets.iter().enumerate() {
        session.ingest(udp_packet_fact_with_payload_bytes(
            3 + index as u64,
            cookie,
            96,
            *dir,
            5061,
            5060,
            payload_bytes,
        ));
    }

    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(100));
    session.export_bundle()
}

fn udp_packet_fact_with_payload_bytes(
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
            payload_bytes: payload_bytes.iter().copied().collect::<BTreeMap<_, _>>(),
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

fn prefix4(bytes: [u8; 4]) -> &'static [(u16, u8)] {
    Box::leak(
        bytes
            .into_iter()
            .enumerate()
            .map(|(offset, value)| (offset as u16, value))
            .collect::<Vec<_>>()
            .into_boxed_slice(),
    )
}

fn rtsp_response_with_header(header: [u8; 4]) -> &'static [(u16, u8)] {
    let mut bytes = vec![
        (0, b'R'),
        (1, b'T'),
        (2, b'S'),
        (3, b'P'),
        (9, b'2'),
        (10, b'0'),
        (11, b'0'),
    ];
    bytes.extend(
        header
            .into_iter()
            .enumerate()
            .map(|(offset, value)| (17 + offset as u16, value)),
    );
    Box::leak(bytes.into_boxed_slice())
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
