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

fn protocol_fixture_path(protocol: &str, entry: &str) -> String {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("protocols")
        .join(protocol)
        .join(entry)
        .join("main.gewy")
        .to_string_lossy()
        .into_owned()
}

#[test]
fn redis_xadd_runtime_path_materializes_stream_ir() {
    let export = run_tcp_path(
        &dsl_fixture_path("redis_xadd_path.gewy"),
        0x7831,
        6379,
        "redis-cli",
        &[
            (
                PacketDir::Egress,
                &[
                    (0, 0x2a),
                    (1, 0x35),
                    (2, 0x0d),
                    (3, 0x0a),
                    (8, 0x58),
                    (9, 0x41),
                    (10, 0x44),
                    (11, 0x44),
                ][..],
            ),
            (PacketDir::Ingress, &[(0, 0x24)][..]),
        ],
    );

    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("redis_xadd".into())
    );
    assert_stage(&export, "send_xadd");
    assert_stage(&export, "receive_bulk_string");

    let ir = protocol_ir(&export, "redis_xadd");
    assert_surface(ir, "redis", "xadd", "stream", "cache-queue-stream");
    assert_json_replay(&export);
}

#[test]
fn redis_zadd_runtime_path_materializes_sorted_set_ir() {
    let export = run_tcp_path(
        &dsl_fixture_path("redis_zadd_path.gewy"),
        0x7a31,
        6379,
        "redis-cli",
        &[
            (
                PacketDir::Egress,
                &[
                    (0, 0x2a),
                    (1, 0x34),
                    (2, 0x0d),
                    (3, 0x0a),
                    (8, 0x5a),
                    (9, 0x41),
                    (10, 0x44),
                    (11, 0x44),
                ][..],
            ),
            (PacketDir::Ingress, &[(0, 0x3a)][..]),
        ],
    );

    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("redis_zadd".into())
    );
    assert_stage(&export, "send_zadd");
    assert_stage(&export, "receive_integer");

    let ir = protocol_ir(&export, "redis_zadd");
    assert_surface(ir, "redis", "zadd", "sorted-set", "cache-queue-stream");
    assert_json_replay(&export);
}

#[test]
fn redis_wrongtype_runtime_path_materializes_failure_ir() {
    let export = run_tcp_path(
        &protocol_fixture_path("redis", "wrongtype"),
        0x7731,
        6379,
        "redis-cli",
        &[
            (
                PacketDir::Egress,
                &[
                    (0, 0x2a),
                    (1, 0x32),
                    (2, 0x0d),
                    (3, 0x0a),
                    (8, 0x47),
                    (9, 0x45),
                    (10, 0x54),
                ][..],
            ),
            (
                PacketDir::Ingress,
                &[(0, 0x2d), (1, 0x57), (2, 0x52), (3, 0x4f), (6, 0x54)][..],
            ),
        ],
    );

    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("redis_wrongtype".into())
    );
    assert_stage(&export, "receive_wrongtype_constraint");

    let ir = protocol_ir(&export, "redis_wrongtype");
    assert_surface(ir, "redis", "wrongtype", "failure", "cache-queue-stream");
    assert_eq!(ir.semantics_category.as_deref(), Some("failure-path"));
    assert_json_replay(&export);
}

#[test]
fn zookeeper_read_runtime_path_materializes_znode_ir() {
    let export = run_tcp_path(
        &dsl_fixture_path("zookeeper_read_path.gewy"),
        0x7a6b,
        2181,
        "zkCli",
        &[
            (PacketDir::Egress, &[(0, 0x00)][..]),
            (PacketDir::Ingress, &[(0, 0x00)][..]),
        ],
    );

    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("zookeeper_read".into())
    );
    assert_stage(&export, "send_read");
    assert_stage(&export, "receive_read");

    let ir = protocol_ir(&export, "zookeeper_read");
    assert_surface(
        ir,
        "zookeeper",
        "read",
        "znode-data",
        "database-query-session",
    );
    assert_eq!(
        ir.semantics_category.as_deref(),
        Some("zookeeper-read-path")
    );
    assert_json_replay(&export);
}

#[test]
fn zookeeper_auth_denied_runtime_path_materializes_failure_ir() {
    let export = run_tcp_path(
        &dsl_fixture_path("zookeeper_auth_denied_path.gewy"),
        0x7a6c,
        2181,
        "zkCli",
        &[
            (PacketDir::Egress, &[(0, 0x00)][..]),
            (PacketDir::Ingress, &[(0, 0x00)][..]),
        ],
    );

    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("zookeeper_auth_denied".into())
    );
    assert_stage(&export, "receive_denial");

    let ir = protocol_ir(&export, "zookeeper_auth_denied");
    assert_surface(
        ir,
        "zookeeper",
        "auth-denied",
        "session-auth",
        "database-query-session",
    );
    assert_eq!(
        ir.semantics_category.as_deref(),
        Some("zookeeper-auth-denied-path")
    );
    assert_json_replay(&export);
}

fn run_tcp_path(
    path: &str,
    cookie: u64,
    server_port: u16,
    process_name: &str,
    packets: &[(PacketDir, &[(u16, u8)])],
) -> ExportBundle {
    let binding = compile_file(path).unwrap();
    let config = SessionConfig::for_template(binding.template).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, cookie, 8000, process_name));
    session.ingest(tcp_state_fact_with_ports(
        2,
        cookie,
        1,
        2,
        45000,
        server_port,
    ));
    session.ingest(tcp_state_fact_with_ports(
        3,
        cookie,
        2,
        3,
        45000,
        server_port,
    ));
    session.ingest(route_fact(4, cookie, 6));

    for (index, (dir, payload_bytes)) in packets.iter().enumerate() {
        session.ingest(packet_fact_with_dir_and_payload_bytes(
            5 + index as u64,
            cookie,
            0x18,
            *dir,
            Some(45000),
            Some(server_port),
            payload_bytes,
        ));
    }

    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(100));
    session.export_bundle()
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
