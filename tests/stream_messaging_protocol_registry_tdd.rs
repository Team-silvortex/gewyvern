mod support;

use gewyvern::dsl::compile_file;
use gewyvern::export::ExportBundle;
use gewyvern::flow::ProgramOperation;
use gewyvern::ledger::PacketDir;
use gewyvern::protocol_profiles::{
    protocol_default_entry, protocol_dsl_path, protocol_entries, protocol_surface,
};
use gewyvern::runtime::{RuntimeSession, SessionConfig};
use std::time::SystemTime;
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
fn stream_messaging_registry_entries_resolve_to_packaged_paths() {
    assert_eq!(
        protocol_dsl_path("kafka", Some("metadata")),
        Some(protocol_fixture_path("kafka/metadata").to_string())
    );
    assert_eq!(
        protocol_dsl_path("kafka", Some("broker-capabilities")),
        Some(protocol_fixture_path("kafka/api-versions").to_string())
    );
    assert_eq!(
        protocol_dsl_path("kafka-api-versions", None),
        Some(protocol_fixture_path("kafka/api-versions").to_string())
    );
    assert_eq!(
        protocol_dsl_path("kafka", Some("topic-write")),
        Some(protocol_fixture_path("kafka/produce").to_string())
    );
    assert_eq!(
        protocol_dsl_path("kafka", Some("consume")),
        Some(protocol_fixture_path("kafka/fetch").to_string())
    );
    assert_eq!(
        protocol_dsl_path("nats", Some("nats-session")),
        Some(protocol_fixture_path("nats/connect").to_string())
    );
    assert_eq!(
        protocol_dsl_path("nats", Some("subject-write")),
        Some(protocol_fixture_path("nats/pub").to_string())
    );
    assert_eq!(
        protocol_dsl_path("nats", Some("subject-read")),
        Some(protocol_fixture_path("nats/sub").to_string())
    );
    assert_eq!(
        protocol_dsl_path("nats", Some("nats-server-error")),
        Some(protocol_fixture_path("nats/error").to_string())
    );
}

#[test]
fn stream_messaging_defaults_shelves_and_semantics_are_stable() {
    assert_eq!(
        protocol_default_entry("kafka"),
        Some("metadata".to_string())
    );
    assert_eq!(protocol_default_entry("nats"), Some("connect".to_string()));

    let kafka_entries = protocol_entries("kafka").expect("kafka entries should resolve");
    assert!(kafka_entries.contains(&"metadata".to_string()));
    assert!(kafka_entries.contains(&"api-versions".to_string()));
    assert!(kafka_entries.contains(&"produce".to_string()));
    assert!(kafka_entries.contains(&"fetch".to_string()));

    let nats_entries = protocol_entries("nats").expect("nats entries should resolve");
    assert!(nats_entries.contains(&"connect".to_string()));
    assert!(nats_entries.contains(&"pub".to_string()));
    assert!(nats_entries.contains(&"sub".to_string()));
    assert!(nats_entries.contains(&"error".to_string()));

    let kafka = protocol_surface("kafka", "produce").expect("kafka produce surface should exist");
    assert_eq!(kafka.shelf.expect("kafka shelf should exist").key, "stream");
    assert_eq!(
        kafka
            .entry_semantics
            .expect("kafka semantics should exist")
            .category,
        "stream-produce-path"
    );

    let kafka_capabilities =
        protocol_surface("kafka", "api-versions").expect("kafka api versions surface should exist");
    assert_eq!(
        kafka_capabilities
            .shelf
            .expect("kafka capability shelf should exist")
            .key,
        "metadata"
    );
    assert_eq!(
        kafka_capabilities
            .entry_semantics
            .expect("kafka capability semantics should exist")
            .category,
        "broker-capability-path"
    );

    let nats = protocol_surface("nats", "sub").expect("nats sub surface should exist");
    assert_eq!(nats.shelf.expect("nats shelf should exist").key, "pubsub");
    assert_eq!(
        nats.entry_semantics
            .expect("nats semantics should exist")
            .category,
        "message-subscribe-path"
    );

    let nats_error = protocol_surface("nats", "error").expect("nats error surface should exist");
    assert_eq!(
        nats_error.shelf.expect("nats error shelf should exist").key,
        "error"
    );
    assert_eq!(
        nats_error
            .entry_semantics
            .expect("nats error semantics should exist")
            .category,
        "failure-path"
    );
}

#[test]
fn stream_messaging_dsl_files_compile_into_expected_operations() {
    let cases = [
        (
            dsl_fixture_path("kafka_metadata_path.gewy"),
            "kafka_metadata_path",
            ProgramOperation::Custom("kafka_metadata".into()),
        ),
        (
            dsl_fixture_path("kafka_api_versions_path.gewy"),
            "kafka_api_versions_path",
            ProgramOperation::Custom("kafka_api_versions".into()),
        ),
        (
            dsl_fixture_path("kafka_produce_path.gewy"),
            "kafka_produce_path",
            ProgramOperation::Custom("kafka_produce".into()),
        ),
        (
            dsl_fixture_path("kafka_fetch_path.gewy"),
            "kafka_fetch_path",
            ProgramOperation::Custom("kafka_fetch".into()),
        ),
        (
            dsl_fixture_path("nats_connect_path.gewy"),
            "nats_connect_path",
            ProgramOperation::Custom("nats_connect".into()),
        ),
        (
            dsl_fixture_path("nats_pub_path.gewy"),
            "nats_pub_path",
            ProgramOperation::Custom("nats_pub".into()),
        ),
        (
            dsl_fixture_path("nats_sub_path.gewy"),
            "nats_sub_path",
            ProgramOperation::Custom("nats_sub".into()),
        ),
        (
            dsl_fixture_path("nats_error_path.gewy"),
            "nats_error_path",
            ProgramOperation::Custom("nats_error".into()),
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
fn kafka_api_versions_runtime_path_materializes_capability_stages() {
    let export = run_stream_path(
        &dsl_fixture_path("kafka_api_versions_path.gewy"),
        9092,
        &[(5, 0x12)],
        &[(0, 0x00)],
    );
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("kafka_api_versions".into())
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("send_api_versions_request"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_api_versions_response"))
    );

    let protocol_ir = export
        .protocol_ir
        .iter()
        .find(|item| item.operation == "kafka_api_versions")
        .expect("kafka api versions should materialize protocol IR");
    assert_eq!(protocol_ir.protocol, "kafka");
    assert_eq!(protocol_ir.entry, "api-versions");
    assert_eq!(protocol_ir.shelf_key.as_deref(), Some("metadata"));
    assert_eq!(
        protocol_ir.semantics_category.as_deref(),
        Some("broker-capability-path")
    );
}

#[test]
fn kafka_produce_runtime_path_materializes_broker_stages() {
    let export = run_stream_path(
        &dsl_fixture_path("kafka_produce_path.gewy"),
        9092,
        &[(5, 0x00)],
        &[(0, 0x00)],
    );
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("kafka_produce".into())
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("send_produce_request"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_produce_response"))
    );

    let protocol_ir = export
        .protocol_ir
        .iter()
        .find(|item| item.operation == "kafka_produce")
        .expect("kafka produce should materialize protocol IR");
    assert_eq!(protocol_ir.protocol, "kafka");
    assert_eq!(protocol_ir.entry, "produce");
    assert_eq!(protocol_ir.shelf_key.as_deref(), Some("stream"));
    assert_eq!(
        protocol_ir.semantics_category.as_deref(),
        Some("stream-produce-path")
    );

    let replayed = ExportBundle::from_json(&export.to_json()).expect("export json should replay");
    assert_eq!(replayed.protocol_ir, export.protocol_ir);
}

#[test]
fn nats_sub_runtime_path_materializes_pubsub_stages() {
    let export = run_stream_path(
        &dsl_fixture_path("nats_sub_path.gewy"),
        4222,
        &[(0, 0x53), (1, 0x55), (2, 0x42), (3, 0x20)],
        &[(0, 0x4d), (1, 0x53), (2, 0x47), (3, 0x20)],
    );
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("nats_sub".into())
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("send_subscribe"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_message"))
    );

    let protocol_ir = export
        .protocol_ir
        .iter()
        .find(|item| item.operation == "nats_sub")
        .expect("nats sub should materialize protocol IR");
    assert_eq!(protocol_ir.protocol, "nats");
    assert_eq!(protocol_ir.entry, "sub");
    assert_eq!(protocol_ir.shelf_key.as_deref(), Some("pubsub"));
    assert_eq!(
        protocol_ir.semantics_category.as_deref(),
        Some("message-subscribe-path")
    );
}

#[test]
fn nats_error_runtime_path_materializes_server_error_stage() {
    let export = run_stream_path(
        &dsl_fixture_path("nats_error_path.gewy"),
        4222,
        &[],
        &[(0, 0x2d), (1, 0x45), (2, 0x52), (3, 0x52)],
    );
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("nats_error".into())
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_error"))
    );

    let protocol_ir = export
        .protocol_ir
        .iter()
        .find(|item| item.operation == "nats_error")
        .expect("nats error should materialize protocol IR");
    assert_eq!(protocol_ir.protocol, "nats");
    assert_eq!(protocol_ir.entry, "error");
    assert_eq!(protocol_ir.shelf_key.as_deref(), Some("error"));
    assert_eq!(
        protocol_ir.semantics_category.as_deref(),
        Some("failure-path")
    );
}

fn run_stream_path(
    path: &str,
    port: u16,
    send_payload: &[(u16, u8)],
    receive_payload: &[(u16, u8)],
) -> gewyvern::export::ExportBundle {
    let binding = compile_file(&path).unwrap();
    let config = SessionConfig::for_template(binding.template).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    let cookie = 0x5151;
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
    session.freeze(SystemTime::UNIX_EPOCH + std::time::Duration::from_millis(40));
    session.export_bundle()
}
