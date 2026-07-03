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
fn postgres_simple_query_runtime_path_keeps_legacy_operation_in_protocol_ir() {
    let export = run_database_path(
        "postgres_simple_query_path.gewy",
        0x7073,
        5432,
        "psql",
        &[
            (PacketDir::Egress, &[(0, 0x51)][..]),
            (PacketDir::Ingress, &[(0, 0x5a)][..]),
        ],
    );

    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("postgres_simple_query".into())
    );
    assert_stage(&export, "send_query");
    assert_stage(&export, "receive_ready");

    let ir = protocol_ir(&export, "postgres_simple_query");
    assert_eq!(ir.protocol, "postgres");
    assert_eq!(ir.entry, "query");
    assert_eq!(ir.shelf_key.as_deref(), Some("query-session"));
    assert_eq!(ir.cluster_key.as_deref(), Some("database-query-session"));
    assert_eq!(
        ir.semantics_category.as_deref(),
        Some("postgres-query-path")
    );

    let replayed = ExportBundle::from_json(&export.to_json()).expect("export json should replay");
    assert_eq!(replayed.protocol_ir, export.protocol_ir);
}

#[test]
fn mysql_simple_query_runtime_path_keeps_legacy_operation_in_protocol_ir() {
    let export = run_database_path(
        "mysql_simple_query_path.gewy",
        0x6d79,
        3306,
        "mysql",
        &[
            (PacketDir::Egress, &[(4, 0x03)][..]),
            (PacketDir::Ingress, &[(4, 0x00)][..]),
        ],
    );

    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("mysql_simple_query".into())
    );
    assert_stage(&export, "send_query");
    assert_stage(&export, "receive_ok");

    let ir = protocol_ir(&export, "mysql_simple_query");
    assert_eq!(ir.protocol, "mysql");
    assert_eq!(ir.entry, "query");
    assert_eq!(ir.shelf_key.as_deref(), Some("query-session"));
    assert_eq!(ir.cluster_key.as_deref(), Some("database-query-session"));
    assert_eq!(ir.semantics_category.as_deref(), Some("mysql-query-path"));

    let replayed = ExportBundle::from_json(&export.to_json()).expect("export json should replay");
    assert_eq!(replayed.protocol_ir, export.protocol_ir);
}

#[test]
fn mysql_auth_denied_runtime_path_materializes_auth_failure_ir() {
    let export = run_database_path(
        "mysql_auth_denied_path.gewy",
        0x6d7a,
        3306,
        "mysql",
        &[
            (PacketDir::Ingress, &[(4, 0x0a)][..]),
            (PacketDir::Egress, &[(3, 0x01)][..]),
            (PacketDir::Ingress, &[(4, 0xff)][..]),
        ],
    );

    assert_operation(&export, "mysql_auth_denied");
    assert_stage(&export, "send_login");
    assert_stage(&export, "receive_auth_denied");

    let ir = protocol_ir(&export, "mysql_auth_denied");
    assert_eq!(ir.protocol, "mysql");
    assert_eq!(ir.entry, "auth-denied");
    assert_eq!(ir.shelf_key.as_deref(), Some("connect-auth"));
    assert_eq!(ir.cluster_key.as_deref(), Some("database-query-session"));
    assert_eq!(ir.semantics_category.as_deref(), Some("failure-path"));
    assert_json_replay(&export);
}

#[test]
fn mysql_auth_denied_runtime_ir_does_not_materialize_for_login_ok() {
    let export = run_database_path(
        "mysql_auth_denied_path.gewy",
        0x6d7b,
        3306,
        "mysql",
        &[
            (PacketDir::Ingress, &[(4, 0x0a)][..]),
            (PacketDir::Egress, &[(3, 0x01)][..]),
            (PacketDir::Ingress, &[(4, 0x00)][..]),
        ],
    );

    assert_operation(&export, "mysql_auth_denied");
    assert_stage(&export, "send_login");
    assert_no_stage(&export, "receive_auth_denied");
    assert_no_protocol_ir(&export, "mysql_auth_denied");
    assert_json_replay(&export);
}

#[test]
fn postgres_auth_denied_runtime_path_materializes_auth_failure_ir() {
    let export = run_database_path(
        "postgres_auth_denied_path.gewy",
        0x7074,
        5432,
        "psql",
        &[
            (PacketDir::Ingress, &[(0, 0x52)][..]),
            (PacketDir::Egress, &[(0, 0x70)][..]),
            (PacketDir::Ingress, &[(0, 0x45)][..]),
        ],
    );

    assert_operation(&export, "postgres_auth_denied");
    assert_stage(&export, "send_password");
    assert_stage(&export, "receive_auth_denied");

    let ir = protocol_ir(&export, "postgres_auth_denied");
    assert_eq!(ir.protocol, "postgres");
    assert_eq!(ir.entry, "auth-denied");
    assert_eq!(ir.shelf_key.as_deref(), Some("connect-auth"));
    assert_eq!(ir.cluster_key.as_deref(), Some("database-query-session"));
    assert_eq!(ir.semantics_category.as_deref(), Some("failure-path"));
    assert_json_replay(&export);
}

#[test]
fn postgres_auth_denied_runtime_ir_does_not_materialize_for_auth_ok() {
    let export = run_database_path(
        "postgres_auth_denied_path.gewy",
        0x7075,
        5432,
        "psql",
        &[
            (PacketDir::Ingress, &[(0, 0x52)][..]),
            (PacketDir::Egress, &[(0, 0x70)][..]),
            (PacketDir::Ingress, &[(0, 0x5a)][..]),
        ],
    );

    assert_operation(&export, "postgres_auth_denied");
    assert_stage(&export, "send_password");
    assert_no_stage(&export, "receive_auth_denied");
    assert_no_protocol_ir(&export, "postgres_auth_denied");
    assert_json_replay(&export);
}

fn run_database_path(
    fixture: &str,
    cookie: u64,
    server_port: u16,
    process_name: &str,
    packets: &[(PacketDir, &[(u16, u8)])],
) -> ExportBundle {
    let binding = compile_file(&dsl_fixture_path(fixture)).unwrap();
    let config = SessionConfig::for_template(binding.template).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, cookie, 7800, process_name));
    session.ingest(tcp_state_fact_with_ports(
        2,
        cookie,
        1,
        2,
        43125,
        server_port,
    ));
    session.ingest(tcp_state_fact_with_ports(
        3,
        cookie,
        2,
        3,
        43125,
        server_port,
    ));
    session.ingest(route_fact(4, cookie, 6));

    for (index, (dir, payload_bytes)) in packets.iter().enumerate() {
        session.ingest(packet_fact_with_dir_and_payload_bytes(
            5 + index as u64,
            cookie,
            0,
            *dir,
            Some(43125),
            Some(server_port),
            payload_bytes,
        ));
    }

    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(90));
    session.export_bundle()
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

fn assert_operation(export: &ExportBundle, operation: &str) {
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom(operation.into())
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

fn protocol_ir<'a>(export: &'a ExportBundle, operation: &str) -> &'a gewyvern::export::ProtocolIr {
    export
        .protocol_ir
        .iter()
        .find(|item| item.operation == operation)
        .expect("protocol IR should materialize")
}
