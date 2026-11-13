mod support;

use gewyvern::dsl::compile_file;
use gewyvern::export::ExportBundle;
use gewyvern::flow::ProgramOperation;
use gewyvern::ledger::PacketDir;
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

#[test]
fn mqtt_connect_recognizes_new_and_resumed_sessions_without_accepting_refusal() {
    for payload in [&[32, 2, 0, 0][..], &[32, 2, 1, 0]] {
        let export = run_mqtt_path("mqtt_connect_path.gewy", &[(PacketDir::Ingress, payload)]);
        assert_stage(&export, "receive_connack");
        let replayed = ExportBundle::from_json(&export.to_json()).unwrap();
        assert_eq!(replayed.program_flows, export.program_flows);
        assert_eq!(replayed.reasons, export.reasons);
    }
    for payload in [
        &[32, 2, 0, 1][..],
        &[32, 2, 0, 2],
        &[32, 2, 1, 5],
        &[32, 2, 2, 0],
        &[33, 2, 0, 0],
        &[32, 3, 0, 0],
        &[32, 2, 1],
    ] {
        let export = run_mqtt_path("mqtt_connect_path.gewy", &[(PacketDir::Ingress, payload)]);
        assert!(
            !export
                .program_flows
                .iter()
                .flat_map(|flow| &flow.stages)
                .any(|stage| stage.phase.as_deref() == Some("receive_connack")),
            "{payload:?}"
        );
    }
}

#[test]
fn mqtt_publish_runtime_path_materializes_publish_and_puback_stages() {
    let export = run_mqtt_path(
        "mqtt_publish_path.gewy",
        &[
            (PacketDir::Egress, &[0x30, 0x00][..]),
            (PacketDir::Ingress, &[0x40, 0x02, 0x00, 0x07][..]),
        ],
    );

    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("mqtt_publish".into())
    );
    assert_stage(&export, "connect");
    assert_stage(&export, "establish");
    assert_stage(&export, "send_publish");
    assert_stage(&export, "receive_puback");

    let ir = protocol_ir(&export, "mqtt_publish");
    assert_eq!(ir.protocol, "mqtt");
    assert_eq!(ir.entry, "publish");
    assert_eq!(ir.shelf_key.as_deref(), Some("publish"));
    assert_eq!(ir.cluster_key.as_deref(), Some("cache-queue-stream"));
    assert_eq!(
        ir.semantics_category.as_deref(),
        Some("message-publish-path")
    );
}

#[test]
fn mqtt_subscribe_runtime_path_materializes_subscribe_and_suback_stages() {
    let export = run_mqtt_path(
        "mqtt_subscribe_path.gewy",
        &[
            (PacketDir::Egress, &[0x82, 0x00][..]),
            (PacketDir::Ingress, &[0x90, 0x00][..]),
        ],
    );

    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("mqtt_subscribe".into())
    );
    assert_stage(&export, "send_subscribe");
    assert_stage(&export, "receive_suback");

    let ir = protocol_ir(&export, "mqtt_subscribe");
    assert_eq!(ir.protocol, "mqtt");
    assert_eq!(ir.entry, "subscribe");
    assert_eq!(ir.shelf_key.as_deref(), Some("subscribe"));
    assert_eq!(
        ir.semantics_category.as_deref(),
        Some("message-subscribe-path")
    );
}

#[test]
fn mqtt_qos2_pubcomp_runtime_path_materializes_full_ack_ladder() {
    let export = run_mqtt_path(
        "mqtt_pubcomp_path.gewy",
        &[
            (PacketDir::Ingress, &[0x50, 0x02][..]),
            (PacketDir::Egress, &[0x62, 0x02][..]),
            (PacketDir::Ingress, &[0x70, 0x02][..]),
        ],
    );

    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("mqtt_pubcomp".into())
    );
    assert_stage(&export, "receive_pubrec");
    assert_stage(&export, "send_pubrel");
    assert_stage(&export, "receive_pubcomp");

    let ir = protocol_ir(&export, "mqtt_pubcomp");
    assert_eq!(ir.protocol, "mqtt");
    assert_eq!(ir.entry, "pubcomp");
    assert_eq!(ir.shelf_key.as_deref(), Some("qos2-teardown"));
    assert_eq!(
        ir.semantics_category.as_deref(),
        Some("qos2-continuation-path")
    );

    let replayed = ExportBundle::from_json(&export.to_json()).expect("export json should replay");
    assert_eq!(replayed.protocol_ir, export.protocol_ir);
}

#[test]
fn mqtt_puback_candidates_allow_different_identifiers_but_require_the_header() {
    for id in [1u16, 7, 256, 65535] {
        let [high, low] = id.to_be_bytes();
        let export = run_mqtt_path(
            "mqtt_publish_path.gewy",
            &[(PacketDir::Ingress, &[64, 2, high, low])],
        );
        assert_stage(&export, "receive_puback");
    }
    for payload in [
        &[64][..],
        &[64, 2],
        &[64, 2, 0],
        &[65, 2, 0, 7],
        &[64, 3, 0, 7],
        &[80, 2, 0, 7],
    ] {
        let export = run_mqtt_path("mqtt_publish_path.gewy", &[(PacketDir::Ingress, payload)]);
        assert!(
            !export
                .program_flows
                .iter()
                .flat_map(|flow| &flow.stages)
                .any(|stage| stage.phase.as_deref() == Some("receive_puback")),
            "{payload:?}"
        );
    }
}

fn run_mqtt_path(name: &str, packets: &[(PacketDir, &[u8])]) -> gewyvern::export::ExportBundle {
    let binding = compile_file(&dsl_fixture_path(name)).unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    let cookie = 0x6d71;
    session.ingest(route_fact(1, cookie, 2));
    session.ingest(tcp_state_fact_with_ports(2, cookie, 1, 2, 45000, 1883));
    session.ingest(tcp_state_fact_with_ports(3, cookie, 2, 3, 45000, 1883));

    for (index, (dir, payload)) in packets.iter().enumerate() {
        let payload_bytes = payload
            .iter()
            .enumerate()
            .map(|(offset, value)| (offset as u16, *value))
            .collect::<Vec<_>>();
        session.ingest(packet_fact_with_dir_and_payload_bytes(
            4 + index as u64,
            cookie,
            0x18,
            *dir,
            Some(45000),
            Some(1883),
            &payload_bytes,
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
