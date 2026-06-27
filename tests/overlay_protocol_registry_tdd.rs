mod support;

use gewyvern::dsl::compile_file;
use gewyvern::flow::ProgramOperation;
use gewyvern::ledger::PacketDir;
use gewyvern::protocol_profiles::{
    protocol_default_entry, protocol_dsl_path, protocol_entries, protocol_surface,
};
use gewyvern::runtime::{RuntimeSession, SessionConfig};
use std::time::{Duration, SystemTime};
use support::{
    packet_fact_with_dir_and_payload_bytes, route_fact, udp_packet_fact_with_dir_and_ports_and_byte,
};

#[test]
fn overlay_registry_entries_resolve_to_packaged_paths() {
    assert_eq!(
        protocol_dsl_path("vxlan", Some("encap")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/vxlan/encap".to_string())
    );
    assert_eq!(
        protocol_dsl_path("vxlan-tunnel", None),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/vxlan/encap".to_string())
    );
    assert_eq!(
        protocol_dsl_path("vxlan", Some("tenant-overlay")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/vxlan/vni".to_string())
    );
    assert_eq!(
        protocol_dsl_path("geneve", Some("encap")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/geneve/encap".to_string())
    );
    assert_eq!(
        protocol_dsl_path("geneve", Some("geneve-tlv")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/geneve/options".to_string())
    );
    assert_eq!(
        protocol_dsl_path("l2tp", Some("control")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/l2tp/control".to_string())
    );
    assert_eq!(
        protocol_dsl_path("l2tp", Some("l2tp-data")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/l2tp/session".to_string())
    );
    assert_eq!(
        protocol_dsl_path("pptp", Some("control")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/pptp/control".to_string())
    );
    assert_eq!(
        protocol_dsl_path("pptp", Some("pptp-gre")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/pptp/data".to_string())
    );
}

#[test]
fn overlay_default_entries_and_shelves_stay_stable() {
    assert_eq!(protocol_default_entry("vxlan"), Some("encap".to_string()));
    assert_eq!(protocol_default_entry("geneve"), Some("encap".to_string()));
    assert_eq!(protocol_default_entry("l2tp"), Some("control".to_string()));
    assert_eq!(protocol_default_entry("pptp"), Some("control".to_string()));

    let vxlan_entries = protocol_entries("vxlan").expect("vxlan entries should resolve");
    assert!(vxlan_entries.contains(&"encap".to_string()));
    assert!(vxlan_entries.contains(&"vni".to_string()));

    let geneve_entries = protocol_entries("geneve").expect("geneve entries should resolve");
    assert!(geneve_entries.contains(&"encap".to_string()));
    assert!(geneve_entries.contains(&"options".to_string()));

    let vxlan = protocol_surface("vxlan", "vni").expect("vxlan vni surface should exist");
    assert_eq!(
        vxlan.shelf.expect("vxlan shelf should exist").key,
        "overlay"
    );
    assert_eq!(
        vxlan
            .entry_semantics
            .expect("vxlan vni semantics should exist")
            .category,
        "overlay-tenant-path"
    );

    let geneve =
        protocol_surface("geneve", "options").expect("geneve options surface should exist");
    assert_eq!(
        geneve.shelf.expect("geneve shelf should exist").page,
        "docs/book/reference-geneve-overlay-surface.md"
    );
    assert_eq!(
        geneve
            .entry_semantics
            .expect("geneve options semantics should exist")
            .category,
        "overlay-option-path"
    );

    let l2tp = protocol_surface("l2tp", "session").expect("l2tp session surface should exist");
    assert_eq!(l2tp.shelf.expect("l2tp shelf should exist").key, "tunnel");
    assert_eq!(
        l2tp.entry_semantics
            .expect("l2tp session semantics should exist")
            .category,
        "tunnel-session-path"
    );

    let pptp = protocol_surface("pptp", "data").expect("pptp data surface should exist");
    assert_eq!(
        pptp.shelf.expect("pptp shelf should exist").page,
        "docs/book/reference-pptp-tunnel-surface.md"
    );
    assert_eq!(
        pptp.entry_semantics
            .expect("pptp data semantics should exist")
            .category,
        "tunnel-data-path"
    );
}

#[test]
fn overlay_dsl_files_compile_into_expected_operations() {
    let cases = [
        (
            "/Users/Shared/chroot/dev/gewyvern/dsl/vxlan_encap_path.gewy",
            "vxlan_encap_path",
            ProgramOperation::Custom("vxlan_encap".into()),
        ),
        (
            "/Users/Shared/chroot/dev/gewyvern/dsl/vxlan_vni_path.gewy",
            "vxlan_vni_path",
            ProgramOperation::Custom("vxlan_vni".into()),
        ),
        (
            "/Users/Shared/chroot/dev/gewyvern/dsl/geneve_encap_path.gewy",
            "geneve_encap_path",
            ProgramOperation::Custom("geneve_encap".into()),
        ),
        (
            "/Users/Shared/chroot/dev/gewyvern/dsl/geneve_options_path.gewy",
            "geneve_options_path",
            ProgramOperation::Custom("geneve_options".into()),
        ),
        (
            "/Users/Shared/chroot/dev/gewyvern/dsl/l2tp_control_path.gewy",
            "l2tp_control_path",
            ProgramOperation::Custom("l2tp_control".into()),
        ),
        (
            "/Users/Shared/chroot/dev/gewyvern/dsl/l2tp_session_path.gewy",
            "l2tp_session_path",
            ProgramOperation::Custom("l2tp_session".into()),
        ),
        (
            "/Users/Shared/chroot/dev/gewyvern/dsl/pptp_control_path.gewy",
            "pptp_control_path",
            ProgramOperation::Custom("pptp_control".into()),
        ),
        (
            "/Users/Shared/chroot/dev/gewyvern/dsl/pptp_data_path.gewy",
            "pptp_data_path",
            ProgramOperation::Custom("pptp_data".into()),
        ),
    ];

    for (path, template_id, operation) in cases {
        let binding = compile_file(path).unwrap();
        assert_eq!(binding.template.id, template_id);
        assert_eq!(
            binding.template.program_model.as_ref().unwrap().operation,
            operation
        );
    }
}

#[test]
fn vxlan_vni_runtime_path_materializes_overlay_stages() {
    let export = run_overlay_path(
        "/Users/Shared/chroot/dev/gewyvern/dsl/vxlan_vni_path.gewy",
        4789,
        Some(0x08),
    );
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("vxlan_vni".into())
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("send_vni_marked_packet"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_vni_marked_packet"))
    );
}

#[test]
fn geneve_options_runtime_path_materializes_overlay_stages() {
    let export = run_overlay_path(
        "/Users/Shared/chroot/dev/gewyvern/dsl/geneve_options_path.gewy",
        6081,
        Some(0x04),
    );
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("geneve_options".into())
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("send_optioned_packet"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_optioned_packet"))
    );
}

#[test]
fn l2tp_control_runtime_path_materializes_tunnel_stages() {
    let export = run_overlay_path(
        "/Users/Shared/chroot/dev/gewyvern/dsl/l2tp_control_path.gewy",
        1701,
        Some(0xc8),
    );
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("l2tp_control".into())
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("send_control_message"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_control_message"))
    );
}

#[test]
fn pptp_control_runtime_path_materializes_control_stages() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/pptp_control_path.gewy").unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(route_fact(1, 7801, 2));
    session.ingest(pptp_control_packet(2, PacketDir::Egress));
    session.ingest(pptp_control_packet(3, PacketDir::Ingress));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(80));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("pptp_control".into())
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("send_control_message"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_control_message"))
    );
}

fn run_overlay_path(
    dsl_path: &str,
    overlay_port: u16,
    payload_byte0: Option<u8>,
) -> gewyvern::export::ExportBundle {
    let binding = compile_file(dsl_path).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(route_fact(1, 7701, 2));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_byte(
        2,
        7701,
        128,
        PacketDir::Egress,
        Some(41000),
        Some(overlay_port),
        payload_byte0,
    ));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_byte(
        3,
        7701,
        128,
        PacketDir::Ingress,
        Some(41000),
        Some(overlay_port),
        payload_byte0,
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(80));
    session.export_bundle()
}

fn pptp_control_packet(id: u64, dir: PacketDir) -> gewyvern::ledger::FactEnvelope {
    packet_fact_with_dir_and_payload_bytes(
        id,
        7801,
        0x18,
        dir,
        Some(43000),
        Some(1723),
        &[(4, 0x1a), (5, 0x2b), (6, 0x3c), (7, 0x4d)],
    )
}
