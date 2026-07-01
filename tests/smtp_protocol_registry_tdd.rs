mod support;

use gewyvern::dsl::compile_file;
use gewyvern::export::ExportBundle;
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
fn smtp_registry_entries_and_aliases_resolve_to_packaged_paths() {
    let cases = [
        (None, "smtp/session"),
        (Some("session"), "smtp/session"),
        (Some("auth"), "smtp/auth"),
        (Some("login"), "smtp/auth"),
        (Some("auth-denied"), "smtp/auth-denied"),
        (Some("login-denied"), "smtp/auth-denied"),
        (Some("mail"), "smtp/mail"),
        (Some("sender"), "smtp/mail"),
        (Some("rcpt"), "smtp/rcpt"),
        (Some("recipient"), "smtp/rcpt"),
        (Some("rcpt-denied"), "smtp/rcpt-denied"),
        (Some("recipient-denied"), "smtp/rcpt-denied"),
        (Some("data"), "smtp/data"),
        (Some("message"), "smtp/data"),
        (Some("data-denied"), "smtp/data-denied"),
        (Some("message-denied"), "smtp/data-denied"),
    ];

    for (entry, path) in cases {
        assert_eq!(
            protocol_dsl_path("smtp", entry),
            Some(protocol_fixture_path(path))
        );
    }
}

#[test]
fn smtp_defaults_shelves_and_semantics_are_stable() {
    assert_eq!(protocol_default_entry("smtp"), Some("session".to_string()));

    let entries = protocol_entries("smtp").expect("smtp entries should resolve");
    for entry in [
        "session",
        "auth",
        "auth-denied",
        "mail",
        "rcpt",
        "rcpt-denied",
        "data",
        "data-denied",
    ] {
        assert!(entries.contains(&entry.to_string()), "missing {entry}");
    }

    let auth = protocol_surface("smtp", "auth").expect("smtp auth surface should exist");
    assert_eq!(auth.shelf.expect("smtp auth shelf").key, "session-auth");
    assert_eq!(
        auth.cluster_hint.expect("smtp cluster").key,
        "mail-delivery-mailbox"
    );

    let rcpt = protocol_surface("smtp", "rcpt-denied").expect("smtp rcpt denied surface");
    assert_eq!(rcpt.shelf.expect("smtp rcpt shelf").key, "envelope");
    assert_eq!(
        rcpt.entry_semantics.expect("smtp rcpt semantics").category,
        "failure-path"
    );

    let data = protocol_surface("smtp", "data-denied").expect("smtp data denied surface");
    assert_eq!(data.shelf.expect("smtp data shelf").key, "data");
    assert_eq!(
        data.entry_semantics
            .expect("smtp data semantics")
            .primary_failure_mode,
        Some("server_denied".to_string())
    );
}

#[test]
fn smtp_dsl_files_compile_into_expected_operations() {
    let cases = [
        (
            "smtp_session_path.gewy",
            "smtp_session_path",
            ProgramOperation::Custom("smtp_session".into()),
        ),
        (
            "smtp_auth_path.gewy",
            "smtp_auth_path",
            ProgramOperation::Custom("smtp_auth".into()),
        ),
        (
            "smtp_auth_denied_path.gewy",
            "smtp_auth_denied_path",
            ProgramOperation::Custom("smtp_auth_denied".into()),
        ),
        (
            "smtp_mail_path.gewy",
            "smtp_mail_path",
            ProgramOperation::Custom("smtp_mail".into()),
        ),
        (
            "smtp_rcpt_path.gewy",
            "smtp_rcpt_path",
            ProgramOperation::Custom("smtp_rcpt".into()),
        ),
        (
            "smtp_rcpt_denied_path.gewy",
            "smtp_rcpt_denied_path",
            ProgramOperation::Custom("smtp_rcpt_denied".into()),
        ),
        (
            "smtp_data_path.gewy",
            "smtp_data_path",
            ProgramOperation::Custom("smtp_data".into()),
        ),
        (
            "smtp_data_denied_path.gewy",
            "smtp_data_denied_path",
            ProgramOperation::Custom("smtp_data_denied".into()),
        ),
    ];

    for (path, template_id, operation) in cases {
        let binding = compile_file(&dsl_fixture_path(path)).unwrap();
        assert_eq!(binding.template.id, template_id);
        assert_eq!(
            binding.template.program_model.as_ref().unwrap().operation,
            operation
        );
    }
}

#[test]
fn smtp_session_runtime_path_materializes_banner_and_ehlo_stages() {
    let export = run_smtp_path(
        &dsl_fixture_path("smtp_session_path.gewy"),
        &[(0, b'E'), (1, b'H'), (2, b'L'), (3, b'O')],
        &[(0, b'2'), (1, b'2'), (2, b'0'), (3, b' ')],
    );

    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("smtp_session".into())
    );
    assert_stage(&export, "receive_banner");
    assert_stage(&export, "send_ehlo");

    let protocol_ir = protocol_ir(&export, "smtp_session");
    assert_eq!(protocol_ir.protocol, "smtp");
    assert_eq!(protocol_ir.entry, "session");
    assert_eq!(protocol_ir.shelf_key.as_deref(), Some("session-auth"));

    let replayed = ExportBundle::from_json(&export.to_json()).expect("export json should replay");
    assert_eq!(replayed.protocol_ir, export.protocol_ir);
}

#[test]
fn smtp_auth_denied_runtime_path_keeps_failure_operation_and_semantics() {
    let export = run_smtp_path(
        &dsl_fixture_path("smtp_auth_denied_path.gewy"),
        &[(0, b'A'), (1, b'U'), (2, b'T'), (3, b'H')],
        &[(0, b'5'), (1, b'3'), (2, b'5'), (3, b' ')],
    );

    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("smtp_auth_denied".into())
    );
    assert_stage(&export, "send_auth_request");
    assert_stage(&export, "receive_auth_denied");

    let protocol_ir = protocol_ir(&export, "smtp_auth_denied");
    assert_eq!(protocol_ir.protocol, "smtp");
    assert_eq!(protocol_ir.entry, "auth-denied");
    assert_eq!(protocol_ir.shelf_key.as_deref(), Some("session-auth"));
    assert_eq!(
        protocol_ir.semantics_category.as_deref(),
        Some("failure-path")
    );
}

fn run_smtp_path(
    path: &str,
    send_payload: &[(u16, u8)],
    receive_payload: &[(u16, u8)],
) -> gewyvern::export::ExportBundle {
    let binding = compile_file(path).unwrap();
    let config = SessionConfig::for_template(binding.template).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    let cookie = 0x5353;
    for fact in [
        route_fact(1, cookie, 2),
        tcp_state_fact_with_ports(2, cookie, 1, 2, 45000, 25),
        tcp_state_fact_with_ports(3, cookie, 2, 3, 45000, 25),
        packet_fact_with_dir_and_payload_bytes(
            4,
            cookie,
            0x18,
            PacketDir::Egress,
            Some(45000),
            Some(25),
            send_payload,
        ),
        packet_fact_with_dir_and_payload_bytes(
            5,
            cookie,
            0x18,
            PacketDir::Ingress,
            Some(45000),
            Some(25),
            receive_payload,
        ),
    ] {
        session.ingest(fact);
    }
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(80));
    session.export_bundle()
}

fn assert_stage(export: &gewyvern::export::ExportBundle, phase: &str) {
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some(phase)),
        "missing stage {phase}"
    );
}

fn protocol_ir<'a>(
    export: &'a gewyvern::export::ExportBundle,
    operation: &str,
) -> &'a gewyvern::export::ProtocolIr {
    export
        .protocol_ir
        .iter()
        .find(|item| item.operation == operation)
        .expect("protocol IR should materialize")
}
