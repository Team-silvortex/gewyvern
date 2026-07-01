mod support;

use gewyvern::dsl::compile_file;
use gewyvern::export::{ExportBundle, ProtocolIr};
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
fn elasticsearch_search_runtime_path_materializes_search_ir() {
    let export = run_tcp_path(
        "elasticsearch_search_path.gewy",
        0x3551,
        80,
        "es-client",
        &[
            (PacketDir::Egress, http_get()),
            (PacketDir::Ingress, http_response()),
        ],
    );

    assert_operation(&export, "elasticsearch_search");
    assert_stage(&export, "send_search_get");
    assert_stage(&export, "receive_response");

    let ir = protocol_ir(&export, "elasticsearch_search");
    assert_surface(ir, "elasticsearch", "search", "query");
    assert_eq!(
        ir.semantics_category.as_deref(),
        Some("elasticsearch-search-path")
    );
    assert_json_replay(&export);
}

#[test]
fn elasticsearch_bulk_runtime_path_materializes_bulk_ir() {
    let export = run_tcp_path(
        "elasticsearch_bulk_path.gewy",
        0x3552,
        80,
        "es-client",
        &[
            (PacketDir::Egress, http_post()),
            (PacketDir::Ingress, http_response()),
        ],
    );

    assert_operation(&export, "elasticsearch_bulk");
    assert_stage(&export, "send_bulk");

    let ir = protocol_ir(&export, "elasticsearch_bulk");
    assert_surface(ir, "elasticsearch", "bulk", "mutation");
    assert_json_replay(&export);
}

#[test]
fn etcd_range_runtime_path_materializes_kv_ir() {
    let export = run_tcp_path(
        "etcd_range_path.gewy",
        0x3731,
        443,
        "etcdctl",
        &[
            (PacketDir::Egress, &[(0, 0x16)][..]),
            (PacketDir::Ingress, &[(0, 0x17)][..]),
        ],
    );

    assert_operation(&export, "etcd_range");
    assert_stage(&export, "send_range");
    assert_stage(&export, "receive_range");

    let ir = protocol_ir(&export, "etcd_range");
    assert_surface(ir, "etcd", "range", "kv");
    assert_eq!(ir.semantics_category.as_deref(), Some("etcd-range-path"));
    assert_json_replay(&export);
}

#[test]
fn etcd_watch_runtime_path_materializes_watch_ir() {
    let export = run_tcp_path(
        "etcd_watch_path.gewy",
        0x3732,
        443,
        "etcdctl",
        &[
            (PacketDir::Egress, &[(0, 0x16)][..]),
            (PacketDir::Ingress, &[(0, 0x17)][..]),
        ],
    );

    assert_operation(&export, "etcd_watch");
    assert_stage(&export, "open_watch");
    assert_stage(&export, "watch_event");

    let ir = protocol_ir(&export, "etcd_watch");
    assert_surface(ir, "etcd", "watch", "stream-lifecycle");
    assert_json_replay(&export);
}

#[test]
fn consul_service_runtime_path_materializes_discovery_ir() {
    let export = run_tcp_path(
        "consul_service_path.gewy",
        0x6331,
        80,
        "consul",
        &[
            (PacketDir::Egress, http_get()),
            (PacketDir::Ingress, http_response()),
        ],
    );

    assert_operation(&export, "consul_service");
    assert_stage(&export, "send_service_query");
    assert_stage(&export, "receive_service");

    let ir = protocol_ir(&export, "consul_service");
    assert_surface(ir, "consul", "service", "discovery-health");
    assert_json_replay(&export);
}

#[test]
fn consul_kv_runtime_path_materializes_state_ir() {
    let export = run_tcp_path(
        "consul_kv_path.gewy",
        0x6332,
        80,
        "consul",
        &[
            (PacketDir::Egress, http_get()),
            (PacketDir::Ingress, http_response()),
        ],
    );

    assert_operation(&export, "consul_kv");
    assert_stage(&export, "send_kv_get");
    assert_stage(&export, "receive_kv");

    let ir = protocol_ir(&export, "consul_kv");
    assert_surface(ir, "consul", "kv", "state-session");
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
    session.ingest(sock_lineage_fact(1, cookie, 8100, process_name));
    session.ingest(tcp_state_fact_with_ports(
        2,
        cookie,
        1,
        2,
        46000,
        server_port,
    ));
    session.ingest(tcp_state_fact_with_ports(
        3,
        cookie,
        2,
        3,
        46000,
        server_port,
    ));
    session.ingest(route_fact(4, cookie, 6));

    for (index, (dir, payload_bytes)) in packets.iter().enumerate() {
        session.ingest(packet_fact_with_dir_and_payload_bytes(
            5 + index as u64,
            cookie,
            0x18,
            *dir,
            Some(46000),
            Some(server_port),
            payload_bytes,
        ));
    }

    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(100));
    session.export_bundle()
}

fn http_get() -> &'static [(u16, u8)] {
    &[(0, 0x47), (1, 0x45), (2, 0x54), (3, 0x20)]
}

fn http_post() -> &'static [(u16, u8)] {
    &[(0, 0x50), (1, 0x4f), (2, 0x53), (3, 0x54)]
}

fn http_response() -> &'static [(u16, u8)] {
    &[(0, 0x48), (1, 0x54), (2, 0x54), (3, 0x50)]
}

fn assert_operation(export: &ExportBundle, operation: &str) {
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom(operation.into())
    );
}

fn assert_surface(ir: &ProtocolIr, protocol: &str, entry: &str, shelf: &str) {
    assert_eq!(ir.protocol, protocol);
    assert_eq!(ir.entry, entry);
    assert_eq!(ir.shelf_key.as_deref(), Some(shelf));
    assert_eq!(ir.cluster_key.as_deref(), Some("database-query-session"));
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
