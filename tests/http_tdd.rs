use gewyvern::dsl::compile_file;
use gewyvern::http::{
    HttpComponentKind, HttpSuspectSide, HttpTransactionVerdict, compose_http_transactions,
};
use gewyvern::runtime::{RuntimeSession, SessionConfig};

mod support;

use gewyvern::ledger::PacketDir;
use gewyvern::ledger::{QuicFrameType, QuicPacketType};
use std::time::{Duration, SystemTime};
use support::{
    packet_fact_with_dir, route_fact, sock_lineage_fact, tcp_state_fact_with_ports,
    udp_packet_fact_with_dir, udp_packet_fact_with_dir_and_ports_and_payload, udp_quic_meta_fact,
};

fn dsl_fixture_path(name: &str) -> String {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("dsl")
        .join(name)
        .to_string_lossy()
        .into_owned()
}
#[test]
fn http_transaction_composes_dns_and_client_request_for_same_process() {
    let dns_binding = compile_file(&dsl_fixture_path("dns_udp_process.gewy")).unwrap();
    let http_binding = compile_file(&dsl_fixture_path("http_request_path.gewy")).unwrap();

    let mut dns_session =
        RuntimeSession::start(SessionConfig::for_binding(dns_binding).unwrap()).unwrap();
    dns_session.ingest(sock_lineage_fact(1, 901, 4242, "curl"));
    dns_session.ingest(route_fact(2, 901, 7));
    dns_session.ingest(udp_packet_fact_with_dir(3, 901, 80, PacketDir::Egress));
    dns_session.ingest(udp_packet_fact_with_dir(4, 901, 128, PacketDir::Ingress));
    dns_session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(50));

    let mut http_session =
        RuntimeSession::start(SessionConfig::for_binding(http_binding).unwrap()).unwrap();
    http_session.ingest(sock_lineage_fact(10, 902, 4242, "curl"));
    http_session.ingest(route_fact(11, 902, 7));
    http_session.ingest(tcp_state_fact_with_ports(12, 902, 1, 2, 42000, 443));
    http_session.ingest(tcp_state_fact_with_ports(13, 902, 2, 3, 42000, 443));
    http_session.ingest(packet_fact_with_dir(14, 902, 0x18, PacketDir::Egress));
    http_session.ingest(packet_fact_with_dir(15, 902, 0x18, PacketDir::Ingress));
    http_session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(160));

    let transactions =
        compose_http_transactions(&[dns_session.export_bundle(), http_session.export_bundle()]);
    assert_eq!(transactions.len(), 1);
    assert_eq!(
        transactions[0].client_process.as_ref().unwrap().comm,
        "curl"
    );
    assert!(
        transactions[0]
            .components
            .iter()
            .any(|component| component.kind == HttpComponentKind::DnsLookup)
    );
    assert!(
        transactions[0]
            .components
            .iter()
            .any(|component| component.kind == HttpComponentKind::ClientRequest)
    );
    assert!(transactions[0].phases.contains(&"send_request".to_string()));
    assert!(
        transactions[0]
            .phases
            .contains(&"receive_response".to_string())
    );
    assert!(
        transactions[0]
            .phase_kinds
            .contains(&"emit_datagram".to_string())
    );
    assert!(
        transactions[0]
            .phase_kinds
            .contains(&"emit_payload".to_string())
    );
    assert!(
        transactions[0]
            .phase_kinds
            .contains(&"receive_payload".to_string())
    );
    assert_eq!(
        transactions[0].verdict,
        HttpTransactionVerdict::HealthyRequestResponsePath
    );
}

#[test]
fn http_transaction_can_attach_overlapping_server_response_component() {
    let http_binding = compile_file(&dsl_fixture_path("http_request_path.gewy")).unwrap();
    let server_binding = compile_file(&dsl_fixture_path("http_server_response_path.gewy")).unwrap();

    let mut http_session =
        RuntimeSession::start(SessionConfig::for_binding(http_binding).unwrap()).unwrap();
    http_session.ingest(sock_lineage_fact(1, 903, 4242, "curl"));
    http_session.ingest(route_fact(2, 903, 7));
    http_session.ingest(tcp_state_fact_with_ports(3, 903, 1, 2, 42000, 443));
    http_session.ingest(tcp_state_fact_with_ports(4, 903, 2, 3, 42000, 443));
    http_session.ingest(packet_fact_with_dir(5, 903, 0x18, PacketDir::Egress));
    http_session.ingest(packet_fact_with_dir(6, 903, 0x18, PacketDir::Ingress));
    http_session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(70));

    let mut server_session =
        RuntimeSession::start(SessionConfig::for_binding(server_binding).unwrap()).unwrap();
    server_session.ingest(sock_lineage_fact(7, 904, 8080, "nginx"));
    server_session.ingest(tcp_state_fact_with_ports(8, 904, 1, 2, 80, 53000));
    server_session.ingest(tcp_state_fact_with_ports(9, 904, 2, 3, 80, 53000));
    server_session.ingest(packet_fact_with_dir(10, 904, 0x18, PacketDir::Ingress));
    server_session.ingest(packet_fact_with_dir(11, 904, 0x18, PacketDir::Egress));
    server_session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(75));

    let transactions =
        compose_http_transactions(&[http_session.export_bundle(), server_session.export_bundle()]);
    assert_eq!(transactions.len(), 1);
    assert_eq!(
        transactions[0].server_process.as_ref().unwrap().comm,
        "nginx"
    );
    assert!(
        transactions[0]
            .components
            .iter()
            .any(|component| component.kind == HttpComponentKind::ServerResponse)
    );
    assert!(
        transactions[0]
            .phases
            .contains(&"receive_request".to_string())
    );
    assert!(
        transactions[0]
            .phases
            .contains(&"send_response".to_string())
    );
    assert!(
        transactions[0]
            .phase_kinds
            .contains(&"receive_payload".to_string())
    );
    assert!(
        transactions[0]
            .phase_kinds
            .contains(&"emit_payload".to_string())
    );
}

#[test]
fn http_transaction_lifts_client_findings_into_transaction_summary() {
    let http_binding = compile_file(&dsl_fixture_path("http_request_path.gewy")).unwrap();

    let mut http_session =
        RuntimeSession::start(SessionConfig::for_binding(http_binding).unwrap()).unwrap();
    http_session.ingest(sock_lineage_fact(1, 905, 4242, "curl"));
    http_session.ingest(route_fact(2, 905, 7));
    http_session.ingest(tcp_state_fact_with_ports(3, 905, 1, 2, 42000, 443));
    http_session.ingest(tcp_state_fact_with_ports(4, 905, 2, 3, 42000, 443));
    http_session.ingest(packet_fact_with_dir(5, 905, 0x18, PacketDir::Egress));
    http_session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(60));

    let transactions = compose_http_transactions(&[http_session.export_bundle()]);
    assert_eq!(transactions.len(), 1);
    assert_eq!(
        transactions[0].severity,
        Some(gewyvern::flow::ModuleSeverity::Low)
    );
    assert!(
        transactions[0]
            .suspect_sides
            .contains(&HttpSuspectSide::Client)
    );
    assert!(
        transactions[0]
            .finding_summaries
            .iter()
            .any(|summary| summary.contains("receive_response"))
    );
    assert_eq!(
        transactions[0].verdict,
        HttpTransactionVerdict::SuspectClientResponseGap
    );
}

#[test]
fn http_transaction_lifts_server_findings_into_transaction_summary() {
    let http_binding = compile_file(&dsl_fixture_path("http_request_path.gewy")).unwrap();
    let server_binding = compile_file(&dsl_fixture_path("http_server_response_path.gewy")).unwrap();

    let mut http_session =
        RuntimeSession::start(SessionConfig::for_binding(http_binding).unwrap()).unwrap();
    http_session.ingest(sock_lineage_fact(1, 906, 4242, "curl"));
    http_session.ingest(route_fact(2, 906, 7));
    http_session.ingest(tcp_state_fact_with_ports(3, 906, 1, 2, 42000, 443));
    http_session.ingest(tcp_state_fact_with_ports(4, 906, 2, 3, 42000, 443));
    http_session.ingest(packet_fact_with_dir(5, 906, 0x18, PacketDir::Egress));
    http_session.ingest(packet_fact_with_dir(6, 906, 0x18, PacketDir::Ingress));
    http_session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(70));

    let mut server_session =
        RuntimeSession::start(SessionConfig::for_binding(server_binding).unwrap()).unwrap();
    server_session.ingest(sock_lineage_fact(7, 907, 8080, "nginx"));
    server_session.ingest(tcp_state_fact_with_ports(8, 907, 1, 2, 80, 53000));
    server_session.ingest(tcp_state_fact_with_ports(9, 907, 2, 3, 80, 53000));
    server_session.ingest(packet_fact_with_dir(10, 907, 0x18, PacketDir::Ingress));
    server_session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(75));

    let transactions =
        compose_http_transactions(&[http_session.export_bundle(), server_session.export_bundle()]);
    assert_eq!(transactions.len(), 1);
    assert!(
        transactions[0]
            .suspect_sides
            .contains(&HttpSuspectSide::Server)
    );
    assert!(
        transactions[0]
            .finding_summaries
            .iter()
            .any(|summary| summary.contains("send_response"))
    );
    assert_eq!(
        transactions[0].verdict,
        HttpTransactionVerdict::SuspectServerResponseGap
    );
}

#[test]
fn http_transaction_lifts_dns_findings_into_transaction_verdict() {
    let dns_binding = compile_file(&dsl_fixture_path("dns_udp_process.gewy")).unwrap();
    let http_binding = compile_file(&dsl_fixture_path("http_request_path.gewy")).unwrap();

    let mut dns_session =
        RuntimeSession::start(SessionConfig::for_binding(dns_binding).unwrap()).unwrap();
    dns_session.ingest(sock_lineage_fact(1, 908, 4242, "curl"));
    dns_session.ingest(route_fact(2, 908, 7));
    dns_session.ingest(udp_packet_fact_with_dir(3, 908, 80, PacketDir::Egress));
    dns_session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(40));

    let mut http_session =
        RuntimeSession::start(SessionConfig::for_binding(http_binding).unwrap()).unwrap();
    http_session.ingest(sock_lineage_fact(10, 909, 4242, "curl"));
    http_session.ingest(route_fact(11, 909, 7));
    http_session.ingest(tcp_state_fact_with_ports(12, 909, 1, 2, 42000, 443));
    http_session.ingest(tcp_state_fact_with_ports(13, 909, 2, 3, 42000, 443));
    http_session.ingest(packet_fact_with_dir(14, 909, 0x18, PacketDir::Egress));
    http_session.ingest(packet_fact_with_dir(15, 909, 0x18, PacketDir::Ingress));
    http_session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(160));

    let transactions =
        compose_http_transactions(&[dns_session.export_bundle(), http_session.export_bundle()]);
    assert_eq!(transactions.len(), 1);
    assert!(
        transactions[0]
            .suspect_sides
            .contains(&HttpSuspectSide::Dns)
    );
    assert!(
        transactions[0]
            .finding_summaries
            .iter()
            .any(|summary| summary.contains("receive_reply"))
    );
    assert_eq!(
        transactions[0].verdict,
        HttpTransactionVerdict::SuspectDnsResolutionGap
    );
}

#[test]
fn http3_transaction_composes_client_and_server_components() {
    let client_binding = compile_file(&dsl_fixture_path("http3_request_path.gewy")).unwrap();
    let server_binding =
        compile_file(&dsl_fixture_path("http3_server_response_path.gewy")).unwrap();

    let mut client_session =
        RuntimeSession::start(SessionConfig::for_binding(client_binding).unwrap()).unwrap();
    client_session.ingest(sock_lineage_fact(1, 1101, 4242, "curl"));
    client_session.ingest(route_fact(2, 1101, 7));
    client_session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        3,
        1101,
        1300,
        PacketDir::Egress,
        Some(53000),
        Some(443),
        Some(0xc0),
        Some(0xc300),
    ));
    client_session.ingest(udp_quic_meta_fact(
        4,
        1101,
        PacketDir::Egress,
        Some(53000),
        Some(443),
        true,
        Some(QuicPacketType::Initial),
        vec![],
    ));
    client_session.ingest(udp_quic_meta_fact(
        5,
        1101,
        PacketDir::Egress,
        Some(53000),
        Some(443),
        true,
        None,
        vec![QuicFrameType::Crypto],
    ));
    client_session.ingest(udp_quic_meta_fact(
        6,
        1101,
        PacketDir::Egress,
        Some(53000),
        Some(443),
        false,
        None,
        vec![QuicFrameType::Stream],
    ));
    client_session.ingest(udp_quic_meta_fact(
        7,
        1101,
        PacketDir::Ingress,
        Some(53000),
        Some(443),
        false,
        None,
        vec![QuicFrameType::Stream],
    ));
    client_session.ingest(udp_quic_meta_fact(
        8,
        1101,
        PacketDir::Ingress,
        Some(53000),
        Some(443),
        false,
        None,
        vec![QuicFrameType::ConnectionClose],
    ));
    client_session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(120));

    let mut server_session =
        RuntimeSession::start(SessionConfig::for_binding(server_binding).unwrap()).unwrap();
    server_session.ingest(sock_lineage_fact(9, 2202, 8080, "nginx"));
    server_session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        10,
        2202,
        1300,
        PacketDir::Ingress,
        Some(443),
        Some(53000),
        Some(0xc0),
        Some(0xc300),
    ));
    server_session.ingest(udp_quic_meta_fact(
        11,
        2202,
        PacketDir::Ingress,
        Some(443),
        Some(53000),
        true,
        Some(QuicPacketType::Initial),
        vec![],
    ));
    server_session.ingest(udp_quic_meta_fact(
        12,
        2202,
        PacketDir::Ingress,
        Some(443),
        Some(53000),
        true,
        None,
        vec![QuicFrameType::Crypto],
    ));
    server_session.ingest(udp_quic_meta_fact(
        13,
        2202,
        PacketDir::Egress,
        Some(443),
        Some(53000),
        true,
        Some(QuicPacketType::Handshake),
        vec![],
    ));
    server_session.ingest(udp_quic_meta_fact(
        14,
        2202,
        PacketDir::Egress,
        Some(443),
        Some(53000),
        true,
        None,
        vec![QuicFrameType::Crypto],
    ));
    server_session.ingest(udp_quic_meta_fact(
        15,
        2202,
        PacketDir::Ingress,
        Some(443),
        Some(53000),
        false,
        None,
        vec![QuicFrameType::Stream],
    ));
    server_session.ingest(udp_quic_meta_fact(
        16,
        2202,
        PacketDir::Egress,
        Some(443),
        Some(53000),
        false,
        None,
        vec![QuicFrameType::Stream],
    ));
    server_session.ingest(udp_quic_meta_fact(
        17,
        2202,
        PacketDir::Egress,
        Some(443),
        Some(53000),
        false,
        None,
        vec![QuicFrameType::ConnectionClose],
    ));
    server_session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(125));

    let transactions = compose_http_transactions(&[
        client_session.export_bundle(),
        server_session.export_bundle(),
    ]);
    assert_eq!(transactions.len(), 1);
    assert_eq!(
        transactions[0].client_process.as_ref().unwrap().comm,
        "curl"
    );
    assert_eq!(
        transactions[0].server_process.as_ref().unwrap().comm,
        "nginx"
    );
    assert!(
        transactions[0]
            .components
            .iter()
            .any(|component| component.operation
                == gewyvern::flow::ProgramOperation::Custom("http3_request".into()))
    );
    assert!(
        transactions[0]
            .components
            .iter()
            .any(|component| component.operation
                == gewyvern::flow::ProgramOperation::Custom("http3_server_response".into()))
    );
    assert!(
        transactions[0]
            .phases
            .contains(&"send_request_stream".to_string())
    );
    assert!(
        transactions[0]
            .phases
            .contains(&"receive_request_stream".to_string())
    );
    assert!(
        transactions[0]
            .phases
            .contains(&"send_response_stream".to_string())
    );
    assert_eq!(
        transactions[0].verdict,
        HttpTransactionVerdict::HealthyRequestResponsePath
    );
}
