use gewyvern::dsl::{DslError, compile_file, compile_str, parse_str_unvalidated};
use gewyvern::flow::ProgramOperation;
use gewyvern::fragment::{RegistryError, RuleTier};
use gewyvern::gewyc::collect_binding_diagnostics;
use gewyvern::ledger::PacketDir;
use gewyvern::reason::{KeyEventKind, ReasonProfile};
use gewyvern::runtime::{RuntimeSession, SessionConfig};
use gewyvern::template::FragmentParamValue;

mod support;

use std::time::{Duration, SystemTime};
use support::{
    packet_fact, packet_fact_with_dir, packet_fact_with_dir_and_payload,
    packet_fact_with_dir_and_payload_and_byte4, packet_fact_with_dir_and_payload_and_bytes4_5_and9,
    packet_fact_with_dir_and_payload_and_bytes4_and5, route_fact, sock_lineage_fact,
    tcp_state_fact, tcp_state_fact_with_ports, udp_packet_fact, udp_packet_fact_with_dir,
    udp_packet_fact_with_dir_and_ports, udp_packet_fact_with_dir_and_ports_and_payload,
    udp_packet_fact_with_dir_and_ports_and_payload_prefix4,
    udp_packet_fact_with_dir_and_ports_and_payload_prefix4_and_byte13,
};

#[test]
fn built_in_udp_process_dsl_compiles_into_template_binding() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy").unwrap();

    assert_eq!(binding.template.id, "udp_process_debug");
    assert_eq!(
        binding.template.fragment_set,
        vec![
            "udp_packet_meta_fragment",
            "route_meta_fragment",
            "sock_lineage_fragment"
        ]
    );
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::DatagramExchange
    );
    assert_eq!(
        binding
            .template
            .window_profile
            .as_ref()
            .unwrap()
            .duration_ms,
        5_000
    );
    assert_eq!(
        binding
            .template
            .window_profile
            .as_ref()
            .unwrap()
            .lateness_ms,
        200
    );
    assert_eq!(
        binding.fragment_params["sock_lineage_fragment"]["capture_comm"],
        FragmentParamValue::Bool(true)
    );
}

#[test]
fn built_in_structured_udp_process_dsl_compiles_into_template_binding() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/structured_udp_process_debug.gewy")
            .unwrap();

    assert_eq!(binding.template.id, "structured_udp_process_debug");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::DatagramExchange
    );
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().rules.len(),
        3
    );
}

#[test]
fn built_in_dns_udp_process_dsl_compiles_into_template_binding() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/dns_udp_process.gewy").unwrap();

    assert_eq!(binding.template.id, "dns_udp_process");
    assert_eq!(
        binding.template.fragment_set,
        vec![
            "udp_packet_meta_fragment",
            "route_meta_fragment",
            "sock_lineage_fragment"
        ]
    );
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("dns_lookup".into())
    );
    assert!(matches!(
        binding.template.reason_profile.as_ref().unwrap(),
        ReasonProfile::Declarative(_)
    ));
}

#[test]
fn built_in_https_connect_process_dsl_compiles_into_template_binding() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/https_connect_process.gewy").unwrap();

    assert_eq!(binding.template.id, "https_connect_process");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("https_connect".into())
    );
    assert!(matches!(
        binding.template.reason_profile.as_ref().unwrap(),
        ReasonProfile::Declarative(_)
    ));
}

#[test]
fn built_in_postgres_connect_process_dsl_compiles_into_template_binding() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/postgres_connect_process.gewy")
            .unwrap();

    assert_eq!(binding.template.id, "postgres_connect_process");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("postgres_connect".into())
    );
}

#[test]
fn built_in_postgres_simple_query_path_dsl_compiles_into_template_binding() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/postgres_simple_query_path.gewy")
            .unwrap();

    assert_eq!(binding.template.id, "postgres_simple_query_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("postgres_simple_query".into())
    );
    assert!(matches!(
        binding.template.reason_profile.as_ref().unwrap(),
        ReasonProfile::Declarative(_)
    ));
}

#[test]
fn built_in_redis_connect_process_dsl_compiles_into_template_binding() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/redis_connect_process.gewy").unwrap();

    assert_eq!(binding.template.id, "redis_connect_process");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("redis_connect".into())
    );
}

#[test]
fn built_in_gtpu_echo_path_dsl_compiles_into_template_binding() {
    let binding = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/gtpu_echo_path.gewy")
        .unwrap();

    assert_eq!(binding.template.id, "gtpu_echo_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("gtpu_echo".into())
    );
    assert!(matches!(
        binding.template.reason_profile.as_ref().unwrap(),
        ReasonProfile::Declarative(_)
    ));
}

#[test]
fn built_in_http_request_path_dsl_compiles_into_template_binding() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/http_request_path.gewy").unwrap();

    assert_eq!(binding.template.id, "http_request_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("http_request".into())
    );
}

#[test]
fn built_in_http_server_response_path_dsl_compiles_into_template_binding() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/http_server_response_path.gewy")
            .unwrap();

    assert_eq!(binding.template.id, "http_server_response_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("http_server_response".into())
    );
}

#[test]
fn dsl_accepts_local_remote_port_predicates_and_legacy_aliases() {
    let local_binding = compile_str(
        r#"
template=http_server_compat
window=default_5s
reason=handshake_l1
fragment=tcp_state_fragment
program_model=http_server_compat_model
operation=http_server_response
rule=socket_state_observed:local:http;socket_state_transition;static:local http socket observed;true
"#,
    )
    .unwrap();
    let legacy_binding = compile_str(
        r#"
template=http_server_legacy
window=default_5s
reason=handshake_l1
fragment=tcp_state_fragment
program_model=http_server_legacy_model
operation=http_server_response
rule=socket_state_observed:sport:http;socket_state_transition;static:legacy local http socket observed;true
"#,
    )
    .unwrap();
    let remote_binding = compile_str(
        r#"
template=http_client_remote
window=default_5s
reason=handshake_l1
fragment=tcp_state_fragment
program_model=http_client_remote_model
operation=http_request
rule=socket_state_observed:remote:https;socket_state_transition;static:remote https socket observed;true
"#,
    )
    .unwrap();

    let local_rule = &local_binding.template.program_model.as_ref().unwrap().rules[0];
    let legacy_rule = &legacy_binding
        .template
        .program_model
        .as_ref()
        .unwrap()
        .rules[0];
    let remote_rule = &remote_binding
        .template
        .program_model
        .as_ref()
        .unwrap()
        .rules[0];

    assert_eq!(local_rule.predicate, legacy_rule.predicate);
    assert_eq!(
        remote_rule.predicate,
        gewyvern::ir::FlowPredicate::SocketStateObserved {
            local_port: None,
            remote_port: Some(443),
            min_new_state: None,
        }
    );
}

#[test]
fn dsl_accepts_flow_direction_predicates_and_legacy_aliases() {
    let packet_binding = compile_str(
        r#"
template=http_client_direction
window=default_5s
reason=handshake_l1
fragment=tcp_packet_meta_fragment
program_model=http_client_direction_model
operation=http_request
rule=packet_observed:tcp:local_to_remote;packet_observed;static:outbound http payload observed;true
"#,
    )
    .unwrap();
    let legacy_packet_binding = compile_str(
        r#"
template=http_client_direction_legacy
window=default_5s
reason=handshake_l1
fragment=tcp_packet_meta_fragment
program_model=http_client_direction_legacy_model
operation=http_request
rule=packet_observed:tcp:egress;packet_observed;static:legacy outbound http payload observed;true
"#,
    )
    .unwrap();
    let datagram_binding = compile_str(
        r#"
template=dns_direction
window=default_5s
reason=udp_datagram_l1
fragment=udp_packet_meta_fragment
program_model=dns_direction_model
operation=dns_lookup
rule=datagram_observed:udp:remote_to_local;datagram_observed;static:inbound dns datagram observed;true
"#,
    )
    .unwrap();
    let legacy_datagram_binding = compile_str(
        r#"
template=dns_direction_legacy
window=default_5s
reason=udp_datagram_l1
fragment=udp_packet_meta_fragment
program_model=dns_direction_legacy_model
operation=dns_lookup
rule=datagram_observed:udp:ingress;datagram_observed;static:legacy inbound dns datagram observed;true
"#,
    )
    .unwrap();

    let packet_rule = &packet_binding
        .template
        .program_model
        .as_ref()
        .unwrap()
        .rules[0];
    let legacy_packet_rule = &legacy_packet_binding
        .template
        .program_model
        .as_ref()
        .unwrap()
        .rules[0];
    let datagram_rule = &datagram_binding
        .template
        .program_model
        .as_ref()
        .unwrap()
        .rules[0];
    let legacy_datagram_rule = &legacy_datagram_binding
        .template
        .program_model
        .as_ref()
        .unwrap()
        .rules[0];

    assert_eq!(packet_rule.predicate, legacy_packet_rule.predicate);
    assert_eq!(
        packet_rule.predicate,
        gewyvern::ir::FlowPredicate::PacketObserved {
            l4_proto: 6,
            dir: Some(PacketDir::Egress),
            local_port: None,
            remote_port: None,
            first_byte_mask: None,
            first_byte_value: None,
            prefix4: None,
            byte4_mask: None,
            byte4_value: None,
            byte13_mask: None,
            byte13_value: None,
            byte_matches: vec![],
        }
    );
    assert_eq!(datagram_rule.predicate, legacy_datagram_rule.predicate);
    assert_eq!(
        datagram_rule.predicate,
        gewyvern::ir::FlowPredicate::DatagramObserved {
            l4_proto: 17,
            dir: Some(PacketDir::Ingress),
            local_port: None,
            remote_port: None,
            min_len: None,
            first_byte_mask: None,
            first_byte_value: None,
            prefix2: None,
            prefix4: None,
            byte13_mask: None,
            byte13_value: None,
            byte_matches: vec![],
        }
    );
}

#[test]
fn dsl_accepts_packet_port_and_prefix4_qualifiers() {
    let binding = compile_str(
        r#"
template=redis_resp_packet_match
window=default_5s
reason=handshake_l1
fragment=tcp_packet_meta_fragment
program_model=redis_resp_packet_match_model
operation=redis_ping
rule=packet_observed:tcp:remote:redis:local_to_remote:byte0_mask:0xff:0x2a;packet_observed;transport_payload_sent;true
rule=packet_observed:tcp:remote:redis:remote_to_local:prefix4:0x2b504f4e;packet_observed;transport_payload_received;true
"#,
    )
    .unwrap();

    let request_rule = &binding.template.program_model.as_ref().unwrap().rules[0];
    let response_rule = &binding.template.program_model.as_ref().unwrap().rules[1];
    assert_eq!(
        request_rule.predicate,
        gewyvern::ir::FlowPredicate::PacketObserved {
            l4_proto: 6,
            dir: Some(PacketDir::Egress),
            local_port: None,
            remote_port: Some(6379),
            first_byte_mask: Some(0xff),
            first_byte_value: Some(0x2a),
            prefix4: None,
            byte4_mask: None,
            byte4_value: None,
            byte13_mask: None,
            byte13_value: None,
            byte_matches: vec![],
        }
    );
    assert_eq!(
        response_rule.predicate,
        gewyvern::ir::FlowPredicate::PacketObserved {
            l4_proto: 6,
            dir: Some(PacketDir::Ingress),
            local_port: None,
            remote_port: Some(6379),
            first_byte_mask: None,
            first_byte_value: None,
            prefix4: Some(0x2b504f4e),
            byte4_mask: None,
            byte4_value: None,
            byte13_mask: None,
            byte13_value: None,
            byte_matches: vec![],
        }
    );
}

#[test]
fn dsl_accepts_packet_byte4_mask_qualifier() {
    let binding = compile_str(
        r#"
template=dns_tcp_packet_match
window=default_5s
reason=handshake_l1
fragment=tcp_packet_meta_fragment
program_model=dns_tcp_packet_match_model
operation=dns_tcp_query
rule=packet_observed:tcp:remote:53:remote_to_local:byte4_mask:0x80:0x80;packet_observed;transport_payload_received;true
"#,
    )
    .unwrap();

    let rule = &binding.template.program_model.as_ref().unwrap().rules[0];
    assert_eq!(
        rule.predicate,
        gewyvern::ir::FlowPredicate::PacketObserved {
            l4_proto: 6,
            dir: Some(PacketDir::Ingress),
            local_port: None,
            remote_port: Some(53),
            first_byte_mask: None,
            first_byte_value: None,
            prefix4: None,
            byte4_mask: Some(0x80),
            byte4_value: Some(0x80),
            byte13_mask: None,
            byte13_value: None,
            byte_matches: vec![],
        }
    );
}

#[test]
fn dsl_accepts_datagram_port_predicates_and_named_quic_alias() {
    let remote_quic_binding = compile_str(
        r#"
template=quic_port_match
window=default_5s
reason=udp_datagram_l1
fragment=udp_packet_meta_fragment
program_model=quic_port_match_model
operation=quic_client_initial
rule=datagram_observed:udp:remote:quic:local_to_remote;datagram_observed;udp_datagram_sent;true
"#,
    )
    .unwrap();
    let legacy_remote_binding = compile_str(
        r#"
template=quic_port_match_legacy
window=default_5s
reason=udp_datagram_l1
fragment=udp_packet_meta_fragment
program_model=quic_port_match_legacy_model
operation=quic_client_initial
rule=datagram_observed:udp:dport:443:local_to_remote;datagram_observed;udp_datagram_sent;true
"#,
    )
    .unwrap();

    let rule = &remote_quic_binding
        .template
        .program_model
        .as_ref()
        .unwrap()
        .rules[0];
    let legacy_rule = &legacy_remote_binding
        .template
        .program_model
        .as_ref()
        .unwrap()
        .rules[0];
    assert_eq!(rule.predicate, legacy_rule.predicate);
    assert_eq!(
        rule.predicate,
        gewyvern::ir::FlowPredicate::DatagramObserved {
            l4_proto: 17,
            dir: Some(PacketDir::Egress),
            local_port: None,
            remote_port: Some(443),
            min_len: None,
            first_byte_mask: None,
            first_byte_value: None,
            prefix2: None,
            prefix4: None,
            byte13_mask: None,
            byte13_value: None,
            byte_matches: vec![],
        }
    );
}

#[test]
fn dsl_accepts_datagram_min_len_qualifier() {
    let binding = compile_str(
        r#"
template=quic_initial_len_match
window=default_5s
reason=udp_datagram_l1
fragment=udp_packet_meta_fragment
program_model=quic_initial_len_match_model
operation=quic_client_initial
rule=datagram_observed:udp:remote:quic:local_to_remote:min_len:1200;datagram_observed;udp_datagram_sent;true
"#,
    )
    .unwrap();

    let rule = &binding.template.program_model.as_ref().unwrap().rules[0];
    assert_eq!(
        rule.predicate,
        gewyvern::ir::FlowPredicate::DatagramObserved {
            l4_proto: 17,
            dir: Some(PacketDir::Egress),
            local_port: None,
            remote_port: Some(443),
            min_len: Some(1200),
            first_byte_mask: None,
            first_byte_value: None,
            prefix2: None,
            prefix4: None,
            byte13_mask: None,
            byte13_value: None,
            byte_matches: vec![],
        }
    );
}

#[test]
fn dsl_accepts_datagram_byte0_mask_qualifier() {
    let binding = compile_str(
        r#"
template=quic_initial_byte_match
window=default_5s
reason=udp_datagram_l1
fragment=udp_packet_meta_fragment
program_model=quic_initial_byte_match_model
operation=quic_client_initial
rule=datagram_observed:udp:remote:quic:local_to_remote:min_len:1200:byte0_mask:0xf0:0xc0;datagram_observed;udp_datagram_sent;true
"#,
    )
    .unwrap();

    let rule = &binding.template.program_model.as_ref().unwrap().rules[0];
    assert_eq!(
        rule.predicate,
        gewyvern::ir::FlowPredicate::DatagramObserved {
            l4_proto: 17,
            dir: Some(PacketDir::Egress),
            local_port: None,
            remote_port: Some(443),
            min_len: Some(1200),
            first_byte_mask: Some(0xf0),
            first_byte_value: Some(0xc0),
            prefix2: None,
            prefix4: None,
            byte13_mask: None,
            byte13_value: None,
            byte_matches: vec![],
        }
    );
}

#[test]
fn dsl_accepts_datagram_prefix4_qualifier() {
    let binding = compile_str(
        r#"
template=mdns_response_prefix4_match
window=default_5s
reason=udp_datagram_l1
fragment=udp_packet_meta_fragment
program_model=mdns_response_prefix4_match_model
operation=mdns_query
rule=datagram_observed:udp:remote:mdns:remote_to_local:prefix4:0x00008400;datagram_observed;udp_datagram_received;true
"#,
    )
    .unwrap();

    let rule = &binding.template.program_model.as_ref().unwrap().rules[0];
    assert_eq!(
        rule.predicate,
        gewyvern::ir::FlowPredicate::DatagramObserved {
            l4_proto: 17,
            dir: Some(PacketDir::Ingress),
            local_port: None,
            remote_port: Some(5353),
            min_len: None,
            first_byte_mask: None,
            first_byte_value: None,
            prefix2: None,
            prefix4: Some(0x00008400),
            byte13_mask: None,
            byte13_value: None,
            byte_matches: vec![],
        }
    );
}

#[test]
fn dsl_accepts_datagram_byte_at_qualifier() {
    let binding = compile_str(
        r#"
template=snmp_byte13_match
window=default_5s
reason=udp_datagram_l1
fragment=udp_packet_meta_fragment
program_model=snmp_byte13_match_model
operation=snmp_get
rule=datagram_observed:udp:remote:snmp:byte_at:13:0xff:0xa0;datagram_observed;udp_datagram_sent;true
"#,
    )
    .unwrap();

    let rule = &binding.template.program_model.as_ref().unwrap().rules[0];
    assert!(matches!(
        &rule.predicate,
        gewyvern::ir::FlowPredicate::DatagramObserved {
            remote_port: Some(161),
            byte13_mask: None,
            byte13_value: None,
            byte_matches,
            ..
        } if byte_matches == &vec![gewyvern::ir::PayloadByteMatch {
            offset: 13,
            mask: 0xff,
            value: 0xa0,
        }]
    ));
}

#[test]
fn directional_narrative_templates_render_and_replay_cleanly() {
    let binding = compile_str(
        r#"
template=directional_narrative_templates
window.duration_ms=5000
window.lateness_ms=200
fragment=udp_packet_meta_fragment
fragment=sock_lineage_fragment
operation=datagram_exchange
rule=process_bound;process_bound;process_bound;true
rule=datagram_observed:udp:remote_to_local;datagram_observed;udp_datagram_received;true
reason.rule=process_bound;process_identified;process_bound;true
reason.rule=datagram_observed:udp:remote_to_local;udp_datagram_seen;udp_datagram_received;true
"#,
    )
    .unwrap();

    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 207, 5355, "curl"));
    session.ingest(udp_packet_fact_with_dir(2, 207, 88, PacketDir::Ingress));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(40));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .narrative
            .iter()
            .any(|line| line == "program received a UDP datagram")
    );
    assert!(
        export.reasons[0]
            .l3
            .narrative
            .iter()
            .any(|line| line.text == "udp datagram received")
    );

    let replay = gewyvern::export::ExportBundle::from_json(&export.to_json())
        .unwrap()
        .replay()
        .unwrap();
    assert_eq!(export.program_flows, replay.program_flows);
    assert_eq!(export.reasons, replay.reasons);
}

#[test]
fn built_in_tls_client_path_dsl_compiles_into_template_binding() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/tls_client_path.gewy").unwrap();

    assert_eq!(binding.template.id, "tls_client_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("tls_client".into())
    );
}

#[test]
fn built_in_quic_client_initial_path_dsl_compiles_into_template_binding() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/quic_client_initial_path.gewy")
            .unwrap();

    assert_eq!(binding.template.id, "quic_client_initial_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("quic_client_initial".into())
    );
}

#[test]
fn built_in_stun_binding_path_dsl_compiles_into_template_binding() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/stun_binding_path.gewy").unwrap();

    assert_eq!(binding.template.id, "stun_binding_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("stun_binding".into())
    );
}

#[test]
fn built_in_coap_get_path_dsl_compiles_into_template_binding() {
    let binding = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/coap_get_path.gewy").unwrap();

    assert_eq!(binding.template.id, "coap_get_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("coap_get".into())
    );
}

#[test]
fn built_in_ntp_client_path_dsl_compiles_into_template_binding() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/ntp_client_path.gewy").unwrap();

    assert_eq!(binding.template.id, "ntp_client_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("ntp_client".into())
    );
}

#[test]
fn built_in_dhcp_client_path_dsl_compiles_into_template_binding() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/dhcp_client_path.gewy").unwrap();

    assert_eq!(binding.template.id, "dhcp_client_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("dhcp_client".into())
    );
}

#[test]
fn built_in_wireguard_handshake_path_dsl_compiles_into_template_binding() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/wireguard_handshake_path.gewy")
            .unwrap();

    assert_eq!(binding.template.id, "wireguard_handshake_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("wireguard_handshake".into())
    );
}

#[test]
fn built_in_mdns_query_path_dsl_compiles_into_template_binding() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/mdns_query_path.gewy").unwrap();

    assert_eq!(binding.template.id, "mdns_query_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("mdns_query".into())
    );
}

#[test]
fn built_in_ssdp_discovery_path_dsl_compiles_into_template_binding() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/ssdp_discovery_path.gewy").unwrap();

    assert_eq!(binding.template.id, "ssdp_discovery_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("ssdp_discovery".into())
    );
}

#[test]
fn built_in_redis_ping_path_dsl_compiles_into_template_binding() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/redis_ping_path.gewy").unwrap();

    assert_eq!(binding.template.id, "redis_ping_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("redis_ping".into())
    );
}

#[test]
fn built_in_mqtt_connect_path_dsl_compiles_into_template_binding() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/mqtt_connect_path.gewy").unwrap();

    assert_eq!(binding.template.id, "mqtt_connect_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("mqtt_connect".into())
    );
    assert!(matches!(
        binding.template.reason_profile.as_ref().unwrap(),
        ReasonProfile::Declarative(_)
    ));
}

#[test]
fn built_in_radius_access_path_dsl_compiles_into_template_binding() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/radius_access_path.gewy").unwrap();

    assert_eq!(binding.template.id, "radius_access_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("radius_access".into())
    );
    assert!(matches!(
        binding.template.reason_profile.as_ref().unwrap(),
        ReasonProfile::Declarative(_)
    ));
}

#[test]
fn built_in_smtp_session_path_dsl_compiles_into_template_binding() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/smtp_session_path.gewy").unwrap();

    assert_eq!(binding.template.id, "smtp_session_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("smtp_session".into())
    );
    assert!(matches!(
        binding.template.reason_profile.as_ref().unwrap(),
        ReasonProfile::Declarative(_)
    ));
}

#[test]
fn built_in_sip_register_path_dsl_compiles_into_template_binding() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/sip_register_path.gewy").unwrap();

    assert_eq!(binding.template.id, "sip_register_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("sip_register".into())
    );
    assert!(matches!(
        binding.template.reason_profile.as_ref().unwrap(),
        ReasonProfile::Declarative(_)
    ));
}

#[test]
fn built_in_ldap_bind_path_dsl_compiles_into_template_binding() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/ldap_bind_path.gewy").unwrap();

    assert_eq!(binding.template.id, "ldap_bind_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("ldap_bind".into())
    );
    assert!(matches!(
        binding.template.reason_profile.as_ref().unwrap(),
        ReasonProfile::Declarative(_)
    ));
}

#[test]
fn built_in_ldap_search_path_dsl_compiles_into_template_binding() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/ldap_search_path.gewy").unwrap();

    assert_eq!(binding.template.id, "ldap_search_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("ldap_search".into())
    );
    assert!(matches!(
        binding.template.reason_profile.as_ref().unwrap(),
        ReasonProfile::Declarative(_)
    ));
}

#[test]
fn built_in_ldap_modify_path_dsl_compiles_into_template_binding() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/ldap_modify_path.gewy").unwrap();

    assert_eq!(binding.template.id, "ldap_modify_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("ldap_modify".into())
    );
    assert!(matches!(
        binding.template.reason_profile.as_ref().unwrap(),
        ReasonProfile::Declarative(_)
    ));
}

#[test]
fn built_in_ldap_modify_denied_path_dsl_compiles_into_template_binding() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/ldap_modify_denied_path.gewy").unwrap();

    assert_eq!(binding.template.id, "ldap_modify_denied_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("ldap_modify_denied".into())
    );
    assert!(matches!(
        binding.template.reason_profile.as_ref().unwrap(),
        ReasonProfile::Declarative(_)
    ));
}

#[test]
fn built_in_ldap_modify_constraint_path_dsl_compiles_into_template_binding() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/ldap_modify_constraint_path.gewy")
            .unwrap();

    assert_eq!(binding.template.id, "ldap_modify_constraint_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("ldap_modify_constraint_violation".into())
    );
    assert!(matches!(
        binding.template.reason_profile.as_ref().unwrap(),
        ReasonProfile::Declarative(_)
    ));
}

#[test]
fn built_in_ldap_directory_session_dsl_compiles_into_template_binding() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/ldap_directory_session.gewy").unwrap();

    assert_eq!(binding.template.id, "ldap_directory_session");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("ldap_directory_session".into())
    );
    assert!(matches!(
        binding.template.reason_profile.as_ref().unwrap(),
        ReasonProfile::Declarative(_)
    ));
}

#[test]
fn built_in_ldap_directory_write_session_dsl_compiles_into_template_binding() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/ldap_directory_write_session.gewy")
            .unwrap();

    assert_eq!(binding.template.id, "ldap_directory_write_session");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("ldap_directory_write_session".into())
    );
    assert!(matches!(
        binding.template.reason_profile.as_ref().unwrap(),
        ReasonProfile::Declarative(_)
    ));
}

#[test]
fn built_in_ldap_directory_sync_session_dsl_compiles_into_template_binding() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/ldap_directory_sync_session.gewy")
            .unwrap();

    assert_eq!(binding.template.id, "ldap_directory_sync_session");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("ldap_directory_sync_session".into())
    );
    assert!(matches!(
        binding.template.reason_profile.as_ref().unwrap(),
        ReasonProfile::Declarative(_)
    ));
}

#[test]
fn built_in_snmp_get_path_dsl_compiles_into_template_binding() {
    let binding = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/snmp_get_path.gewy").unwrap();

    assert_eq!(binding.template.id, "snmp_get_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("snmp_get".into())
    );
    assert!(matches!(
        binding.template.reason_profile.as_ref().unwrap(),
        ReasonProfile::Declarative(_)
    ));
}

#[test]
fn built_in_dns_tcp_query_path_dsl_compiles_into_template_binding() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/dns_tcp_query_path.gewy").unwrap();

    assert_eq!(binding.template.id, "dns_tcp_query_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("dns_tcp_query".into())
    );
}

#[test]
fn udp_process_dsl_binding_drives_runtime_session() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy").unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 201, 4242, "curl"));
    session.ingest(udp_packet_fact(2, 201, 88));
    session.ingest(route_fact(3, 201, 5));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(40));

    let export = session.export_bundle();
    assert_eq!(export.template_id, "udp_process_debug");
    assert_eq!(export.program_flows.len(), 1);
    assert_eq!(export.flows[0].process.as_ref().unwrap().comm, "curl");
}

#[test]
fn dsl_supports_custom_predicates_and_fragment_params() {
    let binding = compile_str(
        r#"
template=udp_dns_debug
window=default_5s
reason=udp_datagram_l1
fragment=udp_packet_meta_fragment
fragment=route_meta_fragment
fragment=sock_lineage_fragment
program_model=dns_lookup_v1
operation=dns_lookup
rule=all(process_bound,datagram_observed:udp);datagram_observed;static:process-owned dns datagram;true
rule=any(route_resolved,socket_state_observed);route_resolved;static:upstream path or socket progress observed;true
param=sock_lineage_fragment.capture_comm=false
param=udp_packet_meta_fragment.min_len=80
"#,
    )
    .unwrap();

    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 202, 5353, "dig"));
    session.ingest(udp_packet_fact(2, 202, 72));
    session.ingest(route_fact(3, 202, 7));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(40));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("dns_lookup".into())
    );
    assert_eq!(export.flows[0].process.as_ref().unwrap().comm, "<redacted>");
    assert_eq!(export.rejected_facts.len(), 1);
    assert_eq!(
        export.rejected_fact_summary[0].reason,
        "filtered_by_fragment_param"
    );
}

#[test]
fn dns_dsl_uses_egress_direction_to_model_lookup_requests() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/dns_udp_process.gewy").unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 303, 5353, "dig"));
    session.ingest(route_fact(2, 303, 7));
    session.ingest(udp_packet_fact_with_dir(3, 303, 96, PacketDir::Egress));
    session.ingest(udp_packet_fact_with_dir(4, 303, 96, PacketDir::Ingress));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(50));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("dns_lookup".into())
    );
    assert!(
        export.program_flows[0]
            .narrative
            .iter()
            .any(|line| line == "program emitted a UDP datagram")
    );
    assert!(
        export.program_flows[0]
            .narrative
            .iter()
            .any(|line| line == "program received a UDP datagram")
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_reply"))
    );
    assert_eq!(export.module_findings.len(), 0);
    assert_eq!(
        export.reasons[0]
            .l1
            .key_events
            .iter()
            .filter(|event| event.kind == KeyEventKind::UdpDatagramSeen)
            .count(),
        2
    );
}

#[test]
fn dns_dsl_does_not_treat_ingress_udp_as_lookup_request() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/dns_udp_process.gewy").unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 304, 5353, "dig"));
    session.ingest(route_fact(2, 304, 7));
    session.ingest(udp_packet_fact_with_dir(3, 304, 96, PacketDir::Ingress));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(40));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .all(|stage| stage.phase.as_deref() != Some("send_request"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_reply"))
    );
    assert!(
        export.program_flows[0]
            .narrative
            .iter()
            .all(|line| line != "program emitted a UDP datagram")
    );
    assert!(export.program_findings.iter().any(|finding| {
        finding.phase.as_deref() == Some("send_request")
            && finding.phase_transition.as_deref() == Some("resolve->send_request")
    }));
}

#[test]
fn dns_dsl_missing_reply_produces_send_request_to_receive_reply_transition() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/dns_udp_process.gewy").unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 305, 5353, "dig"));
    session.ingest(route_fact(2, 305, 7));
    session.ingest(udp_packet_fact_with_dir(3, 305, 96, PacketDir::Egress));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(40));

    let export = session.export_bundle();
    assert!(export.program_findings.iter().any(|finding| {
        finding.phase.as_deref() == Some("receive_reply")
            && finding.phase_transition.as_deref() == Some("send_request->receive_reply")
    }));
    assert!(export.module_findings.iter().any(|finding| {
        finding
            .phase_transitions
            .contains(&"send_request->receive_reply".to_string())
    }));
}

#[test]
fn http_request_path_can_span_connect_and_request_response_phases_in_one_module() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/http_request_path.gewy").unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 601, 4242, "curl"));
    session.ingest(route_fact(2, 601, 7));
    session.ingest(tcp_state_fact_with_ports(3, 601, 1, 2, 42000, 443));
    session.ingest(tcp_state_fact_with_ports(4, 601, 2, 3, 42000, 443));
    session.ingest(packet_fact_with_dir(5, 601, 0x18, PacketDir::Egress));
    session.ingest(packet_fact_with_dir(6, 601, 0x18, PacketDir::Ingress));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(80));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("http_request".into())
    );
    let phases = export.program_flows[0]
        .stages
        .iter()
        .filter_map(|stage| stage.phase.clone())
        .collect::<Vec<_>>();
    assert!(phases.contains(&"bind".to_string()));
    assert!(phases.contains(&"resolve_upstream".to_string()));
    assert!(phases.contains(&"connect".to_string()));
    assert!(phases.contains(&"establish".to_string()));
    assert!(phases.contains(&"send_request".to_string()));
    assert!(phases.contains(&"receive_response".to_string()));
    let phase_kinds = export.program_flows[0]
        .stages
        .iter()
        .filter_map(|stage| stage.phase_kind.clone())
        .collect::<Vec<_>>();
    assert!(phase_kinds.contains(&"resolve_route".to_string()));
    assert!(phase_kinds.contains(&"initiate_connection".to_string()));
    assert!(phase_kinds.contains(&"establish_connection".to_string()));
    assert!(phase_kinds.contains(&"emit_payload".to_string()));
    assert!(phase_kinds.contains(&"receive_payload".to_string()));
    assert_eq!(export.module_findings.len(), 0);
}

#[test]
fn http_request_path_missing_establish_produces_connect_to_establish_transition() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/http_request_path.gewy").unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 602, 4242, "curl"));
    session.ingest(route_fact(2, 602, 7));
    session.ingest(tcp_state_fact_with_ports(3, 602, 1, 2, 42000, 443));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(50));

    let export = session.export_bundle();
    assert!(export.program_findings.iter().any(|finding| {
        finding.module_label == "http_request_path"
            && finding.phase.as_deref() == Some("establish")
            && finding.phase_transition.as_deref() == Some("connect->establish")
    }));
}

#[test]
fn http_request_path_missing_response_produces_request_to_response_transition() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/http_request_path.gewy").unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 603, 4242, "curl"));
    session.ingest(route_fact(2, 603, 7));
    session.ingest(tcp_state_fact_with_ports(3, 603, 1, 2, 42000, 443));
    session.ingest(tcp_state_fact_with_ports(4, 603, 2, 3, 42000, 443));
    session.ingest(packet_fact_with_dir(5, 603, 0x18, PacketDir::Egress));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(70));

    let export = session.export_bundle();
    assert!(export.program_findings.iter().any(|finding| {
        finding.module_label == "http_request_path"
            && finding.phase.as_deref() == Some("receive_response")
            && finding.phase_transition.as_deref() == Some("send_request->receive_response")
    }));
}

#[test]
fn http_server_response_path_can_span_accept_request_and_response_phases() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/http_server_response_path.gewy")
            .unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 604, 8080, "nginx"));
    session.ingest(tcp_state_fact_with_ports(2, 604, 1, 2, 80, 53000));
    session.ingest(tcp_state_fact_with_ports(3, 604, 2, 3, 80, 53000));
    session.ingest(packet_fact_with_dir(4, 604, 0x18, PacketDir::Ingress));
    session.ingest(packet_fact_with_dir(5, 604, 0x18, PacketDir::Egress));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(70));

    let export = session.export_bundle();
    let phases = export.program_flows[0]
        .stages
        .iter()
        .filter_map(|stage| stage.phase.clone())
        .collect::<Vec<_>>();
    assert!(phases.contains(&"accept".to_string()));
    assert!(phases.contains(&"establish".to_string()));
    assert!(phases.contains(&"receive_request".to_string()));
    assert!(phases.contains(&"send_response".to_string()));
    assert_eq!(export.module_findings.len(), 0);
}

#[test]
fn http_server_response_path_missing_response_produces_request_to_response_transition() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/http_server_response_path.gewy")
            .unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 605, 8080, "nginx"));
    session.ingest(tcp_state_fact_with_ports(2, 605, 1, 2, 80, 53000));
    session.ingest(tcp_state_fact_with_ports(3, 605, 2, 3, 80, 53000));
    session.ingest(packet_fact_with_dir(4, 605, 0x18, PacketDir::Ingress));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(60));

    let export = session.export_bundle();
    assert!(export.program_findings.iter().any(|finding| {
        finding.module_label == "http_server_response_path"
            && finding.phase.as_deref() == Some("send_response")
            && finding.phase_transition.as_deref() == Some("receive_request->send_response")
    }));
}

#[test]
fn tls_client_path_materializes_transport_packet_phase() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/tls_client_path.gewy").unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 801, 4242, "curl"));
    session.ingest(route_fact(2, 801, 7));
    session.ingest(tcp_state_fact_with_ports(3, 801, 1, 2, 42310, 443));
    session.ingest(tcp_state_fact_with_ports(4, 801, 2, 3, 42310, 443));
    session.ingest(packet_fact(5, 801, 0x18));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(60));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("tls_client".into())
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("send_client_hello"))
    );
    assert!(
        export.program_flows[0]
            .narrative
            .iter()
            .any(|line| line == "program sent transport payload on this network flow")
    );
    assert_eq!(export.module_findings.len(), 0);
}

#[test]
fn tls_client_path_missing_packet_phase_produces_establish_transition() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/tls_client_path.gewy").unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 802, 4242, "curl"));
    session.ingest(route_fact(2, 802, 7));
    session.ingest(tcp_state_fact_with_ports(3, 802, 1, 2, 42310, 443));
    session.ingest(tcp_state_fact_with_ports(4, 802, 2, 3, 42310, 443));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(50));

    let export = session.export_bundle();
    assert!(export.program_findings.iter().any(|finding| {
        finding.phase.as_deref() == Some("send_client_hello")
            && finding.phase_transition.as_deref() == Some("establish->send_client_hello")
    }));
}

#[test]
fn quic_client_initial_path_materializes_initial_and_handshake_datagrams() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/quic_client_initial_path.gewy")
            .unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 803, 4242, "curl"));
    session.ingest(route_fact(2, 803, 7));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        3,
        803,
        1280,
        PacketDir::Egress,
        Some(42310),
        Some(443),
        Some(0xc3),
        Some(0xc300),
    ));
    session.ingest(udp_packet_fact_with_dir_and_ports(
        4,
        803,
        220,
        PacketDir::Ingress,
        Some(42310),
        Some(443),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(60));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("quic_client_initial".into())
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("send_initial"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_handshake"))
    );
    let phase_kinds = export.program_flows[0]
        .stages
        .iter()
        .filter_map(|stage| stage.phase_kind.clone())
        .collect::<Vec<_>>();
    assert!(phase_kinds.contains(&"emit_datagram".to_string()));
    assert!(phase_kinds.contains(&"receive_datagram".to_string()));
    assert!(
        export.program_flows[0]
            .narrative
            .iter()
            .any(|line| line == "program emitted a UDP datagram")
    );
    assert!(
        export.program_flows[0]
            .narrative
            .iter()
            .any(|line| line == "program received a UDP datagram")
    );
    assert_eq!(export.module_findings.len(), 0);
}

#[test]
fn quic_client_initial_path_missing_handshake_produces_datagram_transition() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/quic_client_initial_path.gewy")
            .unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 804, 4242, "curl"));
    session.ingest(route_fact(2, 804, 7));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        3,
        804,
        1280,
        PacketDir::Egress,
        Some(42310),
        Some(443),
        Some(0xc1),
        Some(0xc300),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(50));

    let export = session.export_bundle();
    assert!(export.program_findings.iter().any(|finding| {
        finding.phase.as_deref() == Some("receive_handshake")
            && finding.phase_transition.as_deref() == Some("send_initial->receive_handshake")
            && finding.phase_transition_kind.as_deref() == Some("emit_datagram->receive_datagram")
    }));
}

#[test]
fn quic_client_initial_path_does_not_match_non_quic_udp_ports() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/quic_client_initial_path.gewy")
            .unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 805, 4242, "curl"));
    session.ingest(route_fact(2, 805, 7));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        3,
        805,
        1280,
        PacketDir::Egress,
        Some(42310),
        Some(53),
        Some(0xc0),
        Some(0xc300),
    ));
    session.ingest(udp_packet_fact_with_dir_and_ports(
        4,
        805,
        220,
        PacketDir::Ingress,
        Some(42310),
        Some(53),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(60));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .all(|stage| stage.phase.as_deref() != Some("send_initial"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .all(|stage| stage.phase.as_deref() != Some("receive_handshake"))
    );
}

#[test]
fn quic_client_initial_path_does_not_treat_small_quic_port_datagrams_as_initial() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/quic_client_initial_path.gewy")
            .unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 806, 4242, "curl"));
    session.ingest(route_fact(2, 806, 7));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        3,
        806,
        200,
        PacketDir::Egress,
        Some(42310),
        Some(443),
        Some(0xc0),
        Some(0xc300),
    ));
    session.ingest(udp_packet_fact_with_dir_and_ports(
        4,
        806,
        220,
        PacketDir::Ingress,
        Some(42310),
        Some(443),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(60));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .all(|stage| stage.phase.as_deref() != Some("send_initial"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_handshake"))
    );
}

#[test]
fn quic_client_initial_path_does_not_treat_wrong_first_byte_as_initial() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/quic_client_initial_path.gewy")
            .unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 807, 4242, "curl"));
    session.ingest(route_fact(2, 807, 7));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        3,
        807,
        1280,
        PacketDir::Egress,
        Some(42310),
        Some(443),
        Some(0x40),
        Some(0x4000),
    ));
    session.ingest(udp_packet_fact_with_dir_and_ports(
        4,
        807,
        220,
        PacketDir::Ingress,
        Some(42310),
        Some(443),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(60));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .all(|stage| stage.phase.as_deref() != Some("send_initial"))
    );
}

#[test]
fn quic_client_initial_path_does_not_treat_wrong_prefix2_as_initial() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/quic_client_initial_path.gewy")
            .unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 808, 4242, "curl"));
    session.ingest(route_fact(2, 808, 7));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        3,
        808,
        1280,
        PacketDir::Egress,
        Some(42310),
        Some(443),
        Some(0xc3),
        Some(0xc301),
    ));
    session.ingest(udp_packet_fact_with_dir_and_ports(
        4,
        808,
        220,
        PacketDir::Ingress,
        Some(42310),
        Some(443),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(60));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .all(|stage| stage.phase.as_deref() != Some("send_initial"))
    );
}

#[test]
fn stun_binding_path_materializes_request_and_response_datagrams() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/stun_binding_path.gewy").unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 809, 5000, "webrtc-app"));
    session.ingest(route_fact(2, 809, 7));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        3,
        809,
        120,
        PacketDir::Egress,
        Some(54000),
        Some(3478),
        Some(0x00),
        Some(0x0001),
    ));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        4,
        809,
        140,
        PacketDir::Ingress,
        Some(54000),
        Some(3478),
        Some(0x01),
        Some(0x0101),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(60));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("stun_binding".into())
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("send_request"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_response"))
    );
    let phase_kinds = export.program_flows[0]
        .stages
        .iter()
        .filter_map(|stage| stage.phase_kind.clone())
        .collect::<Vec<_>>();
    assert!(phase_kinds.contains(&"emit_datagram".to_string()));
    assert!(phase_kinds.contains(&"receive_datagram".to_string()));
    assert_eq!(export.module_findings.len(), 0);
}

#[test]
fn stun_binding_path_does_not_match_wrong_message_type() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/stun_binding_path.gewy").unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 810, 5000, "webrtc-app"));
    session.ingest(route_fact(2, 810, 7));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        3,
        810,
        120,
        PacketDir::Egress,
        Some(54000),
        Some(3478),
        Some(0x00),
        Some(0x0002),
    ));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        4,
        810,
        140,
        PacketDir::Ingress,
        Some(54000),
        Some(3478),
        Some(0x01),
        Some(0x0101),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(60));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .all(|stage| stage.phase.as_deref() != Some("send_request"))
    );
}

#[test]
fn coap_get_path_materializes_request_and_response_datagrams() {
    let binding = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/coap_get_path.gewy").unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 811, 6000, "coap-client"));
    session.ingest(route_fact(2, 811, 7));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        3,
        811,
        64,
        PacketDir::Egress,
        Some(56000),
        Some(5683),
        Some(0x40),
        Some(0x4001),
    ));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        4,
        811,
        80,
        PacketDir::Ingress,
        Some(56000),
        Some(5683),
        Some(0x60),
        Some(0x6045),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(60));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("coap_get".into())
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("send_request"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_response"))
    );
    let phase_kinds = export.program_flows[0]
        .stages
        .iter()
        .filter_map(|stage| stage.phase_kind.clone())
        .collect::<Vec<_>>();
    assert!(phase_kinds.contains(&"emit_datagram".to_string()));
    assert!(phase_kinds.contains(&"receive_datagram".to_string()));
    assert_eq!(export.module_findings.len(), 0);
}

#[test]
fn coap_get_path_does_not_match_wrong_response_code() {
    let binding = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/coap_get_path.gewy").unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 812, 6000, "coap-client"));
    session.ingest(route_fact(2, 812, 7));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        3,
        812,
        64,
        PacketDir::Egress,
        Some(56000),
        Some(5683),
        Some(0x40),
        Some(0x4001),
    ));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        4,
        812,
        80,
        PacketDir::Ingress,
        Some(56000),
        Some(5683),
        Some(0x60),
        Some(0x6050),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(60));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .all(|stage| stage.phase.as_deref() != Some("receive_response"))
    );
}

#[test]
fn ntp_client_path_materializes_request_and_response_datagrams() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/ntp_client_path.gewy").unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 813, 7000, "chrony-client"));
    session.ingest(route_fact(2, 813, 7));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        3,
        813,
        48,
        PacketDir::Egress,
        Some(53000),
        Some(123),
        Some(0x23),
        Some(0x2300),
    ));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        4,
        813,
        48,
        PacketDir::Ingress,
        Some(53000),
        Some(123),
        Some(0x24),
        Some(0x2400),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(60));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("ntp_client".into())
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("send_request"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_response"))
    );
    let phase_kinds = export.program_flows[0]
        .stages
        .iter()
        .filter_map(|stage| stage.phase_kind.clone())
        .collect::<Vec<_>>();
    assert!(phase_kinds.contains(&"emit_datagram".to_string()));
    assert!(phase_kinds.contains(&"receive_datagram".to_string()));
    assert_eq!(export.module_findings.len(), 0);
}

#[test]
fn ntp_client_path_does_not_match_wrong_response_mode() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/ntp_client_path.gewy").unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 814, 7000, "chrony-client"));
    session.ingest(route_fact(2, 814, 7));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        3,
        814,
        48,
        PacketDir::Egress,
        Some(53000),
        Some(123),
        Some(0x23),
        Some(0x2300),
    ));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        4,
        814,
        48,
        PacketDir::Ingress,
        Some(53000),
        Some(123),
        Some(0x25),
        Some(0x2500),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(60));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .all(|stage| stage.phase.as_deref() != Some("receive_response"))
    );
}

#[test]
fn gtpu_echo_path_materializes_request_and_response_datagrams() {
    let binding = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/gtpu_echo_path.gewy")
        .unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 813, 6001, "upf-agent"));
    session.ingest(route_fact(2, 813, 7));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        3,
        813,
        64,
        PacketDir::Egress,
        Some(2152),
        Some(2152),
        Some(0x30),
        Some(0x3001),
    ));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        4,
        813,
        64,
        PacketDir::Ingress,
        Some(2152),
        Some(2152),
        Some(0x30),
        Some(0x3002),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(60));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("gtpu_echo".into())
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("send_echo_request"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_echo_response"))
    );
    let phase_kinds = export.program_flows[0]
        .stages
        .iter()
        .filter_map(|stage| stage.phase_kind.clone())
        .collect::<Vec<_>>();
    assert!(phase_kinds.contains(&"emit_datagram".to_string()));
    assert!(phase_kinds.contains(&"receive_datagram".to_string()));
    assert_eq!(export.module_findings.len(), 0);
}

#[test]
fn gtpu_echo_path_does_not_match_wrong_response_type() {
    let binding = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/gtpu_echo_path.gewy")
        .unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 814, 6002, "upf-agent"));
    session.ingest(route_fact(2, 814, 7));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        3,
        814,
        64,
        PacketDir::Egress,
        Some(2152),
        Some(2152),
        Some(0x30),
        Some(0x3001),
    ));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        4,
        814,
        64,
        PacketDir::Ingress,
        Some(2152),
        Some(2152),
        Some(0x30),
        Some(0x3003),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(60));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .all(|stage| stage.phase.as_deref() != Some("receive_echo_response"))
    );
}

#[test]
fn dhcp_client_path_materializes_request_and_response_datagrams() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/dhcp_client_path.gewy").unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 815, 68, "dhclient"));
    session.ingest(route_fact(2, 815, 7));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        3,
        815,
        300,
        PacketDir::Egress,
        Some(68),
        Some(67),
        Some(0x01),
        Some(0x0101),
    ));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        4,
        815,
        300,
        PacketDir::Ingress,
        Some(68),
        Some(67),
        Some(0x02),
        Some(0x0201),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(60));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("dhcp_client".into())
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("send_discover"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_offer"))
    );
    let phase_kinds = export.program_flows[0]
        .stages
        .iter()
        .filter_map(|stage| stage.phase_kind.clone())
        .collect::<Vec<_>>();
    assert!(phase_kinds.contains(&"emit_datagram".to_string()));
    assert!(phase_kinds.contains(&"receive_datagram".to_string()));
    assert_eq!(export.module_findings.len(), 0);
}

#[test]
fn dhcp_client_path_does_not_match_wrong_reply_opcode() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/dhcp_client_path.gewy").unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 816, 68, "dhclient"));
    session.ingest(route_fact(2, 816, 7));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        3,
        816,
        300,
        PacketDir::Egress,
        Some(68),
        Some(67),
        Some(0x01),
        Some(0x0101),
    ));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        4,
        816,
        300,
        PacketDir::Ingress,
        Some(68),
        Some(67),
        Some(0x01),
        Some(0x0101),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(60));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .all(|stage| stage.phase.as_deref() != Some("receive_offer"))
    );
}

#[test]
fn wireguard_handshake_path_materializes_initiation_and_response_datagrams() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/wireguard_handshake_path.gewy")
            .unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 817, 53000, "wg-quick"));
    session.ingest(route_fact(2, 817, 7));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        3,
        817,
        148,
        PacketDir::Egress,
        Some(53000),
        Some(51820),
        Some(0x01),
        Some(0x0100),
    ));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        4,
        817,
        92,
        PacketDir::Ingress,
        Some(53000),
        Some(51820),
        Some(0x02),
        Some(0x0200),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(60));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("wireguard_handshake".into())
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("send_initiation"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_response"))
    );
    let phase_kinds = export.program_flows[0]
        .stages
        .iter()
        .filter_map(|stage| stage.phase_kind.clone())
        .collect::<Vec<_>>();
    assert!(phase_kinds.contains(&"emit_datagram".to_string()));
    assert!(phase_kinds.contains(&"receive_datagram".to_string()));
    assert_eq!(export.module_findings.len(), 0);
}

#[test]
fn wireguard_handshake_path_does_not_match_wrong_response_type() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/wireguard_handshake_path.gewy")
            .unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 818, 53000, "wg-quick"));
    session.ingest(route_fact(2, 818, 7));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        3,
        818,
        148,
        PacketDir::Egress,
        Some(53000),
        Some(51820),
        Some(0x01),
        Some(0x0100),
    ));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        4,
        818,
        64,
        PacketDir::Ingress,
        Some(53000),
        Some(51820),
        Some(0x04),
        Some(0x0400),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(60));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .all(|stage| stage.phase.as_deref() != Some("receive_response"))
    );
}

#[test]
fn mdns_query_path_materializes_query_and_response_datagrams() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/mdns_query_path.gewy").unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 819, 5353, "avahi-daemon"));
    session.ingest(route_fact(2, 819, 7));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload_prefix4(
        3,
        819,
        64,
        PacketDir::Egress,
        Some(5353),
        Some(5353),
        Some(0x00),
        Some(0x0000),
        Some(0x00000000),
    ));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload_prefix4(
        4,
        819,
        96,
        PacketDir::Ingress,
        Some(5353),
        Some(5353),
        Some(0x00),
        Some(0x0000),
        Some(0x00008400),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(60));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("mdns_query".into())
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("send_query"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_response"))
    );
    let phase_kinds = export.program_flows[0]
        .stages
        .iter()
        .filter_map(|stage| stage.phase_kind.clone())
        .collect::<Vec<_>>();
    assert!(phase_kinds.contains(&"emit_datagram".to_string()));
    assert!(phase_kinds.contains(&"receive_datagram".to_string()));
    assert_eq!(export.module_findings.len(), 0);
}

#[test]
fn mdns_query_path_does_not_match_wrong_response_flags() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/mdns_query_path.gewy").unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 820, 5353, "avahi-daemon"));
    session.ingest(route_fact(2, 820, 7));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload_prefix4(
        3,
        820,
        64,
        PacketDir::Egress,
        Some(5353),
        Some(5353),
        Some(0x00),
        Some(0x0000),
        Some(0x00000000),
    ));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload_prefix4(
        4,
        820,
        96,
        PacketDir::Ingress,
        Some(5353),
        Some(5353),
        Some(0x00),
        Some(0x0000),
        Some(0x00000400),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(60));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .all(|stage| stage.phase.as_deref() != Some("receive_response"))
    );
}

#[test]
fn ssdp_discovery_path_materializes_search_and_response_datagrams() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/ssdp_discovery_path.gewy").unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 821, 1900, "ssdp-client"));
    session.ingest(route_fact(2, 821, 7));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload_prefix4(
        3,
        821,
        96,
        PacketDir::Egress,
        Some(1900),
        Some(1900),
        Some(0x4d),
        Some(0x4d2d),
        Some(0x4d2d5345),
    ));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload_prefix4(
        4,
        821,
        180,
        PacketDir::Ingress,
        Some(1900),
        Some(1900),
        Some(0x48),
        Some(0x4854),
        Some(0x48545450),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(60));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("ssdp_discovery".into())
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("send_search"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_response"))
    );
    let phase_kinds = export.program_flows[0]
        .stages
        .iter()
        .filter_map(|stage| stage.phase_kind.clone())
        .collect::<Vec<_>>();
    assert!(phase_kinds.contains(&"emit_datagram".to_string()));
    assert!(phase_kinds.contains(&"receive_datagram".to_string()));
    assert_eq!(export.module_findings.len(), 0);
}

#[test]
fn ssdp_discovery_path_does_not_match_wrong_response_prefix() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/ssdp_discovery_path.gewy").unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 822, 1900, "ssdp-client"));
    session.ingest(route_fact(2, 822, 7));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload_prefix4(
        3,
        822,
        96,
        PacketDir::Egress,
        Some(1900),
        Some(1900),
        Some(0x4d),
        Some(0x4d2d),
        Some(0x4d2d5345),
    ));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload_prefix4(
        4,
        822,
        180,
        PacketDir::Ingress,
        Some(1900),
        Some(1900),
        Some(0x4e),
        Some(0x4e4f),
        Some(0x4e4f5459),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(60));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .all(|stage| stage.phase.as_deref() != Some("receive_response"))
    );
}

#[test]
fn redis_ping_path_materializes_request_and_response_payload_phases() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/redis_ping_path.gewy").unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 823, 53001, "redis-cli"));
    session.ingest(route_fact(2, 823, 7));
    session.ingest(packet_fact_with_dir_and_payload(
        3,
        823,
        0x18,
        PacketDir::Egress,
        Some(53001),
        Some(6379),
        Some(0x2a),
        Some(0x2a31),
        Some(0x2a310d0a),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        4,
        823,
        0x18,
        PacketDir::Ingress,
        Some(53001),
        Some(6379),
        Some(0x2b),
        Some(0x2b50),
        Some(0x2b504f4e),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(60));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("redis_ping".into())
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("send_ping"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_pong"))
    );
    let phase_kinds = export.program_flows[0]
        .stages
        .iter()
        .filter_map(|stage| stage.phase_kind.clone())
        .collect::<Vec<_>>();
    assert!(phase_kinds.contains(&"emit_payload".to_string()));
    assert!(phase_kinds.contains(&"receive_payload".to_string()));
    assert_eq!(export.module_findings.len(), 0);
}

#[test]
fn redis_ping_path_does_not_match_wrong_response_prefix() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/redis_ping_path.gewy").unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 824, 53001, "redis-cli"));
    session.ingest(route_fact(2, 824, 7));
    session.ingest(packet_fact_with_dir_and_payload(
        3,
        824,
        0x18,
        PacketDir::Egress,
        Some(53001),
        Some(6379),
        Some(0x2a),
        Some(0x2a31),
        Some(0x2a310d0a),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        4,
        824,
        0x18,
        PacketDir::Ingress,
        Some(53001),
        Some(6379),
        Some(0x2d),
        Some(0x2d45),
        Some(0x2d455252),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(60));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .all(|stage| stage.phase.as_deref() != Some("receive_pong"))
    );
}

#[test]
fn mqtt_connect_path_materializes_connect_and_connack_payload_phases() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/mqtt_connect_path.gewy").unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 824, 53002, "mosquitto-pub"));
    session.ingest(route_fact(2, 824, 7));
    session.ingest(packet_fact_with_dir_and_payload(
        3,
        824,
        0x18,
        PacketDir::Egress,
        Some(53002),
        Some(1883),
        Some(0x10),
        Some(0x1016),
        Some(0x10160004),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        4,
        824,
        0x18,
        PacketDir::Ingress,
        Some(53002),
        Some(1883),
        Some(0x20),
        Some(0x2002),
        Some(0x20020000),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(60));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("mqtt_connect".into())
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("send_connect"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_connack"))
    );
    let phase_kinds = export.program_flows[0]
        .stages
        .iter()
        .filter_map(|stage| stage.phase_kind.clone())
        .collect::<Vec<_>>();
    assert!(phase_kinds.contains(&"emit_payload".to_string()));
    assert!(phase_kinds.contains(&"receive_payload".to_string()));
    assert_eq!(export.module_findings.len(), 0);
}

#[test]
fn radius_access_path_materializes_request_and_accept_datagrams() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/radius_access_path.gewy").unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 825, 53000, "wpa_supplicant"));
    session.ingest(route_fact(2, 825, 7));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        3,
        825,
        96,
        PacketDir::Egress,
        Some(53000),
        Some(1812),
        Some(0x01),
        None,
    ));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        4,
        825,
        96,
        PacketDir::Ingress,
        Some(53000),
        Some(1812),
        Some(0x02),
        None,
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(60));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("radius_access".into())
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("send_access_request"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_access_accept"))
    );
    let phase_kinds = export.program_flows[0]
        .stages
        .iter()
        .filter_map(|stage| stage.phase_kind.clone())
        .collect::<Vec<_>>();
    assert!(phase_kinds.contains(&"emit_datagram".to_string()));
    assert!(phase_kinds.contains(&"receive_datagram".to_string()));
    assert_eq!(export.module_findings.len(), 0);
}

#[test]
fn radius_access_path_does_not_match_wrong_response_code() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/radius_access_path.gewy").unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 826, 53000, "wpa_supplicant"));
    session.ingest(route_fact(2, 826, 7));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        3,
        826,
        96,
        PacketDir::Egress,
        Some(53000),
        Some(1812),
        Some(0x01),
        None,
    ));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        4,
        826,
        96,
        PacketDir::Ingress,
        Some(53000),
        Some(1812),
        Some(0x03),
        None,
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(60));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .all(|stage| stage.phase.as_deref() != Some("receive_access_accept"))
    );
}

#[test]
fn smtp_session_path_materializes_connect_banner_and_ehlo_phases() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/smtp_session_path.gewy").unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 827, 53010, "postfix-client"));
    session.ingest(route_fact(2, 827, 7));
    session.ingest(tcp_state_fact_with_ports(3, 827, 1, 2, 53010, 25));
    session.ingest(packet_fact_with_dir_and_payload(
        4,
        827,
        0x18,
        PacketDir::Ingress,
        Some(53010),
        Some(25),
        Some(0x32),
        Some(0x3232),
        Some(0x32323020),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        5,
        827,
        0x18,
        PacketDir::Egress,
        Some(53010),
        Some(25),
        Some(0x45),
        Some(0x4548),
        Some(0x45484c4f),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(70));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("smtp_session".into())
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("connect"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_banner"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("send_ehlo"))
    );
    let phase_kinds = export.program_flows[0]
        .stages
        .iter()
        .filter_map(|stage| stage.phase_kind.clone())
        .collect::<Vec<_>>();
    assert!(phase_kinds.contains(&"initiate_connection".to_string()));
    assert!(phase_kinds.contains(&"receive_payload".to_string()));
    assert!(phase_kinds.contains(&"emit_payload".to_string()));
    assert_eq!(export.module_findings.len(), 0);
}

#[test]
fn smtp_session_path_does_not_match_wrong_banner_prefix() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/smtp_session_path.gewy").unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 828, 53010, "postfix-client"));
    session.ingest(route_fact(2, 828, 7));
    session.ingest(tcp_state_fact_with_ports(3, 828, 1, 2, 53010, 25));
    session.ingest(packet_fact_with_dir_and_payload(
        4,
        828,
        0x18,
        PacketDir::Ingress,
        Some(53010),
        Some(25),
        Some(0x35),
        Some(0x3535),
        Some(0x35353020),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        5,
        828,
        0x18,
        PacketDir::Egress,
        Some(53010),
        Some(25),
        Some(0x45),
        Some(0x4548),
        Some(0x45484c4f),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(70));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .all(|stage| stage.phase.as_deref() != Some("receive_banner"))
    );
}

#[test]
fn sip_register_path_materializes_register_and_ok_datagrams() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/sip_register_path.gewy").unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 829, 54010, "sip-client"));
    session.ingest(route_fact(2, 829, 7));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload_prefix4(
        3,
        829,
        180,
        PacketDir::Egress,
        Some(54010),
        Some(5060),
        Some(0x52),
        Some(0x5245),
        Some(0x52454749),
    ));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload_prefix4(
        4,
        829,
        220,
        PacketDir::Ingress,
        Some(54010),
        Some(5060),
        Some(0x53),
        Some(0x5349),
        Some(0x5349502f),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(60));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("sip_register".into())
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("send_register"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_ok"))
    );
    let phase_kinds = export.program_flows[0]
        .stages
        .iter()
        .filter_map(|stage| stage.phase_kind.clone())
        .collect::<Vec<_>>();
    assert!(phase_kinds.contains(&"emit_datagram".to_string()));
    assert!(phase_kinds.contains(&"receive_datagram".to_string()));
    assert_eq!(export.module_findings.len(), 0);
}

#[test]
fn sip_register_path_does_not_match_wrong_response_prefix() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/sip_register_path.gewy").unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 830, 54010, "sip-client"));
    session.ingest(route_fact(2, 830, 7));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload_prefix4(
        3,
        830,
        180,
        PacketDir::Egress,
        Some(54010),
        Some(5060),
        Some(0x52),
        Some(0x5245),
        Some(0x52454749),
    ));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload_prefix4(
        4,
        830,
        220,
        PacketDir::Ingress,
        Some(54010),
        Some(5060),
        Some(0x52),
        Some(0x5245),
        Some(0x52455350),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(60));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .all(|stage| stage.phase.as_deref() != Some("receive_ok"))
    );
}

#[test]
fn ldap_bind_path_materializes_connect_bind_and_response_phases() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/ldap_bind_path.gewy").unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 831, 54020, "ldap-client"));
    session.ingest(route_fact(2, 831, 7));
    session.ingest(tcp_state_fact_with_ports(3, 831, 1, 2, 54020, 389));
    session.ingest(tcp_state_fact_with_ports(4, 831, 2, 3, 54020, 389));
    session.ingest(packet_fact_with_dir_and_payload_and_bytes4_and5(
        5,
        831,
        0x18,
        PacketDir::Egress,
        Some(54020),
        Some(389),
        Some(0x30),
        Some(0x300c),
        Some(0x300c0201),
        Some(0x01),
        Some(0x60),
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_bytes4_and5(
        6,
        831,
        0x18,
        PacketDir::Ingress,
        Some(54020),
        Some(389),
        Some(0x30),
        Some(0x300c),
        Some(0x300c0201),
        Some(0x01),
        Some(0x61),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(80));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("ldap_bind".into())
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("connect"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("establish"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("send_bind"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_bind_response"))
    );
    let phase_kinds = export.program_flows[0]
        .stages
        .iter()
        .filter_map(|stage| stage.phase_kind.clone())
        .collect::<Vec<_>>();
    assert!(phase_kinds.contains(&"initiate_connection".to_string()));
    assert!(phase_kinds.contains(&"establish_connection".to_string()));
    assert!(phase_kinds.contains(&"emit_payload".to_string()));
    assert!(phase_kinds.contains(&"receive_payload".to_string()));
    assert_eq!(export.module_findings.len(), 0);
}

#[test]
fn ldap_bind_path_does_not_match_wrong_response_op_tag() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/ldap_bind_path.gewy").unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 832, 54020, "ldap-client"));
    session.ingest(route_fact(2, 832, 7));
    session.ingest(tcp_state_fact_with_ports(3, 832, 1, 2, 54020, 389));
    session.ingest(tcp_state_fact_with_ports(4, 832, 2, 3, 54020, 389));
    session.ingest(packet_fact_with_dir_and_payload_and_bytes4_and5(
        5,
        832,
        0x18,
        PacketDir::Egress,
        Some(54020),
        Some(389),
        Some(0x30),
        Some(0x300c),
        Some(0x300c0201),
        Some(0x01),
        Some(0x60),
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_bytes4_and5(
        6,
        832,
        0x18,
        PacketDir::Ingress,
        Some(54020),
        Some(389),
        Some(0x30),
        Some(0x300c),
        Some(0x300c0201),
        Some(0x01),
        Some(0x64),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(80));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .all(|stage| stage.phase.as_deref() != Some("receive_bind_response"))
    );
}

#[test]
fn ldap_search_path_materializes_connect_search_and_result_phases() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/ldap_search_path.gewy").unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 833, 54021, "ldapsearch"));
    session.ingest(route_fact(2, 833, 7));
    session.ingest(tcp_state_fact_with_ports(3, 833, 1, 2, 54021, 389));
    session.ingest(tcp_state_fact_with_ports(4, 833, 2, 3, 54021, 389));
    session.ingest(packet_fact_with_dir_and_payload_and_bytes4_and5(
        5,
        833,
        0x18,
        PacketDir::Egress,
        Some(54021),
        Some(389),
        Some(0x30),
        Some(0x3010),
        Some(0x30100201),
        Some(0x01),
        Some(0x63),
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_bytes4_and5(
        6,
        833,
        0x18,
        PacketDir::Ingress,
        Some(54021),
        Some(389),
        Some(0x30),
        Some(0x300c),
        Some(0x300c0201),
        Some(0x01),
        Some(0x65),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(80));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("ldap_search".into())
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("connect"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("establish"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("send_search"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_search_result"))
    );
    let phase_kinds = export.program_flows[0]
        .stages
        .iter()
        .filter_map(|stage| stage.phase_kind.clone())
        .collect::<Vec<_>>();
    assert!(phase_kinds.contains(&"initiate_connection".to_string()));
    assert!(phase_kinds.contains(&"establish_connection".to_string()));
    assert!(phase_kinds.contains(&"emit_payload".to_string()));
    assert!(phase_kinds.contains(&"receive_payload".to_string()));
    assert_eq!(export.module_findings.len(), 0);
}

#[test]
fn ldap_search_path_does_not_match_wrong_response_op_tag() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/ldap_search_path.gewy").unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 834, 54021, "ldapsearch"));
    session.ingest(route_fact(2, 834, 7));
    session.ingest(tcp_state_fact_with_ports(3, 834, 1, 2, 54021, 389));
    session.ingest(tcp_state_fact_with_ports(4, 834, 2, 3, 54021, 389));
    session.ingest(packet_fact_with_dir_and_payload_and_bytes4_and5(
        5,
        834,
        0x18,
        PacketDir::Egress,
        Some(54021),
        Some(389),
        Some(0x30),
        Some(0x3010),
        Some(0x30100201),
        Some(0x01),
        Some(0x63),
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_bytes4_and5(
        6,
        834,
        0x18,
        PacketDir::Ingress,
        Some(54021),
        Some(389),
        Some(0x30),
        Some(0x300c),
        Some(0x300c0201),
        Some(0x01),
        Some(0x64),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(80));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .all(|stage| stage.phase.as_deref() != Some("receive_search_result"))
    );
}

#[test]
fn ldap_modify_path_materializes_connect_modify_and_response_phases() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/ldap_modify_path.gewy").unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 836, 54023, "ldapmodify"));
    session.ingest(route_fact(2, 836, 7));
    session.ingest(tcp_state_fact_with_ports(3, 836, 1, 2, 54023, 389));
    session.ingest(tcp_state_fact_with_ports(4, 836, 2, 3, 54023, 389));
    session.ingest(packet_fact_with_dir_and_payload_and_bytes4_5_and9(
        5,
        836,
        0x18,
        PacketDir::Egress,
        Some(54023),
        Some(389),
        Some(0x30),
        Some(0x3012),
        Some(0x30120201),
        Some(0x01),
        Some(0x66),
        None,
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_bytes4_5_and9(
        6,
        836,
        0x18,
        PacketDir::Ingress,
        Some(54023),
        Some(389),
        Some(0x30),
        Some(0x300c),
        Some(0x300c0201),
        Some(0x01),
        Some(0x67),
        Some(0x00),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(80));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("ldap_modify".into())
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("connect"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("establish"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("send_modify"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_modify_response"))
    );
    let phase_kinds = export.program_flows[0]
        .stages
        .iter()
        .filter_map(|stage| stage.phase_kind.clone())
        .collect::<Vec<_>>();
    assert!(phase_kinds.contains(&"initiate_connection".to_string()));
    assert!(phase_kinds.contains(&"establish_connection".to_string()));
    assert!(phase_kinds.contains(&"emit_payload".to_string()));
    assert!(phase_kinds.contains(&"receive_payload".to_string()));
    assert_eq!(export.module_findings.len(), 0);
}

#[test]
fn ldap_modify_path_does_not_match_wrong_response_op_tag() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/ldap_modify_path.gewy").unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 837, 54023, "ldapmodify"));
    session.ingest(route_fact(2, 837, 7));
    session.ingest(tcp_state_fact_with_ports(3, 837, 1, 2, 54023, 389));
    session.ingest(tcp_state_fact_with_ports(4, 837, 2, 3, 54023, 389));
    session.ingest(packet_fact_with_dir_and_payload_and_bytes4_5_and9(
        5,
        837,
        0x18,
        PacketDir::Egress,
        Some(54023),
        Some(389),
        Some(0x30),
        Some(0x3012),
        Some(0x30120201),
        Some(0x01),
        Some(0x66),
        None,
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_bytes4_5_and9(
        6,
        837,
        0x18,
        PacketDir::Ingress,
        Some(54023),
        Some(389),
        Some(0x30),
        Some(0x300c),
        Some(0x300c0201),
        Some(0x01),
        Some(0x67),
        Some(0x31),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(80));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .all(|stage| stage.phase.as_deref() != Some("receive_modify_response"))
    );
}

#[test]
fn ldap_modify_denied_path_materializes_denied_modify_phase() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/ldap_modify_denied_path.gewy").unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 842, 54028, "ldapmodify"));
    session.ingest(route_fact(2, 842, 7));
    session.ingest(tcp_state_fact_with_ports(3, 842, 1, 2, 54028, 389));
    session.ingest(tcp_state_fact_with_ports(4, 842, 2, 3, 54028, 389));
    session.ingest(packet_fact_with_dir_and_payload_and_bytes4_5_and9(
        5,
        842,
        0x18,
        PacketDir::Egress,
        Some(54028),
        Some(389),
        Some(0x30),
        Some(0x3012),
        Some(0x30120201),
        Some(0x01),
        Some(0x66),
        None,
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_bytes4_5_and9(
        6,
        842,
        0x18,
        PacketDir::Ingress,
        Some(54028),
        Some(389),
        Some(0x30),
        Some(0x300c),
        Some(0x300c0201),
        Some(0x01),
        Some(0x67),
        Some(0x32),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(80));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("ldap_modify_denied".into())
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("send_modify"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_modify_denied"))
    );
    let phase_kinds = export.program_flows[0]
        .stages
        .iter()
        .filter_map(|stage| stage.phase_kind.clone())
        .collect::<Vec<_>>();
    assert!(phase_kinds.contains(&"emit_payload".to_string()));
    assert!(phase_kinds.contains(&"receive_payload".to_string()));
    assert_eq!(export.module_findings.len(), 0);
}

#[test]
fn ldap_modify_denied_path_does_not_match_success_result_code() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/ldap_modify_denied_path.gewy").unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 843, 54028, "ldapmodify"));
    session.ingest(route_fact(2, 843, 7));
    session.ingest(tcp_state_fact_with_ports(3, 843, 1, 2, 54028, 389));
    session.ingest(tcp_state_fact_with_ports(4, 843, 2, 3, 54028, 389));
    session.ingest(packet_fact_with_dir_and_payload_and_bytes4_5_and9(
        5,
        843,
        0x18,
        PacketDir::Egress,
        Some(54028),
        Some(389),
        Some(0x30),
        Some(0x3012),
        Some(0x30120201),
        Some(0x01),
        Some(0x66),
        None,
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_bytes4_5_and9(
        6,
        843,
        0x18,
        PacketDir::Ingress,
        Some(54028),
        Some(389),
        Some(0x30),
        Some(0x300c),
        Some(0x300c0201),
        Some(0x01),
        Some(0x67),
        Some(0x00),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(80));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .all(|stage| stage.phase.as_deref() != Some("receive_modify_denied"))
    );
}

#[test]
fn ldap_modify_constraint_path_materializes_constraint_violation_phase() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/ldap_modify_constraint_path.gewy")
            .unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 844, 54029, "ldapmodify"));
    session.ingest(route_fact(2, 844, 7));
    session.ingest(tcp_state_fact_with_ports(3, 844, 1, 2, 54029, 389));
    session.ingest(tcp_state_fact_with_ports(4, 844, 2, 3, 54029, 389));
    session.ingest(packet_fact_with_dir_and_payload_and_bytes4_5_and9(
        5,
        844,
        0x18,
        PacketDir::Egress,
        Some(54029),
        Some(389),
        Some(0x30),
        Some(0x3012),
        Some(0x30120201),
        Some(0x01),
        Some(0x66),
        None,
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_bytes4_5_and9(
        6,
        844,
        0x18,
        PacketDir::Ingress,
        Some(54029),
        Some(389),
        Some(0x30),
        Some(0x300c),
        Some(0x300c0201),
        Some(0x01),
        Some(0x67),
        Some(0x13),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(80));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("ldap_modify_constraint_violation".into())
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("send_modify"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| { stage.phase.as_deref() == Some("receive_modify_constraint_violation") })
    );
    let phase_kinds = export.program_flows[0]
        .stages
        .iter()
        .filter_map(|stage| stage.phase_kind.clone())
        .collect::<Vec<_>>();
    assert!(phase_kinds.contains(&"emit_payload".to_string()));
    assert!(phase_kinds.contains(&"receive_payload".to_string()));
    assert_eq!(export.module_findings.len(), 0);
}

#[test]
fn ldap_modify_constraint_path_does_not_match_access_denied_result_code() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/ldap_modify_constraint_path.gewy")
            .unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 845, 54029, "ldapmodify"));
    session.ingest(route_fact(2, 845, 7));
    session.ingest(tcp_state_fact_with_ports(3, 845, 1, 2, 54029, 389));
    session.ingest(tcp_state_fact_with_ports(4, 845, 2, 3, 54029, 389));
    session.ingest(packet_fact_with_dir_and_payload_and_bytes4_5_and9(
        5,
        845,
        0x18,
        PacketDir::Egress,
        Some(54029),
        Some(389),
        Some(0x30),
        Some(0x3012),
        Some(0x30120201),
        Some(0x01),
        Some(0x66),
        None,
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_bytes4_5_and9(
        6,
        845,
        0x18,
        PacketDir::Ingress,
        Some(54029),
        Some(389),
        Some(0x30),
        Some(0x300c),
        Some(0x300c0201),
        Some(0x01),
        Some(0x67),
        Some(0x32),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(80));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .all(|stage| { stage.phase.as_deref() != Some("receive_modify_constraint_violation") })
    );
}

#[test]
fn ldap_directory_session_can_span_bind_and_search_in_one_module() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/ldap_directory_session.gewy").unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 835, 54022, "ldap-directory-client"));
    session.ingest(route_fact(2, 835, 7));
    session.ingest(tcp_state_fact_with_ports(3, 835, 1, 2, 54022, 389));
    session.ingest(tcp_state_fact_with_ports(4, 835, 2, 3, 54022, 389));
    session.ingest(packet_fact_with_dir_and_payload_and_bytes4_and5(
        5,
        835,
        0x18,
        PacketDir::Egress,
        Some(54022),
        Some(389),
        Some(0x30),
        Some(0x300c),
        Some(0x300c0201),
        Some(0x01),
        Some(0x60),
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_bytes4_and5(
        6,
        835,
        0x18,
        PacketDir::Ingress,
        Some(54022),
        Some(389),
        Some(0x30),
        Some(0x300c),
        Some(0x300c0201),
        Some(0x01),
        Some(0x61),
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_bytes4_and5(
        7,
        835,
        0x18,
        PacketDir::Egress,
        Some(54022),
        Some(389),
        Some(0x30),
        Some(0x3010),
        Some(0x30100201),
        Some(0x01),
        Some(0x63),
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_bytes4_and5(
        8,
        835,
        0x18,
        PacketDir::Ingress,
        Some(54022),
        Some(389),
        Some(0x30),
        Some(0x300c),
        Some(0x300c0201),
        Some(0x01),
        Some(0x65),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(100));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("ldap_directory_session".into())
    );
    let phases = export.program_flows[0]
        .stages
        .iter()
        .filter_map(|stage| stage.phase.clone())
        .collect::<Vec<_>>();
    assert!(phases.contains(&"connect".to_string()));
    assert!(phases.contains(&"establish".to_string()));
    assert!(phases.contains(&"send_bind".to_string()));
    assert!(phases.contains(&"receive_bind_response".to_string()));
    assert!(phases.contains(&"send_search".to_string()));
    assert!(phases.contains(&"receive_search_result".to_string()));
    let phase_kinds = export.program_flows[0]
        .stages
        .iter()
        .filter_map(|stage| stage.phase_kind.clone())
        .collect::<Vec<_>>();
    assert!(phase_kinds.contains(&"initiate_connection".to_string()));
    assert!(phase_kinds.contains(&"establish_connection".to_string()));
    assert!(phase_kinds.contains(&"emit_payload".to_string()));
    assert!(phase_kinds.contains(&"receive_payload".to_string()));
    assert_eq!(export.module_findings.len(), 0);
}

#[test]
fn ldap_directory_write_session_can_span_bind_and_modify_in_one_module() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/ldap_directory_write_session.gewy")
            .unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 838, 54024, "ldap-directory-writer"));
    session.ingest(route_fact(2, 838, 7));
    session.ingest(tcp_state_fact_with_ports(3, 838, 1, 2, 54024, 389));
    session.ingest(tcp_state_fact_with_ports(4, 838, 2, 3, 54024, 389));
    session.ingest(packet_fact_with_dir_and_payload_and_bytes4_and5(
        5,
        838,
        0x18,
        PacketDir::Egress,
        Some(54024),
        Some(389),
        Some(0x30),
        Some(0x300c),
        Some(0x300c0201),
        Some(0x01),
        Some(0x60),
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_bytes4_and5(
        6,
        838,
        0x18,
        PacketDir::Ingress,
        Some(54024),
        Some(389),
        Some(0x30),
        Some(0x300c),
        Some(0x300c0201),
        Some(0x01),
        Some(0x61),
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_bytes4_and5(
        7,
        838,
        0x18,
        PacketDir::Egress,
        Some(54024),
        Some(389),
        Some(0x30),
        Some(0x3012),
        Some(0x30120201),
        Some(0x01),
        Some(0x66),
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_bytes4_5_and9(
        8,
        838,
        0x18,
        PacketDir::Ingress,
        Some(54024),
        Some(389),
        Some(0x30),
        Some(0x300c),
        Some(0x300c0201),
        Some(0x01),
        Some(0x67),
        Some(0x00),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(100));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("ldap_directory_write_session".into())
    );
    let phases = export.program_flows[0]
        .stages
        .iter()
        .filter_map(|stage| stage.phase.clone())
        .collect::<Vec<_>>();
    assert!(phases.contains(&"connect".to_string()));
    assert!(phases.contains(&"establish".to_string()));
    assert!(phases.contains(&"send_bind".to_string()));
    assert!(phases.contains(&"receive_bind_response".to_string()));
    assert!(phases.contains(&"send_modify".to_string()));
    assert!(phases.contains(&"receive_modify_response".to_string()));
    let phase_kinds = export.program_flows[0]
        .stages
        .iter()
        .filter_map(|stage| stage.phase_kind.clone())
        .collect::<Vec<_>>();
    assert!(phase_kinds.contains(&"initiate_connection".to_string()));
    assert!(phase_kinds.contains(&"establish_connection".to_string()));
    assert!(phase_kinds.contains(&"emit_payload".to_string()));
    assert!(phase_kinds.contains(&"receive_payload".to_string()));
    assert_eq!(export.module_findings.len(), 0);
}

#[test]
fn ldap_directory_sync_session_can_span_bind_search_and_modify_in_one_module() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/ldap_directory_sync_session.gewy")
            .unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 839, 54025, "ldap-directory-sync"));
    session.ingest(route_fact(2, 839, 7));
    session.ingest(tcp_state_fact_with_ports(3, 839, 1, 2, 54025, 389));
    session.ingest(tcp_state_fact_with_ports(4, 839, 2, 3, 54025, 389));
    session.ingest(packet_fact_with_dir_and_payload_and_bytes4_and5(
        5,
        839,
        0x18,
        PacketDir::Egress,
        Some(54025),
        Some(389),
        Some(0x30),
        Some(0x300c),
        Some(0x300c0201),
        Some(0x01),
        Some(0x60),
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_bytes4_and5(
        6,
        839,
        0x18,
        PacketDir::Ingress,
        Some(54025),
        Some(389),
        Some(0x30),
        Some(0x300c),
        Some(0x300c0201),
        Some(0x01),
        Some(0x61),
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_bytes4_and5(
        7,
        839,
        0x18,
        PacketDir::Egress,
        Some(54025),
        Some(389),
        Some(0x30),
        Some(0x3010),
        Some(0x30100201),
        Some(0x01),
        Some(0x63),
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_bytes4_and5(
        8,
        839,
        0x18,
        PacketDir::Ingress,
        Some(54025),
        Some(389),
        Some(0x30),
        Some(0x300c),
        Some(0x300c0201),
        Some(0x01),
        Some(0x65),
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_bytes4_5_and9(
        9,
        839,
        0x18,
        PacketDir::Egress,
        Some(54025),
        Some(389),
        Some(0x30),
        Some(0x3012),
        Some(0x30120201),
        Some(0x01),
        Some(0x66),
        None,
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_bytes4_5_and9(
        10,
        839,
        0x18,
        PacketDir::Ingress,
        Some(54025),
        Some(389),
        Some(0x30),
        Some(0x300c),
        Some(0x300c0201),
        Some(0x01),
        Some(0x67),
        Some(0x00),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(120));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("ldap_directory_sync_session".into())
    );
    let phases = export.program_flows[0]
        .stages
        .iter()
        .filter_map(|stage| stage.phase.clone())
        .collect::<Vec<_>>();
    assert!(phases.contains(&"connect".to_string()));
    assert!(phases.contains(&"establish".to_string()));
    assert!(phases.contains(&"send_bind".to_string()));
    assert!(phases.contains(&"receive_bind_response".to_string()));
    assert!(phases.contains(&"send_search".to_string()));
    assert!(phases.contains(&"receive_search_result".to_string()));
    assert!(phases.contains(&"send_modify".to_string()));
    assert!(phases.contains(&"receive_modify_response".to_string()));
    let phase_kinds = export.program_flows[0]
        .stages
        .iter()
        .filter_map(|stage| stage.phase_kind.clone())
        .collect::<Vec<_>>();
    assert!(phase_kinds.contains(&"initiate_connection".to_string()));
    assert!(phase_kinds.contains(&"establish_connection".to_string()));
    assert!(phase_kinds.contains(&"emit_payload".to_string()));
    assert!(phase_kinds.contains(&"receive_payload".to_string()));
    assert_eq!(export.module_findings.len(), 0);
}

#[test]
fn ldap_directory_sync_session_missing_modify_produces_search_to_modify_transition() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/ldap_directory_sync_session.gewy")
            .unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 840, 54026, "ldap-directory-sync"));
    session.ingest(route_fact(2, 840, 7));
    session.ingest(tcp_state_fact_with_ports(3, 840, 1, 2, 54026, 389));
    session.ingest(tcp_state_fact_with_ports(4, 840, 2, 3, 54026, 389));
    session.ingest(packet_fact_with_dir_and_payload_and_bytes4_and5(
        5,
        840,
        0x18,
        PacketDir::Egress,
        Some(54026),
        Some(389),
        Some(0x30),
        Some(0x300c),
        Some(0x300c0201),
        Some(0x01),
        Some(0x60),
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_bytes4_and5(
        6,
        840,
        0x18,
        PacketDir::Ingress,
        Some(54026),
        Some(389),
        Some(0x30),
        Some(0x300c),
        Some(0x300c0201),
        Some(0x01),
        Some(0x61),
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_bytes4_and5(
        7,
        840,
        0x18,
        PacketDir::Egress,
        Some(54026),
        Some(389),
        Some(0x30),
        Some(0x3010),
        Some(0x30100201),
        Some(0x01),
        Some(0x63),
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_bytes4_and5(
        8,
        840,
        0x18,
        PacketDir::Ingress,
        Some(54026),
        Some(389),
        Some(0x30),
        Some(0x300c),
        Some(0x300c0201),
        Some(0x01),
        Some(0x65),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(100));

    let export = session.export_bundle();
    assert!(export.program_findings.iter().any(|finding| {
        finding.module_label == "ldap_directory_sync_session"
            && finding.phase.as_deref() == Some("send_modify")
            && finding.phase_transition.as_deref() == Some("receive_search_result->send_modify")
    }));
    assert!(export.module_findings.iter().any(|finding| {
        finding
            .phase_transitions
            .contains(&"receive_search_result->send_modify".to_string())
    }));
}

#[test]
fn ldap_directory_sync_session_failed_modify_response_produces_modify_transition() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/ldap_directory_sync_session.gewy")
            .unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 841, 54027, "ldap-directory-sync"));
    session.ingest(route_fact(2, 841, 7));
    session.ingest(tcp_state_fact_with_ports(3, 841, 1, 2, 54027, 389));
    session.ingest(tcp_state_fact_with_ports(4, 841, 2, 3, 54027, 389));
    session.ingest(packet_fact_with_dir_and_payload_and_bytes4_and5(
        5,
        841,
        0x18,
        PacketDir::Egress,
        Some(54027),
        Some(389),
        Some(0x30),
        Some(0x300c),
        Some(0x300c0201),
        Some(0x01),
        Some(0x60),
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_bytes4_and5(
        6,
        841,
        0x18,
        PacketDir::Ingress,
        Some(54027),
        Some(389),
        Some(0x30),
        Some(0x300c),
        Some(0x300c0201),
        Some(0x01),
        Some(0x61),
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_bytes4_and5(
        7,
        841,
        0x18,
        PacketDir::Egress,
        Some(54027),
        Some(389),
        Some(0x30),
        Some(0x3010),
        Some(0x30100201),
        Some(0x01),
        Some(0x63),
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_bytes4_and5(
        8,
        841,
        0x18,
        PacketDir::Ingress,
        Some(54027),
        Some(389),
        Some(0x30),
        Some(0x300c),
        Some(0x300c0201),
        Some(0x01),
        Some(0x65),
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_bytes4_5_and9(
        9,
        841,
        0x18,
        PacketDir::Egress,
        Some(54027),
        Some(389),
        Some(0x30),
        Some(0x3012),
        Some(0x30120201),
        Some(0x01),
        Some(0x66),
        None,
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_bytes4_5_and9(
        10,
        841,
        0x18,
        PacketDir::Ingress,
        Some(54027),
        Some(389),
        Some(0x30),
        Some(0x300c),
        Some(0x300c0201),
        Some(0x01),
        Some(0x67),
        Some(0x31),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(120));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .all(|stage| stage.phase.as_deref() != Some("receive_modify_response"))
    );
    assert!(export.program_findings.iter().any(|finding| {
        finding.module_label == "ldap_directory_sync_session"
            && finding.phase.as_deref() == Some("receive_modify_response")
            && finding.phase_transition.as_deref() == Some("send_modify->receive_modify_response")
    }));
    assert!(export.module_findings.iter().any(|finding| {
        finding
            .phase_transitions
            .contains(&"send_modify->receive_modify_response".to_string())
    }));
}

#[test]
fn snmp_get_path_materializes_request_and_response_datagrams() {
    let binding = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/snmp_get_path.gewy").unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 829, 54000, "snmpwalk"));
    session.ingest(route_fact(2, 829, 7));
    session.ingest(
        udp_packet_fact_with_dir_and_ports_and_payload_prefix4_and_byte13(
            3,
            829,
            96,
            PacketDir::Egress,
            Some(54000),
            Some(161),
            Some(0x30),
            Some(0x3026),
            Some(0x30260201),
            Some(0xa0),
        ),
    );
    session.ingest(
        udp_packet_fact_with_dir_and_ports_and_payload_prefix4_and_byte13(
            4,
            829,
            104,
            PacketDir::Ingress,
            Some(54000),
            Some(161),
            Some(0x30),
            Some(0x3028),
            Some(0x30280201),
            Some(0xa2),
        ),
    );
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(60));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("snmp_get".into())
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("send_get_request"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_get_response"))
    );
    let phase_kinds = export.program_flows[0]
        .stages
        .iter()
        .filter_map(|stage| stage.phase_kind.clone())
        .collect::<Vec<_>>();
    assert!(phase_kinds.contains(&"emit_datagram".to_string()));
    assert!(phase_kinds.contains(&"receive_datagram".to_string()));
    assert_eq!(export.module_findings.len(), 0);
}

#[test]
fn snmp_get_path_does_not_match_wrong_response_pdu_type() {
    let binding = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/snmp_get_path.gewy").unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 830, 54000, "snmpwalk"));
    session.ingest(route_fact(2, 830, 7));
    session.ingest(
        udp_packet_fact_with_dir_and_ports_and_payload_prefix4_and_byte13(
            3,
            830,
            96,
            PacketDir::Egress,
            Some(54000),
            Some(161),
            Some(0x30),
            Some(0x3026),
            Some(0x30260201),
            Some(0xa0),
        ),
    );
    session.ingest(
        udp_packet_fact_with_dir_and_ports_and_payload_prefix4_and_byte13(
            4,
            830,
            104,
            PacketDir::Ingress,
            Some(54000),
            Some(161),
            Some(0x30),
            Some(0x3028),
            Some(0x30280201),
            Some(0xa1),
        ),
    );
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(60));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .all(|stage| stage.phase.as_deref() != Some("receive_get_response"))
    );
}

#[test]
fn mqtt_connect_path_does_not_match_wrong_connack_prefix() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/mqtt_connect_path.gewy").unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 825, 53002, "mosquitto-pub"));
    session.ingest(route_fact(2, 825, 7));
    session.ingest(packet_fact_with_dir_and_payload(
        3,
        825,
        0x18,
        PacketDir::Egress,
        Some(53002),
        Some(1883),
        Some(0x10),
        Some(0x1016),
        Some(0x10160004),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        4,
        825,
        0x18,
        PacketDir::Ingress,
        Some(53002),
        Some(1883),
        Some(0x20),
        Some(0x2002),
        Some(0x20020001),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(60));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .all(|stage| stage.phase.as_deref() != Some("receive_connack"))
    );
}

#[test]
fn dns_tcp_query_path_materializes_request_and_response_payload_phases() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/dns_tcp_query_path.gewy").unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 825, 53053, "dig"));
    session.ingest(route_fact(2, 825, 7));
    session.ingest(packet_fact_with_dir_and_payload_and_byte4(
        3,
        825,
        0x18,
        PacketDir::Egress,
        Some(53053),
        Some(53),
        Some(0x00),
        Some(0x001c),
        Some(0x001c1234),
        Some(0x01),
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_byte4(
        4,
        825,
        0x18,
        PacketDir::Ingress,
        Some(53053),
        Some(53),
        Some(0x00),
        Some(0x001c),
        Some(0x001c1234),
        Some(0x81),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(60));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("dns_tcp_query".into())
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("send_query"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_response"))
    );
    let phase_kinds = export.program_flows[0]
        .stages
        .iter()
        .filter_map(|stage| stage.phase_kind.clone())
        .collect::<Vec<_>>();
    assert!(phase_kinds.contains(&"emit_payload".to_string()));
    assert!(phase_kinds.contains(&"receive_payload".to_string()));
    assert_eq!(export.module_findings.len(), 0);
}

#[test]
fn dns_tcp_query_path_does_not_match_wrong_response_qr_bit() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/dns_tcp_query_path.gewy").unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 826, 53053, "dig"));
    session.ingest(route_fact(2, 826, 7));
    session.ingest(packet_fact_with_dir_and_payload_and_byte4(
        3,
        826,
        0x18,
        PacketDir::Egress,
        Some(53053),
        Some(53),
        Some(0x00),
        Some(0x001c),
        Some(0x001c1234),
        Some(0x01),
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_byte4(
        4,
        826,
        0x18,
        PacketDir::Ingress,
        Some(53053),
        Some(53),
        Some(0x00),
        Some(0x001c),
        Some(0x001c1234),
        Some(0x01),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(60));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .all(|stage| stage.phase.as_deref() != Some("receive_response"))
    );
}

#[test]
fn https_connect_dsl_uses_destination_port_to_model_connect_path() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/https_connect_process.gewy").unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 401, 9001, "curl"));
    session.ingest(tcp_state_fact_with_ports(2, 401, 1, 2, 42310, 443));
    session.ingest(route_fact(3, 401, 5));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(40));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("https_connect".into())
    );
    assert!(
        export.program_flows[0]
            .narrative
            .iter()
            .any(|line| line.contains("HTTPS socket state transition"))
    );
    assert!(
        export.reasons[0]
            .l3
            .narrative
            .iter()
            .any(|line| line.text.contains("tcp state 1 -> 2"))
    );
}

#[test]
fn https_connect_dsl_does_not_treat_other_ports_as_https_connect() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/https_connect_process.gewy").unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 402, 9002, "curl"));
    session.ingest(tcp_state_fact_with_ports(2, 402, 1, 2, 42310, 80));
    session.ingest(route_fact(3, 402, 5));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(40));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .narrative
            .iter()
            .all(|line| !line.contains("HTTPS socket state transition"))
    );
    assert!(
        export.reasons[0]
            .l3
            .narrative
            .iter()
            .all(|line| !line.text.contains("tcp state 1 -> 2"))
    );
}

#[test]
fn postgres_connect_dsl_uses_named_port_alias() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/postgres_connect_process.gewy")
            .unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 501, 7777, "psql"));
    session.ingest(tcp_state_fact_with_ports(2, 501, 1, 2, 43123, 5432));
    session.ingest(tcp_state_fact_with_ports(3, 501, 2, 3, 43123, 5432));
    session.ingest(route_fact(4, 501, 6));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(50));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("postgres_connect".into())
    );
    assert!(
        export.program_flows[0]
            .narrative
            .iter()
            .any(|line| line.contains("PostgreSQL socket state transition"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("connect"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("establish"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("resolve"))
    );
    assert_eq!(export.module_findings.len(), 0);
}

#[test]
fn postgres_simple_query_path_materializes_connect_query_and_ready_phases() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/postgres_simple_query_path.gewy")
            .unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 506, 7781, "psql"));
    session.ingest(tcp_state_fact_with_ports(2, 506, 1, 2, 43125, 5432));
    session.ingest(tcp_state_fact_with_ports(3, 506, 2, 3, 43125, 5432));
    session.ingest(route_fact(4, 506, 6));
    session.ingest(packet_fact_with_dir_and_payload(
        5,
        506,
        0,
        PacketDir::Egress,
        Some(43125),
        Some(5432),
        Some(0x51),
        None,
        None,
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        6,
        506,
        0,
        PacketDir::Ingress,
        Some(43125),
        Some(5432),
        Some(0x5a),
        None,
        None,
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(70));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("postgres_simple_query".into())
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("connect"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("establish"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("send_query"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_ready"))
    );
    let phase_kinds = export.program_flows[0]
        .stages
        .iter()
        .filter_map(|stage| stage.phase_kind.clone())
        .collect::<Vec<_>>();
    assert!(phase_kinds.contains(&"initiate_connection".to_string()));
    assert!(phase_kinds.contains(&"establish_connection".to_string()));
    assert!(phase_kinds.contains(&"emit_payload".to_string()));
    assert!(phase_kinds.contains(&"receive_payload".to_string()));
    assert_eq!(export.module_findings.len(), 0);
}

#[test]
fn postgres_simple_query_path_does_not_match_wrong_server_message_type() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/postgres_simple_query_path.gewy")
            .unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 507, 7782, "psql"));
    session.ingest(tcp_state_fact_with_ports(2, 507, 1, 2, 43126, 5432));
    session.ingest(tcp_state_fact_with_ports(3, 507, 2, 3, 43126, 5432));
    session.ingest(route_fact(4, 507, 6));
    session.ingest(packet_fact_with_dir_and_payload(
        5,
        507,
        0,
        PacketDir::Egress,
        Some(43126),
        Some(5432),
        Some(0x51),
        None,
        None,
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        6,
        507,
        0,
        PacketDir::Ingress,
        Some(43126),
        Some(5432),
        Some(0x45),
        None,
        None,
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(70));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .all(|stage| stage.phase.as_deref() != Some("receive_ready"))
    );
}

#[test]
fn redis_connect_dsl_uses_named_port_alias() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/redis_connect_process.gewy").unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 502, 8888, "redis-cli"));
    session.ingest(tcp_state_fact_with_ports(2, 502, 1, 2, 43124, 6379));
    session.ingest(route_fact(3, 502, 6));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(40));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("redis_connect".into())
    );
    assert!(
        export.program_flows[0]
            .narrative
            .iter()
            .any(|line| line.contains("Redis socket state transition"))
    );
}

#[test]
fn declarative_module_phases_are_preserved_in_export_and_replay() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/postgres_connect_process.gewy")
            .unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 503, 7778, "psql"));
    session.ingest(tcp_state_fact_with_ports(2, 503, 1, 2, 43123, 5432));
    session.ingest(route_fact(3, 503, 6));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(40));

    let export = session.export_bundle();
    let phases = export.program_flows[0]
        .stages
        .iter()
        .filter_map(|stage| stage.phase.clone())
        .collect::<Vec<_>>();
    assert!(phases.contains(&"bind".to_string()));
    assert!(phases.contains(&"connect".to_string()));
    assert!(phases.contains(&"resolve".to_string()));

    let replay = gewyvern::export::ExportBundle::from_json(&export.to_json())
        .unwrap()
        .replay()
        .unwrap();
    assert_eq!(export.program_flows, replay.program_flows);
}

#[test]
fn missing_connect_phase_produces_bind_to_connect_transition_finding() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/postgres_connect_process.gewy")
            .unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 504, 7779, "psql"));
    session.ingest(route_fact(2, 504, 6));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(40));

    let export = session.export_bundle();
    assert_eq!(export.program_findings.len(), 1);
    assert_eq!(export.program_findings[0].phase.as_deref(), Some("connect"));
    assert_eq!(
        export.program_findings[0].phase_transition.as_deref(),
        Some("bind->connect")
    );
    assert_eq!(
        export.program_findings[0].phase_transition_kind.as_deref(),
        Some("bind_process->initiate_connection")
    );
    assert!(export.program_findings[0].summary.contains("bind->connect"));
    assert_eq!(
        export.module_findings[0].phase_transitions,
        vec!["bind->connect".to_string()]
    );

    let replay = gewyvern::export::ExportBundle::from_json(&export.to_json())
        .unwrap()
        .replay()
        .unwrap();
    assert_eq!(export.program_findings, replay.program_findings);
    assert_eq!(export.module_findings, replay.module_findings);
}

#[test]
fn missing_establish_phase_produces_connect_to_establish_transition_finding() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/postgres_connect_process.gewy")
            .unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 505, 7780, "psql"));
    session.ingest(tcp_state_fact_with_ports(2, 505, 1, 2, 43123, 5432));
    session.ingest(route_fact(3, 505, 6));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(40));

    let export = session.export_bundle();
    assert!(export.program_findings.iter().any(|finding| {
        finding.phase.as_deref() == Some("establish")
            && finding.phase_transition.as_deref() == Some("connect->establish")
    }));
    assert!(export.module_findings.iter().any(|finding| {
        finding
            .phase_transitions
            .contains(&"connect->establish".to_string())
    }));
}

#[test]
fn handshake_dsl_compiles_and_preserves_tcp_shape() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/handshake_debug.gewy").unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(tcp_state_fact(1, 203, 1, 2));
    session.ingest(route_fact(2, 203, 2));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(30));

    let export = session.export_bundle();
    assert_eq!(export.template_id, "handshake_debug");
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::ConnectFlow
    );
}

#[test]
fn dsl_supports_inline_window_and_infers_program_model_id() {
    let binding = compile_str(
        r#"
template=udp_inline_debug
window.duration_ms=9000
window.lateness_ms=450
reason=udp_datagram_l1
fragment=udp_packet_meta_fragment
fragment=route_meta_fragment
operation=datagram_exchange
rule=datagram_observed:udp;datagram_observed;static:inline udp activity observed;true
"#,
    )
    .unwrap();

    assert_eq!(
        binding.template.window_profile.as_ref().unwrap().id,
        "inline"
    );
    assert_eq!(
        binding
            .template
            .window_profile
            .as_ref()
            .unwrap()
            .duration_ms,
        9_000
    );
    assert_eq!(
        binding
            .template
            .window_profile
            .as_ref()
            .unwrap()
            .lateness_ms,
        450
    );
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().id,
        "udp_inline_debug_dsl_model"
    );
}

#[test]
fn dsl_accepts_structured_template_blocks() {
    let binding = compile_str(
        r#"
template structured_udp_debug {
  window default_5s
  reason udp_datagram_l1

  fragments {
    udp_packet_meta_fragment
    route_meta_fragment
    sock_lineage_fragment
  }

  program_model structured_udp_debug_model {
    operation datagram_exchange

    rule {
      predicate process_bound
      stage process_bound
      narrative process_bound
      dedupe true
      module structured_udp_debug
      phase bind
    }

    rule {
      predicate datagram_observed:udp:local_to_remote
      stage datagram_observed
      narrative udp_datagram_sent
      dedupe true
      module structured_udp_debug
      phase send_request
    }
  }
}
"#,
    )
    .unwrap();

    assert_eq!(binding.template.id, "structured_udp_debug");
    assert_eq!(
        binding.template.fragment_set,
        vec![
            "udp_packet_meta_fragment",
            "route_meta_fragment",
            "sock_lineage_fragment"
        ]
    );
    let model = binding.template.program_model.as_ref().unwrap();
    assert_eq!(model.id, "structured_udp_debug_model");
    assert_eq!(model.operation, ProgramOperation::DatagramExchange);
    assert_eq!(model.rules.len(), 2);
    assert_eq!(
        model.rules[1].module.as_deref(),
        Some("structured_udp_debug")
    );
    assert_eq!(model.rules[1].phase.as_deref(), Some("send_request"));
}

#[test]
fn dsl_accepts_structured_reason_model_blocks() {
    let binding = compile_str(
        r#"
template structured_reason_udp {
  window default_5s
  fragments {
    udp_packet_meta_fragment
    route_meta_fragment
    sock_lineage_fragment
  }

  program_model structured_reason_udp_model {
    operation datagram_exchange
    rule {
      predicate process_bound
      stage process_bound
      narrative process_bound
      dedupe true
      module structured_reason_udp
      phase bind
    }
  }

  reason_model structured_reason_udp_reason {
    rule {
      predicate process_bound
      key_event process_identified
      narrative process_bound
      dedupe true
      module structured_reason_udp
      phase bind
    }
  }
}
"#,
    )
    .unwrap();

    assert!(matches!(
        binding.template.reason_profile.as_ref().unwrap(),
        ReasonProfile::Declarative(reason) if reason.id == "structured_reason_udp_reason"
    ));
}

#[test]
fn dsl_can_fall_back_to_default_program_model_from_reason_profile() {
    let binding = compile_str(
        r#"
template=udp_minimal
window.duration_ms=5000
window.lateness_ms=200
reason=udp_datagram_l1
fragment=udp_packet_meta_fragment
fragment=route_meta_fragment
"#,
    )
    .unwrap();

    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::DatagramExchange
    );
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().id,
        "datagram_exchange_v1"
    );
}

#[test]
fn dsl_supports_declarative_reason_rules_and_replay_preserves_them() {
    let binding = compile_str(
        r#"
template=udp_reason_inline
window.duration_ms=5000
window.lateness_ms=200
fragment=udp_packet_meta_fragment
fragment=route_meta_fragment
fragment=sock_lineage_fragment
operation=datagram_exchange
rule=process_bound;process_bound;process_bound;true
reason.rule=process_bound;process_identified;process_bound;true
reason.rule=datagram_observed:udp;udp_datagram_seen;udp_datagram_observed;true
reason.rule=route_resolved;route_changed;route_changed;true
"#,
    )
    .unwrap();

    assert!(matches!(
        binding.template.reason_profile.as_ref().unwrap(),
        ReasonProfile::Declarative(_)
    ));
    assert_eq!(
        binding.template.reason_profile.as_ref().unwrap().id(),
        "udp_reason_inline_reason_model"
    );

    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 204, 4444, "dig"));
    session.ingest(udp_packet_fact(2, 204, 96));
    session.ingest(route_fact(3, 204, 8));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(40));

    let export = session.export_bundle();
    assert_eq!(export.reason_profile.id(), "udp_reason_inline_reason_model");
    assert_eq!(export.reasons[0].l1.key_events.len(), 3);
    assert_eq!(
        export.reasons[0].l1.key_events[0].kind,
        KeyEventKind::ProcessIdentified
    );
    assert_eq!(
        export.reasons[0].l1.key_events[1].kind,
        KeyEventKind::UdpDatagramSeen
    );
    assert_eq!(
        export.reasons[0].l1.key_events[2].kind,
        KeyEventKind::RouteChanged
    );

    let replay = gewyvern::export::ExportBundle::from_json(&export.to_json())
        .unwrap()
        .replay()
        .unwrap();
    assert_eq!(export.reason_profile, replay.reason_profile);
    assert_eq!(export.reasons, replay.reasons);
}

#[test]
fn dsl_program_rules_can_use_shared_narrative_templates() {
    let binding = compile_str(
        r#"
template=udp_shared_ir
window.duration_ms=5000
window.lateness_ms=200
reason=udp_datagram_l1
fragment=udp_packet_meta_fragment
fragment=route_meta_fragment
fragment=sock_lineage_fragment
operation=datagram_exchange
rule=process_bound;process_bound;process_bound;true
rule=datagram_observed:udp;datagram_observed;udp_datagram_observed;true
rule=route_resolved;route_resolved;route_changed;true
"#,
    )
    .unwrap();

    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 205, 5353, "dig"));
    session.ingest(udp_packet_fact(2, 205, 88));
    session.ingest(route_fact(3, 205, 9));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(40));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .narrative
            .iter()
            .any(|line| line == "process dig (pid=5353) bound this network flow")
    );
    assert!(
        export.program_flows[0]
            .narrative
            .iter()
            .any(|line| line == "program emitted or received a UDP datagram")
    );
    assert!(
        export.program_flows[0]
            .narrative
            .iter()
            .any(|line| line == "program resolved a route for this network flow")
    );
}

#[test]
fn dsl_reason_rules_can_use_shared_signal_ids() {
    let binding = compile_str(
        r#"
template=udp_shared_signal_reason
window.duration_ms=5000
window.lateness_ms=200
fragment=udp_packet_meta_fragment
fragment=route_meta_fragment
fragment=sock_lineage_fragment
operation=datagram_exchange
rule=process_bound;process_bound;process_bound;true
reason.rule=process_bound;process_bound;process_bound;true
reason.rule=datagram_observed:udp;datagram_observed;udp_datagram_observed;true
reason.rule=route_resolved;route_resolved;route_changed;true
"#,
    )
    .unwrap();

    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 206, 5354, "dig"));
    session.ingest(udp_packet_fact(2, 206, 88));
    session.ingest(route_fact(3, 206, 9));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(40));

    let export = session.export_bundle();
    assert_eq!(
        export.reasons[0].l1.key_events[0].kind,
        KeyEventKind::ProcessIdentified
    );
    assert_eq!(
        export.reasons[0].l1.key_events[1].kind,
        KeyEventKind::UdpDatagramSeen
    );
    assert_eq!(
        export.reasons[0].l1.key_events[2].kind,
        KeyEventKind::RouteChanged
    );
}

#[test]
fn dsl_rejects_program_rules_when_fragment_set_cannot_supply_evidence() {
    let err = compile_str(
        r#"
template=route_only_invalid
window.duration_ms=5000
window.lateness_ms=200
reason=udp_datagram_l1
fragment=route_meta_fragment
operation=datagram_exchange
rule=process_bound;process_bound;process_bound;true
"#,
    )
    .unwrap_err();

    assert_eq!(
        err,
        DslError::Registry(RegistryError::MissingRuleEvidence {
            model: "program_model".into(),
            rule_index: 0,
            missing: vec![gewyvern::ledger::FactKindTag::SockLineage],
        })
    );
}

#[test]
fn binding_diagnostics_report_rule_support_and_supporting_fragments() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy").unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let session = RuntimeSession::start(config).unwrap();
    let export = session.export_bundle();

    let diagnostics = export.binding_diagnostics.program_model.as_ref().unwrap();
    assert_eq!(diagnostics.model, "udp_process_debug_dsl_model");
    assert_eq!(diagnostics.rules.len(), 3);
    assert!(diagnostics.rules.iter().all(|rule| rule.supported));
    assert_eq!(diagnostics.rules[0].tier, RuleTier::OptionalEnhancement);
    assert_eq!(diagnostics.rules[1].tier, RuleTier::CoreRequirement);
    assert_eq!(diagnostics.rules[2].tier, RuleTier::CoreRequirement);
    assert_eq!(
        diagnostics.rules[0].required_facts,
        vec![gewyvern::ledger::FactKindTag::SockLineage]
    );
    assert_eq!(
        diagnostics.rules[0].supporting_fragments,
        vec!["sock_lineage_fragment".to_string()]
    );
    assert_eq!(
        diagnostics.rules[1].supporting_fragments,
        vec!["udp_packet_meta_fragment".to_string()]
    );
    assert_eq!(
        diagnostics.rules[2].supporting_fragments,
        vec!["route_meta_fragment".to_string()]
    );

    let replay = gewyvern::export::ExportBundle::from_json(&export.to_json())
        .unwrap()
        .replay()
        .unwrap();
    assert_eq!(export.binding_diagnostics, replay.binding_diagnostics);
}

#[test]
fn binding_diagnostics_reports_unsupported_payload_offsets() {
    let binding = parse_str_unvalidated(
        r#"
template=unsupported_payload_offset
window=default_5s
reason=udp_datagram_l1
fragment=udp_packet_meta_fragment
program_model=unsupported_payload_offset_model
operation=snmp_get
rule=datagram_observed:udp:remote:snmp:byte_at:8:0xff:0xa0;datagram_observed;udp_datagram_sent;true
"#,
    )
    .unwrap();

    let diagnostics = collect_binding_diagnostics(&binding).unwrap();
    let rule = &diagnostics.program_model.as_ref().unwrap().rules[0];
    assert!(!rule.supported);
    assert_eq!(rule.tier, RuleTier::Unsupported);
    assert_eq!(
        rule.missing_facts,
        Vec::<gewyvern::ledger::FactKindTag>::new()
    );
    assert_eq!(rule.unsupported_payload_offsets, vec![8]);
}

#[test]
fn dsl_validation_rejects_rules_with_unsupported_payload_offsets() {
    let err = compile_str(
        r#"
template=unsupported_payload_offset_compile
window=default_5s
reason=udp_datagram_l1
fragment=udp_packet_meta_fragment
program_model=unsupported_payload_offset_compile_model
operation=snmp_get
rule=datagram_observed:udp:remote:snmp:byte_at:8:0xff:0xa0;datagram_observed;udp_datagram_sent;true
"#,
    )
    .unwrap_err();

    assert_eq!(
        err,
        DslError::Registry(RegistryError::UnsupportedRulePayloadOffsets {
            model: "program_model".into(),
            rule_index: 0,
            offsets: vec![8],
        })
    );
}

#[test]
fn dsl_can_override_evidence_tiers_per_template() {
    let binding = compile_str(
        r#"
template=udp_process_core_lineage
window=default_5s
reason=udp_datagram_l1
fragment=udp_packet_meta_fragment
fragment=route_meta_fragment
fragment=sock_lineage_fragment
operation=datagram_exchange
rule=process_bound;process_bound;process_bound;true
rule=datagram_observed:udp;datagram_observed;udp_datagram_observed;true
rule=route_resolved;route_resolved;route_changed;true
evidence=sock_lineage:core_requirement
evidence=packet_meta:optional_enhancement
"#,
    )
    .unwrap();

    let config = SessionConfig::for_binding(binding).unwrap();
    let session = RuntimeSession::start(config).unwrap();
    let export = session.export_bundle();
    let diagnostics = export.binding_diagnostics.program_model.as_ref().unwrap();

    assert_eq!(diagnostics.rules[0].tier, RuleTier::CoreRequirement);
    assert_eq!(diagnostics.rules[1].tier, RuleTier::OptionalEnhancement);
    assert_eq!(diagnostics.rules[2].tier, RuleTier::CoreRequirement);

    let replay = gewyvern::export::ExportBundle::from_json(&export.to_json())
        .unwrap()
        .replay()
        .unwrap();
    assert_eq!(export.evidence_overrides, replay.evidence_overrides);
    assert_eq!(export.binding_diagnostics, replay.binding_diagnostics);
}

#[test]
fn dsl_rejects_unknown_fragment_param_keys() {
    let err = compile_str(
        r#"
template=udp_process_debug
window=default_5s
reason=udp_datagram_l1
fragment=udp_packet_meta_fragment
fragment=route_meta_fragment
fragment=sock_lineage_fragment
program_model=datagram_exchange_v1
operation=datagram_exchange
rule=process_bound;process_bound;process_bound;true
param=sock_lineage_fragment.not_a_real_param=true
"#,
    )
    .unwrap_err();

    assert_eq!(
        err,
        DslError::Registry(RegistryError::UnknownFragmentParam {
            fragment_id: "sock_lineage_fragment".into(),
            key: "not_a_real_param".into(),
        })
    );
}

#[test]
fn dsl_rejects_fragment_param_type_mismatches() {
    let err = compile_str(
        r#"
template=udp_process_debug
window=default_5s
reason=udp_datagram_l1
fragment=udp_packet_meta_fragment
fragment=route_meta_fragment
fragment=sock_lineage_fragment
program_model=datagram_exchange_v1
operation=datagram_exchange
rule=process_bound;process_bound;process_bound;true
param=udp_packet_meta_fragment.min_len=false
"#,
    )
    .unwrap_err();

    assert_eq!(
        err,
        DslError::Registry(RegistryError::InvalidFragmentParamType {
            fragment_id: "udp_packet_meta_fragment".into(),
            key: "min_len".into(),
            expected: "u64",
        })
    );
}
#[test]
fn dsl_accepts_datagram_prefix2_qualifier() {
    let binding = compile_str(
        r#"
template=quic_initial_prefix2_match
window=default_5s
reason=udp_datagram_l1
fragment=udp_packet_meta_fragment
program_model=quic_initial_prefix2_match_model
operation=quic_client_initial
rule=datagram_observed:udp:remote:quic:local_to_remote:min_len:1200:byte0_mask:0xf0:0xc0:prefix2:0xc300;datagram_observed;udp_datagram_sent;true
"#,
    )
    .unwrap();

    let rule = &binding.template.program_model.as_ref().unwrap().rules[0];
    assert_eq!(
        rule.predicate,
        gewyvern::ir::FlowPredicate::DatagramObserved {
            l4_proto: 17,
            dir: Some(PacketDir::Egress),
            local_port: None,
            remote_port: Some(443),
            min_len: Some(1200),
            first_byte_mask: Some(0xf0),
            first_byte_value: Some(0xc0),
            prefix2: Some(0xc300),
            prefix4: None,
            byte13_mask: None,
            byte13_value: None,
            byte_matches: vec![],
        }
    );
}
