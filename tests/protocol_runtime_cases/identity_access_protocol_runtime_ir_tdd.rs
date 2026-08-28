use crate::support;

use gewyvern::dsl::compile_file;
use gewyvern::export::{ExportBundle, ProtocolIr};
use gewyvern::flow::ProgramOperation;
use gewyvern::ledger::PacketDir;
use gewyvern::runtime::{RuntimeSession, SessionConfig};
use std::time::{Duration, SystemTime};
use support::{
    packet_fact_with_dir_and_payload_bytes, route_fact, sock_lineage_fact,
    tcp_state_fact_with_ports, udp_packet_fact_with_dir_and_ports_and_payload,
};

fn dsl_fixture_path(name: &str) -> String {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("dsl")
        .join(name)
        .to_string_lossy()
        .into_owned()
}

#[test]
fn ldap_sync_runtime_path_materializes_directory_ir() {
    let export = run_tcp_path(
        "ldap_directory_sync_session.gewy",
        0x1da9,
        389,
        "ldapsearch",
        &[
            (PacketDir::Egress, &[(5, 0x60)][..]),
            (PacketDir::Ingress, &[(5, 0x61)][..]),
            (PacketDir::Egress, &[(5, 0x63)][..]),
            (PacketDir::Ingress, &[(5, 0x65)][..]),
            (PacketDir::Egress, &[(5, 0x66)][..]),
            (PacketDir::Ingress, &[(5, 0x67), (9, 0x00)][..]),
        ],
    );

    assert_operation(&export, "ldap_directory_sync_session");
    assert_stage(&export, "send_bind");
    assert_stage(&export, "receive_search_result");
    assert_stage(&export, "receive_modify_response");

    let ir = protocol_ir(&export, "ldap_directory_sync_session");
    assert_surface(
        ir,
        "ldap",
        "sync",
        "write-sync",
        "identity-directory-access",
    );
    assert_eq!(
        ir.semantics_category.as_deref(),
        Some("directory-sync-path")
    );
    assert_json_replay(&export);
}

#[test]
fn ssh_channel_runtime_path_materializes_channel_ir() {
    let export = run_tcp_path(
        "ssh_channel_session_path.gewy",
        0x55c9,
        22,
        "ssh",
        &[
            (
                PacketDir::Ingress,
                &[(0, 0x53), (1, 0x53), (2, 0x48), (3, 0x2d)][..],
            ),
            (
                PacketDir::Egress,
                &[(0, 0x53), (1, 0x53), (2, 0x48), (3, 0x2d)][..],
            ),
            (PacketDir::Egress, &[(5, 0x14)][..]),
            (PacketDir::Egress, &[(5, 0x32)][..]),
            (PacketDir::Ingress, &[(5, 0x34)][..]),
            (PacketDir::Egress, &[(5, 0x5a)][..]),
            (PacketDir::Ingress, &[(5, 0x5b)][..]),
        ],
    );

    assert_operation(&export, "ssh_channel_session");
    assert_stage(&export, "receive_server_banner");
    assert_stage(&export, "send_channel_open");
    assert_stage(&export, "receive_channel_open_confirmation");

    let ir = protocol_ir(&export, "ssh_channel_session");
    assert_surface(ir, "ssh", "channel", "channel", "identity-directory-access");
    assert_eq!(
        ir.semantics_category.as_deref(),
        Some("interactive-channel-path")
    );
    assert_json_replay(&export);
}

#[test]
fn ssh_auth_denied_runtime_path_materializes_auth_failure_ir() {
    let export = run_tcp_path(
        "ssh_auth_denied_path.gewy",
        0x55ca,
        22,
        "ssh",
        &[
            (
                PacketDir::Ingress,
                &[(0, 0x53), (1, 0x53), (2, 0x48), (3, 0x2d)][..],
            ),
            (
                PacketDir::Egress,
                &[(0, 0x53), (1, 0x53), (2, 0x48), (3, 0x2d)][..],
            ),
            (PacketDir::Egress, &[(5, 0x14)][..]),
            (PacketDir::Egress, &[(5, 0x32)][..]),
            (PacketDir::Ingress, &[(5, 0x33)][..]),
        ],
    );

    assert_operation(&export, "ssh_auth_denied");
    assert_stage(&export, "send_auth_request");
    assert_stage(&export, "receive_auth_denied");

    let ir = protocol_ir(&export, "ssh_auth_denied");
    assert_surface(
        ir,
        "ssh",
        "auth-denied",
        "auth",
        "identity-directory-access",
    );
    assert_eq!(ir.semantics_category.as_deref(), Some("failure-path"));
    assert_json_replay(&export);
}

#[test]
fn ssh_auth_denied_runtime_ir_does_not_materialize_for_auth_success() {
    let export = run_tcp_path(
        "ssh_auth_denied_path.gewy",
        0x55cb,
        22,
        "ssh",
        &[
            (
                PacketDir::Ingress,
                &[(0, 0x53), (1, 0x53), (2, 0x48), (3, 0x2d)][..],
            ),
            (
                PacketDir::Egress,
                &[(0, 0x53), (1, 0x53), (2, 0x48), (3, 0x2d)][..],
            ),
            (PacketDir::Egress, &[(5, 0x14)][..]),
            (PacketDir::Egress, &[(5, 0x32)][..]),
            (PacketDir::Ingress, &[(5, 0x34)][..]),
        ],
    );

    assert_operation(&export, "ssh_auth_denied");
    assert_stage(&export, "send_auth_request");
    assert_no_stage(&export, "receive_auth_denied");
    assert_no_protocol_ir(&export, "ssh_auth_denied");
    assert_json_replay(&export);
}

#[test]
fn smb_tree_runtime_path_materializes_share_ir() {
    let export = run_tcp_path(
        "smb_tree_path.gewy",
        0x5b44,
        445,
        "smbclient",
        &[
            (PacketDir::Egress, &[(4, 0xfe), (16, 0x03)][..]),
            (PacketDir::Ingress, &[(4, 0xfe)][..]),
        ],
    );

    assert_operation(&export, "smb_tree");
    assert_stage(&export, "send_tree_connect");
    assert_stage(&export, "receive_tree_connect");

    let ir = protocol_ir(&export, "smb_tree");
    assert_surface(ir, "smb", "tree", "share", "identity-directory-access");
    assert_eq!(
        ir.semantics_category.as_deref(),
        Some("file-share-tree-path")
    );
    assert_json_replay(&export);
}

#[test]
fn rdp_denied_runtime_path_materializes_denied_ir() {
    let export = run_tcp_path(
        "rdp_denied_path.gewy",
        0x8d09,
        3389,
        "xfreerdp",
        &[
            (PacketDir::Egress, &[(0, 0x03), (5, 0xe0)][..]),
            (PacketDir::Ingress, &[(0, 0x03), (5, 0x80)][..]),
        ],
    );

    assert_operation(&export, "rdp_denied");
    assert_stage(&export, "send_x224_connect");
    assert_stage(&export, "receive_x224_disconnect");

    let ir = protocol_ir(&export, "rdp_denied");
    assert_surface(ir, "rdp", "denied", "denied", "identity-directory-access");
    assert_eq!(
        ir.semantics_category.as_deref(),
        Some("remote-desktop-denied-path")
    );
    assert_json_replay(&export);
}

#[test]
fn rdp_denied_runtime_ir_does_not_materialize_without_denial_frame() {
    let export = run_tcp_path(
        "rdp_denied_path.gewy",
        0x8d0a,
        3389,
        "xfreerdp",
        &[
            (PacketDir::Egress, &[(0, 0x03), (5, 0xe0)][..]),
            (PacketDir::Ingress, &[(0, 0x03), (5, 0xd0)][..]),
        ],
    );

    assert_operation(&export, "rdp_denied");
    assert_stage(&export, "send_x224_connect");
    assert_no_stage(&export, "receive_x224_disconnect");
    assert_no_stage(&export, "receive_negotiation_failure");
    assert_no_protocol_ir(&export, "rdp_denied");
    assert_json_replay(&export);
}

#[test]
fn radius_denied_runtime_path_materializes_denied_ir() {
    let export = run_udp_path(
        "radius_denied_path.gewy",
        0x8ad1,
        "radiusd",
        &[(PacketDir::Egress, 0x01), (PacketDir::Ingress, 0x03)],
    );

    assert_operation(&export, "radius_denied");
    assert_stage(&export, "send_access_request");
    assert_stage(&export, "receive_access_reject");

    let ir = protocol_ir(&export, "radius_denied");
    assert_surface(
        ir,
        "radius",
        "denied",
        "denied",
        "identity-directory-access",
    );
    assert_eq!(ir.semantics_category.as_deref(), Some("failure-path"));
    assert_json_replay(&export);
}

#[test]
fn radius_denied_runtime_ir_does_not_materialize_for_access_accept() {
    let export = run_udp_path(
        "radius_denied_path.gewy",
        0x8ad2,
        "radiusd",
        &[(PacketDir::Egress, 0x01), (PacketDir::Ingress, 0x02)],
    );

    assert_operation(&export, "radius_denied");
    assert_stage(&export, "send_access_request");
    assert_no_stage(&export, "receive_access_reject");
    assert_no_protocol_ir(&export, "radius_denied");
    assert_json_replay(&export);
}

#[test]
fn ldap_bind_denied_runtime_path_materializes_bind_failure_ir() {
    let export = run_tcp_path(
        "ldap_bind_denied_path.gewy",
        0x1daa,
        389,
        "ldapsearch",
        &[
            (PacketDir::Egress, &[(5, 0x60)][..]),
            (PacketDir::Ingress, &[(5, 0x61), (9, 0x31)][..]),
        ],
    );

    assert_operation(&export, "ldap_bind_denied");
    assert_stage(&export, "send_bind");
    assert_stage(&export, "receive_bind_denied");

    let ir = protocol_ir(&export, "ldap_bind_denied");
    assert_surface(
        ir,
        "ldap",
        "bind-denied",
        "bind",
        "identity-directory-access",
    );
    assert_eq!(ir.semantics_category.as_deref(), Some("failure-path"));
    assert_json_replay(&export);
}

#[test]
fn ldap_bind_denied_runtime_ir_does_not_materialize_for_bind_success() {
    let export = run_tcp_path(
        "ldap_bind_denied_path.gewy",
        0x1dab,
        389,
        "ldapsearch",
        &[
            (PacketDir::Egress, &[(5, 0x60)][..]),
            (PacketDir::Ingress, &[(5, 0x61), (9, 0x00)][..]),
        ],
    );

    assert_operation(&export, "ldap_bind_denied");
    assert_stage(&export, "send_bind");
    assert_no_stage(&export, "receive_bind_denied");
    assert_no_protocol_ir(&export, "ldap_bind_denied");
    assert_json_replay(&export);
}

#[test]
fn ldap_modify_denied_runtime_path_materializes_denied_ir() {
    let export = run_tcp_path(
        "ldap_modify_denied_path.gewy",
        0x1dac,
        389,
        "ldapmodify",
        &[
            (PacketDir::Egress, &[(5, 0x66)][..]),
            (PacketDir::Ingress, &[(5, 0x67), (9, 0x32)][..]),
        ],
    );

    assert_operation(&export, "ldap_modify_denied");
    assert_stage(&export, "send_modify");
    assert_stage(&export, "receive_modify_denied");

    let ir = protocol_ir(&export, "ldap_modify_denied");
    assert_surface(
        ir,
        "ldap",
        "denied",
        "write-sync",
        "identity-directory-access",
    );
    assert_eq!(ir.semantics_category.as_deref(), Some("failure-path"));
    assert_json_replay(&export);
}

#[test]
fn ldap_modify_denied_runtime_ir_does_not_materialize_for_modify_success() {
    let export = run_tcp_path(
        "ldap_modify_denied_path.gewy",
        0x1dad,
        389,
        "ldapmodify",
        &[
            (PacketDir::Egress, &[(5, 0x66)][..]),
            (PacketDir::Ingress, &[(5, 0x67), (9, 0x00)][..]),
        ],
    );

    assert_operation(&export, "ldap_modify_denied");
    assert_stage(&export, "send_modify");
    assert_no_stage(&export, "receive_modify_denied");
    assert_no_protocol_ir(&export, "ldap_modify_denied");
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
    session.ingest(sock_lineage_fact(1, cookie, 8400, process_name));
    session.ingest(tcp_state_fact_with_ports(
        2,
        cookie,
        1,
        2,
        49000,
        server_port,
    ));
    session.ingest(tcp_state_fact_with_ports(
        3,
        cookie,
        2,
        3,
        49000,
        server_port,
    ));
    session.ingest(route_fact(4, cookie, 8));

    for (index, (dir, payload_bytes)) in packets.iter().enumerate() {
        session.ingest(packet_fact_with_dir_and_payload_bytes(
            5 + index as u64,
            cookie,
            0x18,
            *dir,
            Some(49000),
            Some(server_port),
            payload_bytes,
        ));
    }

    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(160));
    session.export_bundle()
}

fn run_udp_path(
    fixture: &str,
    cookie: u64,
    process_name: &str,
    packets: &[(PacketDir, u8)],
) -> ExportBundle {
    let binding = compile_file(&dsl_fixture_path(fixture)).unwrap();
    let config = SessionConfig::for_template(binding.template).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, cookie, 8500, process_name));
    session.ingest(route_fact(2, cookie, 8));

    for (index, (dir, byte0)) in packets.iter().enumerate() {
        session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
            3 + index as u64,
            cookie,
            96,
            *dir,
            Some(53001),
            Some(1812),
            Some(*byte0),
            None,
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
