mod support;

use gewyvern::dsl::compile_file;
use gewyvern::export::ExportBundle;
use gewyvern::flow::ProgramOperation;
use gewyvern::ledger::PacketDir;
use gewyvern::runtime::{RuntimeSession, SessionConfig};
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
fn memcached_miss_runtime_path_materializes_not_found_result() {
    let export = run_memcached_path(
        "memcached_miss_path.gewy",
        &[(0, 0x81), (1, 0x00), (6, 0x00), (7, 0x01)],
    );

    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("memcached_miss".into())
    );
    assert_stage(&export, "connect");
    assert_stage(&export, "establish");
    assert_stage(&export, "receive_not_found");

    let ir = protocol_ir(&export, "memcached_miss");
    assert_eq!(ir.protocol, "memcached");
    assert_eq!(ir.entry, "miss");
    assert_eq!(ir.cluster_key.as_deref(), Some("cache-queue-stream"));
    assert_eq!(ir.shelf_key.as_deref(), Some("get"));
    assert_eq!(ir.semantics_category.as_deref(), Some("cache-miss-path"));
    assert_eq!(ir.typical_signal.as_deref(), Some("NOT_FOUND"));
}

#[test]
fn memcached_not_stored_runtime_path_materializes_write_miss_result() {
    let export = run_memcached_path(
        "memcached_not_stored_path.gewy",
        &[(0, 0x81), (1, 0x01), (6, 0x00), (7, 0x05)],
    );

    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("memcached_not_stored".into())
    );
    assert_stage(&export, "connect");
    assert_stage(&export, "establish");
    assert_stage(&export, "receive_not_stored");

    let ir = protocol_ir(&export, "memcached_not_stored");
    assert_eq!(ir.protocol, "memcached");
    assert_eq!(ir.entry, "not-stored");
    assert_eq!(ir.cluster_key.as_deref(), Some("cache-queue-stream"));
    assert_eq!(ir.shelf_key.as_deref(), Some("set"));
    assert_eq!(
        ir.semantics_category.as_deref(),
        Some("cache-not-stored-path")
    );
    assert_eq!(ir.typical_signal.as_deref(), Some("NOT_STORED"));

    let replayed = ExportBundle::from_json(&export.to_json()).expect("export json should replay");
    assert_eq!(replayed.protocol_ir, export.protocol_ir);
}

fn run_memcached_path(
    name: &str,
    response_payload: &[(u16, u8)],
) -> gewyvern::export::ExportBundle {
    let binding = compile_file(&dsl_fixture_path(name)).unwrap();
    let config = SessionConfig::for_template(binding.template).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    let cookie = 0x6d63;
    session.ingest(sock_lineage_fact(1, cookie, 5180, "memcached-client"));
    session.ingest(route_fact(2, cookie, 6));
    session.ingest(tcp_state_fact_with_ports(3, cookie, 1, 2, 43135, 11211));
    session.ingest(tcp_state_fact_with_ports(4, cookie, 2, 3, 43135, 11211));
    session.ingest(packet_fact_with_dir_and_payload_bytes(
        5,
        cookie,
        0x18,
        PacketDir::Ingress,
        Some(43135),
        Some(11211),
        response_payload,
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(90));
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
