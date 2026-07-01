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
fn amqp_publish_runtime_path_materializes_ack_and_protocol_ir() {
    let export = run_amqp_path(
        "amqp_basic_publish_path.gewy",
        &[
            (PacketDir::Egress, &[(0, 0x01), (10, 0x28)][..]),
            (PacketDir::Ingress, &[(0, 0x01), (10, 0x50)][..]),
        ],
    );

    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("amqp_basic_publish".into())
    );
    assert_stage(&export, "send_publish");
    assert_stage(&export, "receive_ack");

    let ir = protocol_ir(&export, "amqp_basic_publish");
    assert_eq!(ir.protocol, "amqp");
    assert_eq!(ir.entry, "publish");
    assert_eq!(ir.cluster_key.as_deref(), Some("cache-queue-stream"));
    assert_eq!(ir.shelf_key.as_deref(), Some("session-publish"));
}

#[test]
fn amqp_consume_runtime_path_materializes_delivery_and_protocol_ir() {
    let export = run_amqp_path(
        "amqp_basic_consume_path.gewy",
        &[
            (PacketDir::Egress, &[(0, 0x01), (10, 0x14)][..]),
            (PacketDir::Ingress, &[(0, 0x01), (10, 0x3c)][..]),
        ],
    );

    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("amqp_basic_consume".into())
    );
    assert_stage(&export, "send_consume");
    assert_stage(&export, "receive_deliver");

    let ir = protocol_ir(&export, "amqp_basic_consume");
    assert_eq!(ir.protocol, "amqp");
    assert_eq!(ir.entry, "consume");
    assert_eq!(ir.shelf_key.as_deref(), Some("consume"));
}

#[test]
fn amqp_auth_denied_runtime_path_keeps_failure_semantics() {
    let export = run_amqp_path(
        "amqp_auth_denied_path.gewy",
        &[
            (
                PacketDir::Egress,
                &[(0, b'A'), (1, b'M'), (2, b'Q'), (3, b'P')][..],
            ),
            (PacketDir::Ingress, &[(0, 0x01), (10, 0x0a)][..]),
            (PacketDir::Egress, &[(0, 0x01), (10, 0x0b)][..]),
            (PacketDir::Ingress, &[(0, 0x01), (10, 0x32)][..]),
        ],
    );

    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("amqp_auth_denied".into())
    );
    assert_stage(&export, "receive_connection_close");

    let ir = protocol_ir(&export, "amqp_auth_denied");
    assert_eq!(ir.protocol, "amqp");
    assert_eq!(ir.entry, "auth-denied");
    assert_eq!(ir.shelf_key.as_deref(), Some("start-negotiation"));
    assert_eq!(ir.semantics_category.as_deref(), Some("failure-path"));
    assert_eq!(ir.typical_signal.as_deref(), Some("connection.close"));

    let replayed = ExportBundle::from_json(&export.to_json()).expect("export json should replay");
    assert_eq!(replayed.protocol_ir, export.protocol_ir);
}

fn run_amqp_path(
    name: &str,
    packets: &[(PacketDir, &[(u16, u8)])],
) -> gewyvern::export::ExportBundle {
    let binding = compile_file(&dsl_fixture_path(name)).unwrap();
    let config = SessionConfig::for_template(binding.template).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    let cookie = 0x616d;
    session.ingest(sock_lineage_fact(1, cookie, 56720, "amqp-client"));
    session.ingest(route_fact(2, cookie, 6));
    session.ingest(tcp_state_fact_with_ports(3, cookie, 1, 2, 43139, 5672));
    session.ingest(tcp_state_fact_with_ports(4, cookie, 2, 3, 43139, 5672));

    for (index, (dir, payload)) in packets.iter().enumerate() {
        session.ingest(packet_fact_with_dir_and_payload_bytes(
            5 + index as u64,
            cookie,
            0x18,
            *dir,
            Some(43139),
            Some(5672),
            payload,
        ));
    }

    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(120));
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
