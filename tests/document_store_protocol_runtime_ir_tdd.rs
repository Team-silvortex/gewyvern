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
fn mongodb_command_runtime_path_materializes_command_ir() {
    let export = run_protocol_path(
        "mongodb_command_path.gewy",
        0x6d6f,
        27017,
        "mongosh",
        &[(
            PacketDir::Egress,
            &[(12, 0xdd), (13, 0x07), (14, 0x00), (15, 0x00)][..],
        )],
    );

    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("mongodb_command".into())
    );
    assert_stage(&export, "send_op_msg");

    let ir = protocol_ir(&export, "mongodb_command");
    assert_protocol_surface(ir, "mongodb", "command", "command");
    assert_eq!(
        ir.semantics_category.as_deref(),
        Some("mongodb-command-path")
    );
    assert_json_replay(&export);
}

#[test]
fn mongodb_query_failure_runtime_path_materializes_hyphenated_entry_ir() {
    let export = run_protocol_path(
        "mongodb_query_failure_path.gewy",
        0x6d71,
        27017,
        "mongosh",
        &[(
            PacketDir::Ingress,
            &[(12, 0x01), (13, 0x00), (14, 0x00), (15, 0x00), (16, 0x02)][..],
        )],
    );

    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("mongodb_query_failure".into())
    );
    assert_stage(&export, "receive_query_failure");

    let ir = protocol_ir(&export, "mongodb_query_failure");
    assert_protocol_surface(ir, "mongodb", "query-failure", "legacy-query");
    assert_eq!(ir.semantics_category.as_deref(), Some("failure-path"));
    assert_eq!(ir.typical_signal.as_deref(), Some("OP_REPLY QueryFailure"));
    assert_json_replay(&export);
}

#[test]
fn cassandra_query_runtime_path_materializes_query_ir() {
    let export = run_protocol_path(
        "cassandra_query_path.gewy",
        0xca55,
        9042,
        "cqlsh",
        &[(PacketDir::Egress, &[(4, 0x07)][..])],
    );

    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("cassandra_query".into())
    );
    assert_stage(&export, "send_query");

    let ir = protocol_ir(&export, "cassandra_query");
    assert_protocol_surface(ir, "cassandra", "query", "query-result");
    assert_eq!(
        ir.semantics_category.as_deref(),
        Some("cassandra-query-path")
    );
    assert_json_replay(&export);
}

#[test]
fn cassandra_error_runtime_path_materializes_failure_ir() {
    let export = run_protocol_path(
        "cassandra_error_path.gewy",
        0xca56,
        9042,
        "cqlsh",
        &[(PacketDir::Ingress, &[(4, 0x00)][..])],
    );

    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("cassandra_error".into())
    );
    assert_stage(&export, "receive_error");

    let ir = protocol_ir(&export, "cassandra_error");
    assert_protocol_surface(ir, "cassandra", "error", "error");
    assert_eq!(ir.semantics_category.as_deref(), Some("failure-path"));
    assert_eq!(ir.typical_signal.as_deref(), Some("ERROR opcode 0x00"));
    assert_json_replay(&export);
}

#[test]
fn mssql_query_runtime_path_materializes_sql_batch_ir() {
    let export = run_protocol_path(
        "mssql_query_path.gewy",
        0x5a31,
        1433,
        "sqlcmd",
        &[(PacketDir::Egress, &[(0, 0x01)][..])],
    );

    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("mssql_query".into())
    );
    assert_stage(&export, "send_sql_batch");

    let ir = protocol_ir(&export, "mssql_query");
    assert_protocol_surface(ir, "mssql", "query", "query-response");
    assert_eq!(ir.semantics_category.as_deref(), Some("mssql-query-path"));
    assert_json_replay(&export);
}

#[test]
fn mssql_done_runtime_path_materializes_completion_token_ir() {
    let export = run_protocol_path(
        "mssql_done_path.gewy",
        0x5a32,
        1433,
        "sqlcmd",
        &[(PacketDir::Ingress, &[(0, 0x04), (8, 0xfd)][..])],
    );

    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("mssql_done".into())
    );
    assert_stage(&export, "receive_done");

    let ir = protocol_ir(&export, "mssql_done");
    assert_protocol_surface(ir, "mssql", "done", "token");
    assert_eq!(ir.semantics_category.as_deref(), Some("mssql-done-path"));
    assert_eq!(
        ir.typical_signal.as_deref(),
        Some("DONE/DONEPROC/DONEINPROC token 0xfd/0xfe/0xff")
    );
    assert_json_replay(&export);
}

#[test]
fn mssql_error_runtime_path_materializes_error_token_ir() {
    let export = run_protocol_path(
        "mssql_error_path.gewy",
        0x5a33,
        1433,
        "sqlcmd",
        &[(PacketDir::Ingress, &[(0, 0x04), (8, 0xaa)][..])],
    );

    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("mssql_error".into())
    );
    assert_stage(&export, "receive_error_token");

    let ir = protocol_ir(&export, "mssql_error");
    assert_protocol_surface(ir, "mssql", "error", "error");
    assert_eq!(ir.semantics_category.as_deref(), Some("failure-path"));
    assert_eq!(ir.typical_signal.as_deref(), Some("ERROR token 0xaa"));
    assert_json_replay(&export);
}

fn run_protocol_path(
    fixture: &str,
    cookie: u64,
    server_port: u16,
    process_name: &str,
    packets: &[(PacketDir, &[(u16, u8)])],
) -> ExportBundle {
    let binding = compile_file(&dsl_fixture_path(fixture)).unwrap();
    let config = SessionConfig::for_template(binding.template).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, cookie, 7900, process_name));
    session.ingest(tcp_state_fact_with_ports(
        2,
        cookie,
        1,
        2,
        44000,
        server_port,
    ));
    session.ingest(tcp_state_fact_with_ports(
        3,
        cookie,
        2,
        3,
        44000,
        server_port,
    ));
    session.ingest(route_fact(4, cookie, 6));

    for (index, (dir, payload_bytes)) in packets.iter().enumerate() {
        session.ingest(packet_fact_with_dir_and_payload_bytes(
            5 + index as u64,
            cookie,
            0,
            *dir,
            Some(44000),
            Some(server_port),
            payload_bytes,
        ));
    }

    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(100));
    session.export_bundle()
}

fn assert_protocol_surface(ir: &ProtocolIr, protocol: &str, entry: &str, shelf: &str) {
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
