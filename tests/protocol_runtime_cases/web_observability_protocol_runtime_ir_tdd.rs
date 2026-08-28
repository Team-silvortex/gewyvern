use crate::support;

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
fn http_connect_denied_runtime_path_materializes_denied_ir_from_legacy_operation() {
    let export = run_tcp_path(
        "http_connect_denied_path.gewy",
        0x4831,
        8080,
        "curl",
        &[
            (PacketDir::Egress, connect_prefix()),
            (PacketDir::Ingress, http_403_prefix()),
        ],
    );

    assert_operation(&export, "http_connect_denied");
    assert_stage(&export, "send_connect_request");
    assert_stage(&export, "receive_connect_denied");

    let ir = protocol_ir(&export, "http_connect_denied");
    assert_surface(
        ir,
        "http",
        "denied",
        "connect",
        "web-proxy-request-response",
    );
    assert_eq!(ir.semantics_category.as_deref(), Some("failure-path"));
    assert_json_replay(&export);
}

#[test]
fn http_connect_denied_runtime_ir_does_not_materialize_for_success_response() {
    let export = run_tcp_path(
        "http_connect_denied_path.gewy",
        0x4832,
        8080,
        "curl",
        &[
            (PacketDir::Egress, connect_prefix()),
            (PacketDir::Ingress, http_200_prefix()),
        ],
    );

    assert_operation(&export, "http_connect_denied");
    assert_stage(&export, "send_connect_request");
    assert_no_stage(&export, "receive_connect_denied");
    assert_no_protocol_ir(&export, "http_connect_denied");
    assert_json_replay(&export);
}

#[test]
fn graphql_query_runtime_path_materializes_query_ir() {
    let export = run_tcp_path(
        "graphql_query_path.gewy",
        0x4731,
        80,
        "graphql-client",
        &[
            (PacketDir::Egress, http_get()),
            (PacketDir::Ingress, http_response()),
        ],
    );

    assert_operation(&export, "graphql_query");
    assert_stage(&export, "send_query_get");
    assert_stage(&export, "receive_response");

    let ir = protocol_ir(&export, "graphql_query");
    assert_surface(
        ir,
        "graphql",
        "query",
        "query",
        "web-proxy-request-response",
    );
    assert_eq!(ir.semantics_category.as_deref(), Some("graphql-query-path"));
    assert_json_replay(&export);
}

#[test]
fn s3_get_object_runtime_path_materializes_object_read_ir() {
    let export = run_tcp_path(
        "s3_get_object_path.gewy",
        0x5331,
        80,
        "aws",
        &[
            (PacketDir::Egress, http_get()),
            (PacketDir::Ingress, http_response()),
        ],
    );

    assert_operation(&export, "s3_get_object");
    assert_stage(&export, "send_get_object");
    assert_stage(&export, "receive_response");

    let ir = protocol_ir(&export, "s3_get_object");
    assert_surface(
        ir,
        "s3",
        "get-object",
        "object-read",
        "web-proxy-request-response",
    );
    assert_eq!(ir.semantics_category.as_deref(), Some("s3-get-object-path"));
    assert_json_replay(&export);
}

#[test]
fn prometheus_remote_write_runtime_path_materializes_collection_ir() {
    let export = run_tcp_path(
        "prometheus_remote_write_path.gewy",
        0x5031,
        80,
        "prometheus",
        &[
            (PacketDir::Egress, http_post()),
            (PacketDir::Ingress, http_response()),
        ],
    );

    assert_operation(&export, "prometheus_remote_write");
    assert_stage(&export, "send_write_batch");
    assert_stage(&export, "receive_write_response");

    let ir = protocol_ir(&export, "prometheus_remote_write");
    assert_surface(
        ir,
        "prometheus",
        "remote-write",
        "metrics-collection",
        "web-proxy-request-response",
    );
    assert_eq!(
        ir.semantics_category.as_deref(),
        Some("prometheus-remote-write-path")
    );
    assert_json_replay(&export);
}

#[test]
fn otlp_traces_runtime_path_materializes_signal_export_ir_from_export_operation() {
    let export = run_tcp_path(
        "otlp_traces_path.gewy",
        0x4f31,
        443,
        "otelcol",
        &[
            (PacketDir::Egress, &[(3, 0x01)][..]),
            (PacketDir::Egress, &[(3, 0x00)][..]),
        ],
    );

    assert_operation(&export, "otlp_traces_export");
    assert_stage(&export, "send_trace_headers");
    assert_stage(&export, "send_trace_batch");

    let ir = protocol_ir(&export, "otlp_traces_export");
    assert_surface(
        ir,
        "otlp",
        "traces",
        "signal-export",
        "web-proxy-request-response",
    );
    assert_eq!(
        ir.semantics_category.as_deref(),
        Some("otlp-traces-export-path")
    );
    assert_json_replay(&export);
}

#[test]
fn loki_push_runtime_path_materializes_log_ingest_ir() {
    let export = run_tcp_path(
        "loki_push_path.gewy",
        0x4c31,
        80,
        "promtail",
        &[
            (PacketDir::Egress, http_post()),
            (PacketDir::Ingress, http_response()),
        ],
    );

    assert_operation(&export, "loki_push");
    assert_stage(&export, "send_log_batch");
    assert_stage(&export, "receive_push_response");

    let ir = protocol_ir(&export, "loki_push");
    assert_surface(
        ir,
        "loki",
        "push",
        "log-ingest",
        "web-proxy-request-response",
    );
    assert_eq!(ir.semantics_category.as_deref(), Some("loki-push-path"));
    assert_json_replay(&export);
}

#[test]
fn jaeger_collector_runtime_path_materializes_trace_ingest_ir() {
    let export = run_tcp_path(
        "jaeger_collector_path.gewy",
        0x4a31,
        80,
        "jaeger-client",
        &[
            (PacketDir::Egress, http_post()),
            (PacketDir::Ingress, http_response()),
        ],
    );

    assert_operation(&export, "jaeger_collector");
    assert_stage(&export, "send_spans");
    assert_stage(&export, "receive_collector_response");

    let ir = protocol_ir(&export, "jaeger_collector");
    assert_surface(
        ir,
        "jaeger",
        "collector",
        "trace-ingest",
        "web-proxy-request-response",
    );
    assert_eq!(
        ir.semantics_category.as_deref(),
        Some("jaeger-collector-path")
    );
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
    session.ingest(sock_lineage_fact(1, cookie, 8300, process_name));
    session.ingest(tcp_state_fact_with_ports(
        2,
        cookie,
        1,
        2,
        48000,
        server_port,
    ));
    session.ingest(tcp_state_fact_with_ports(
        3,
        cookie,
        2,
        3,
        48000,
        server_port,
    ));
    session.ingest(route_fact(4, cookie, 6));

    for (index, (dir, payload_bytes)) in packets.iter().enumerate() {
        session.ingest(packet_fact_with_dir_and_payload_bytes(
            5 + index as u64,
            cookie,
            0x18,
            *dir,
            Some(48000),
            Some(server_port),
            payload_bytes,
        ));
    }

    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(110));
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

fn connect_prefix() -> &'static [(u16, u8)] {
    &[(0, 0x43), (1, 0x4f), (2, 0x4e), (3, 0x4e)]
}

fn http_403_prefix() -> &'static [(u16, u8)] {
    &[(0, 0x34), (1, 0x30), (2, 0x33), (3, 0x20)]
}

fn http_200_prefix() -> &'static [(u16, u8)] {
    &[(0, 0x32), (1, 0x30), (2, 0x30), (3, 0x20)]
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
