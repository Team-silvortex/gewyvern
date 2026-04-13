use gewyvern::dsl::compile_file;
use gewyvern::http::{
    compose_http_transactions, HttpComponentKind, HttpSuspectSide, HttpTransactionVerdict,
};
use gewyvern::runtime::{RuntimeSession, SessionConfig};

mod support;

use support::{
    packet_fact_with_dir, route_fact, sock_lineage_fact, tcp_state_fact_with_ports,
    udp_packet_fact_with_dir,
};
use gewyvern::ledger::PacketDir;
use std::time::{Duration, SystemTime};

#[test]
fn http_transaction_composes_dns_and_client_request_for_same_process() {
    let dns_binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/dns_udp_process.gewy").unwrap();
    let http_binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/http_request_path.gewy").unwrap();

    let mut dns_session = RuntimeSession::start(SessionConfig::for_binding(dns_binding).unwrap()).unwrap();
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

    let transactions = compose_http_transactions(&[dns_session.export_bundle(), http_session.export_bundle()]);
    assert_eq!(transactions.len(), 1);
    assert_eq!(transactions[0].client_process.as_ref().unwrap().comm, "curl");
    assert!(transactions[0]
        .components
        .iter()
        .any(|component| component.kind == HttpComponentKind::DnsLookup));
    assert!(transactions[0]
        .components
        .iter()
        .any(|component| component.kind == HttpComponentKind::ClientRequest));
    assert!(transactions[0].phases.contains(&"send_request".to_string()));
    assert!(transactions[0].phases.contains(&"receive_response".to_string()));
    assert!(transactions[0]
        .phase_kinds
        .contains(&"emit_datagram".to_string()));
    assert!(transactions[0]
        .phase_kinds
        .contains(&"emit_payload".to_string()));
    assert!(transactions[0]
        .phase_kinds
        .contains(&"receive_payload".to_string()));
    assert_eq!(
        transactions[0].verdict,
        HttpTransactionVerdict::HealthyRequestResponsePath
    );
}

#[test]
fn http_transaction_can_attach_overlapping_server_response_component() {
    let http_binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/http_request_path.gewy").unwrap();
    let server_binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/http_server_response_path.gewy")
            .unwrap();

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
    assert!(transactions[0]
        .components
        .iter()
        .any(|component| component.kind == HttpComponentKind::ServerResponse));
    assert!(transactions[0].phases.contains(&"receive_request".to_string()));
    assert!(transactions[0].phases.contains(&"send_response".to_string()));
    assert!(transactions[0]
        .phase_kinds
        .contains(&"receive_payload".to_string()));
    assert!(transactions[0]
        .phase_kinds
        .contains(&"emit_payload".to_string()));
}

#[test]
fn http_transaction_lifts_client_findings_into_transaction_summary() {
    let http_binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/http_request_path.gewy").unwrap();

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
    assert_eq!(transactions[0].severity, Some(gewyvern::flow::ModuleSeverity::Low));
    assert!(transactions[0]
        .suspect_sides
        .contains(&HttpSuspectSide::Client));
    assert!(transactions[0]
        .finding_summaries
        .iter()
        .any(|summary| summary.contains("receive_response")));
    assert_eq!(
        transactions[0].verdict,
        HttpTransactionVerdict::SuspectClientResponseGap
    );
}

#[test]
fn http_transaction_lifts_server_findings_into_transaction_summary() {
    let http_binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/http_request_path.gewy").unwrap();
    let server_binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/http_server_response_path.gewy")
            .unwrap();

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
    assert!(transactions[0]
        .suspect_sides
        .contains(&HttpSuspectSide::Server));
    assert!(transactions[0]
        .finding_summaries
        .iter()
        .any(|summary| summary.contains("send_response")));
    assert_eq!(
        transactions[0].verdict,
        HttpTransactionVerdict::SuspectServerResponseGap
    );
}

#[test]
fn http_transaction_lifts_dns_findings_into_transaction_verdict() {
    let dns_binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/dns_udp_process.gewy").unwrap();
    let http_binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/http_request_path.gewy").unwrap();

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
    assert!(transactions[0]
        .suspect_sides
        .contains(&HttpSuspectSide::Dns));
    assert!(transactions[0]
        .finding_summaries
        .iter()
        .any(|summary| summary.contains("receive_reply")));
    assert_eq!(
        transactions[0].verdict,
        HttpTransactionVerdict::SuspectDnsResolutionGap
    );
}
