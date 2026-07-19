mod support;

use gewyvern::dsl::compile_file;
use gewyvern::flow::ProgramOperation;
use gewyvern::ledger::PacketDir;
use gewyvern::protocol_profiles::{
    protocol_default_entry, protocol_dsl_path, protocol_entries, protocol_surface,
};
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

fn protocol_fixture_path(relative: &str) -> String {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("protocols")
        .join(relative)
        .to_string_lossy()
        .into_owned()
}
#[test]
fn remote_access_registry_entries_resolve_to_packaged_paths() {
    assert_eq!(
        protocol_dsl_path("smb", Some("smb2-session")),
        Some(protocol_fixture_path("smb/session").to_string())
    );
    assert_eq!(
        protocol_dsl_path("smb", Some("tree-connect")),
        Some(protocol_fixture_path("smb/tree").to_string())
    );
    assert_eq!(
        protocol_dsl_path("rdp", Some("x224-connect")),
        Some(protocol_fixture_path("rdp/connect").to_string())
    );
    assert_eq!(
        protocol_dsl_path("rdp", Some("rdp-data")),
        Some(protocol_fixture_path("rdp/channel").to_string())
    );
    assert_eq!(
        protocol_dsl_path("rdp", Some("x224-disconnect")),
        Some(protocol_fixture_path("rdp/denied").to_string())
    );
    assert_eq!(
        protocol_dsl_path("rdp", Some("negotiation-failed")),
        Some(protocol_fixture_path("rdp/denied").to_string())
    );
}

#[test]
fn remote_access_defaults_shelves_and_semantics_are_stable() {
    assert_eq!(protocol_default_entry("smb"), Some("negotiate".to_string()));
    assert_eq!(protocol_default_entry("rdp"), Some("connect".to_string()));

    let smb_entries = protocol_entries("smb").expect("smb entries should resolve");
    assert!(smb_entries.contains(&"negotiate".to_string()));
    assert!(smb_entries.contains(&"session".to_string()));
    assert!(smb_entries.contains(&"tree".to_string()));

    let rdp_entries = protocol_entries("rdp").expect("rdp entries should resolve");
    assert!(rdp_entries.contains(&"connect".to_string()));
    assert!(rdp_entries.contains(&"channel".to_string()));
    assert!(rdp_entries.contains(&"denied".to_string()));

    let smb = protocol_surface("smb", "tree").expect("smb tree surface should exist");
    assert_eq!(smb.shelf.expect("smb shelf should exist").key, "share");
    assert_eq!(
        smb.entry_semantics.expect("smb semantics").category,
        "file-share-tree-path"
    );

    let rdp = protocol_surface("rdp", "channel").expect("rdp channel surface should exist");
    assert_eq!(rdp.shelf.expect("rdp shelf should exist").key, "channel");
    assert_eq!(
        rdp.entry_semantics.expect("rdp semantics").category,
        "remote-desktop-channel-path"
    );

    let denied = protocol_surface("rdp", "denied").expect("rdp denied surface should exist");
    assert_eq!(
        denied.shelf.expect("rdp denied shelf should exist").key,
        "denied"
    );
    assert_eq!(
        denied
            .entry_semantics
            .expect("rdp denied semantics")
            .category,
        "remote-desktop-denied-path"
    );
}

#[test]
fn remote_access_dsl_files_compile_into_expected_operations() {
    let cases = [
        (
            dsl_fixture_path("smb_negotiate_path.gewy"),
            "smb_negotiate_path",
            ProgramOperation::Custom("smb_negotiate".into()),
        ),
        (
            dsl_fixture_path("smb_session_path.gewy"),
            "smb_session_path",
            ProgramOperation::Custom("smb_session".into()),
        ),
        (
            dsl_fixture_path("smb_tree_path.gewy"),
            "smb_tree_path",
            ProgramOperation::Custom("smb_tree".into()),
        ),
        (
            dsl_fixture_path("rdp_connect_path.gewy"),
            "rdp_connect_path",
            ProgramOperation::Custom("rdp_connect".into()),
        ),
        (
            dsl_fixture_path("rdp_channel_path.gewy"),
            "rdp_channel_path",
            ProgramOperation::Custom("rdp_channel".into()),
        ),
        (
            dsl_fixture_path("rdp_denied_path.gewy"),
            "rdp_denied_path",
            ProgramOperation::Custom("rdp_denied".into()),
        ),
    ];

    for (path, template_id, operation) in cases {
        let binding = compile_file(&path).unwrap();
        assert_eq!(binding.template.id, template_id);
        assert_eq!(
            binding.template.program_model.as_ref().unwrap().operation,
            operation
        );
    }
}

#[test]
fn smb_tree_runtime_path_materializes_share_stages() {
    let export = run_remote_access_path(
        &dsl_fixture_path("smb_tree_path.gewy"),
        445,
        &[(4, 0xfe), (16, 0x03)],
        &[(4, 0xfe)],
    );
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("smb_tree".into())
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("send_tree_connect"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_tree_connect"))
    );
}

#[test]
fn rdp_channel_runtime_path_materializes_desktop_stages() {
    let export = run_remote_access_path(
        &dsl_fixture_path("rdp_channel_path.gewy"),
        3389,
        &[(0, 0x03), (5, 0xf0)],
        &[(0, 0x03), (5, 0xf0)],
    );
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("rdp_channel".into())
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("send_channel_data"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_channel_data"))
    );
}

#[test]
fn rdp_denied_runtime_path_materializes_failure_stages() {
    let export = run_remote_access_path(
        &dsl_fixture_path("rdp_denied_path.gewy"),
        3389,
        &[(0, 0x03), (5, 0xe0)],
        &[(0, 0x03), (5, 0x80)],
    );
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("rdp_denied".into())
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_x224_disconnect"))
    );
}

fn run_remote_access_path(
    path: &str,
    port: u16,
    send_payload: &[(u16, u8)],
    receive_payload: &[(u16, u8)],
) -> gewyvern::export::ExportBundle {
    let binding = compile_file(path).unwrap();
    let config = SessionConfig::for_template(binding.template).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    let cookie = 0x7171;
    for fact in [
        route_fact(1, cookie, 2),
        tcp_state_fact_with_ports(2, cookie, 1, 2, 45000, port),
        tcp_state_fact_with_ports(3, cookie, 2, 3, 45000, port),
        packet_fact_with_dir_and_payload_bytes(
            4,
            cookie,
            0x18,
            PacketDir::Egress,
            Some(45000),
            Some(port),
            send_payload,
        ),
        packet_fact_with_dir_and_payload_bytes(
            5,
            cookie,
            0x18,
            PacketDir::Ingress,
            Some(45000),
            Some(port),
            receive_payload,
        ),
    ] {
        session.ingest(fact);
    }
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(80));
    session.export_bundle()
}
