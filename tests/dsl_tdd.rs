use gewyvern::dsl::{DslError, compile_file, compile_str, parse_str_unvalidated};
use gewyvern::flow::ProgramOperation;
use gewyvern::fragment::{RegistryError, RuleTier, builtin_registry};
use gewyvern::gewyc::collect_binding_diagnostics;
use gewyvern::ledger::PacketDir;
use gewyvern::reason::{KeyEventKind, ReasonProfile};
use gewyvern::runtime::{RuntimeSession, SessionConfig};
use gewyvern::template::FragmentParamValue;

mod support;

use std::fs;
use std::time::{Duration, SystemTime};
use support::{
    packet_fact, packet_fact_with_dir, packet_fact_with_dir_and_payload,
    packet_fact_with_dir_and_payload_and_byte1, packet_fact_with_dir_and_payload_and_byte4,
    packet_fact_with_dir_and_payload_and_byte10,
    packet_fact_with_dir_and_payload_and_bytes4_5_and9,
    packet_fact_with_dir_and_payload_and_bytes4_and5, packet_fact_with_dir_and_payload_bytes,
    route_fact, sock_lineage_fact, tcp_state_fact, tcp_state_fact_with_ports, udp_packet_fact,
    udp_packet_fact_with_dir, udp_packet_fact_with_dir_and_ports_and_payload,
    udp_packet_fact_with_dir_and_ports_and_payload_prefix4,
    udp_packet_fact_with_dir_and_ports_and_payload_prefix4_and_byte13, udp_quic_meta_fact,
    udp_quic_meta_fact_with_payload_bytes,
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
fn built_in_pipeline_udp_process_dsl_compiles_into_template_binding() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/pipeline_udp_process_debug.gewy")
            .unwrap();

    assert_eq!(binding.template.id, "pipeline_udp_process_debug");
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
fn built_in_postgres_auth_path_dsl_compiles_into_template_binding() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/postgres_auth_path.gewy").unwrap();

    assert_eq!(binding.template.id, "postgres_auth_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("postgres_auth".into())
    );
    assert!(matches!(
        binding.template.reason_profile.as_ref().unwrap(),
        ReasonProfile::Declarative(_)
    ));
}

#[test]
fn built_in_postgres_query_error_path_dsl_compiles_into_template_binding() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/postgres_query_error_path.gewy")
            .unwrap();

    assert_eq!(binding.template.id, "postgres_query_error_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("postgres_query_error".into())
    );
    assert!(matches!(
        binding.template.reason_profile.as_ref().unwrap(),
        ReasonProfile::Declarative(_)
    ));
}

#[test]
fn built_in_mysql_connect_process_dsl_compiles_into_template_binding() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/mysql_connect_process.gewy").unwrap();

    assert_eq!(binding.template.id, "mysql_connect_process");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("mysql_connect".into())
    );
}

#[test]
fn built_in_mysql_simple_query_path_dsl_compiles_into_template_binding() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/mysql_simple_query_path.gewy").unwrap();

    assert_eq!(binding.template.id, "mysql_simple_query_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("mysql_simple_query".into())
    );
}

#[test]
fn built_in_mysql_query_session_dsl_compiles_into_template_binding() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/mysql_query_session.gewy").unwrap();

    assert_eq!(binding.template.id, "mysql_query_session");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("mysql_query_session".into())
    );
}

#[test]
fn built_in_mysql_query_error_path_dsl_compiles_into_template_binding() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/mysql_query_error_path.gewy").unwrap();

    assert_eq!(binding.template.id, "mysql_query_error_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("mysql_query_error".into())
    );
}

#[test]
fn built_in_memcached_get_path_dsl_compiles_into_template_binding() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/memcached_get_path.gewy").unwrap();

    assert_eq!(binding.template.id, "memcached_get_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("memcached_get".into())
    );
}

#[test]
fn built_in_memcached_set_path_dsl_compiles_into_template_binding() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/memcached_set_path.gewy").unwrap();

    assert_eq!(binding.template.id, "memcached_set_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("memcached_set".into())
    );
}

#[test]
fn built_in_amqp_connection_start_path_dsl_compiles_into_template_binding() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/amqp_connection_start_path.gewy")
            .unwrap();

    assert_eq!(binding.template.id, "amqp_connection_start_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("amqp_connection_start".into())
    );
}

#[test]
fn built_in_amqp_basic_publish_path_dsl_compiles_into_template_binding() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/amqp_basic_publish_path.gewy").unwrap();

    assert_eq!(binding.template.id, "amqp_basic_publish_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("amqp_basic_publish".into())
    );
}

#[test]
fn built_in_amqp_publish_session_dsl_compiles_into_template_binding() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/amqp_publish_session.gewy").unwrap();

    assert_eq!(binding.template.id, "amqp_publish_session");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("amqp_publish_session".into())
    );
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
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/gtpu_echo_path.gewy").unwrap();

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
fn built_in_http3_request_path_dsl_compiles_into_template_binding() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/http3_request_path.gewy").unwrap();

    assert_eq!(binding.template.id, "http3_request_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("http3_request".into())
    );
}

#[test]
fn built_in_http3_server_response_path_dsl_compiles_into_template_binding() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/http3_server_response_path.gewy")
            .unwrap();

    assert_eq!(binding.template.id, "http3_server_response_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("http3_server_response".into())
    );
}

#[test]
fn built_in_hy2_auth_path_dsl_compiles_into_template_binding() {
    let binding = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/hy2_auth_path.gewy").unwrap();

    assert_eq!(binding.template.id, "hy2_auth_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("hy2_auth".into())
    );
}

#[test]
fn built_in_hy2_udp_relay_path_dsl_compiles_into_template_binding() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/hy2_udp_relay_path.gewy").unwrap();

    assert_eq!(binding.template.id, "hy2_udp_relay_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("hy2_udp_relay".into())
    );
}

#[test]
fn built_in_hy2_tcp_relay_path_dsl_compiles_into_template_binding() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/hy2_tcp_relay_path.gewy").unwrap();

    assert_eq!(binding.template.id, "hy2_tcp_relay_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("hy2_tcp_relay".into())
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
            byte_sequences: vec![],
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
            byte_sequences: vec![],
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
            byte_sequences: vec![],
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
            byte_sequences: vec![],
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
            byte_sequences: vec![],
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
            byte_sequences: vec![],
        }
    );
}

#[test]
fn dsl_accepts_quic_packet_observed_predicate() {
    let binding = compile_str(
        r#"
template=quic_packet_match
window=default_5s
reason=udp_datagram_l1
fragment=udp_packet_meta_fragment
program_model=quic_packet_match_model
operation=quic_client_initial
rule=quic_packet_observed:remote:quic:local_to_remote:min_len:1200:long_header:true:type:initial;datagram_observed;udp_datagram_sent;true
"#,
    )
    .unwrap();

    let rule = &binding.template.program_model.as_ref().unwrap().rules[0];
    assert_eq!(
        rule.predicate,
        gewyvern::ir::FlowPredicate::QuicPacketObserved {
            dir: Some(PacketDir::Egress),
            local_port: None,
            remote_port: Some(443),
            min_len: Some(1200),
            long_header: Some(true),
            packet_type: Some(gewyvern::ir::QuicPacketType::Initial),
        }
    );
}

#[test]
fn dsl_accepts_quic_frame_observed_predicate() {
    let binding = compile_str(
        r#"
template=quic_frame_match
window=default_5s
reason=udp_datagram_l1
fragment=udp_packet_meta_fragment
program_model=quic_frame_match_model
operation=quic_crypto_handshake
rule=quic_frame_observed:remote:quic:remote_to_local:type:handshake:frame:crypto;packet_observed;transport_payload_received;true
"#,
    )
    .unwrap();

    let rule = &binding.template.program_model.as_ref().unwrap().rules[0];
    assert_eq!(
        rule.predicate,
        gewyvern::ir::FlowPredicate::QuicFrameObserved {
            dir: Some(PacketDir::Ingress),
            local_port: None,
            remote_port: Some(443),
            packet_type: Some(gewyvern::ir::QuicPacketType::Handshake),
            frame_type: gewyvern::ir::QuicFrameType::Crypto,
            byte_matches: vec![],
            byte_sequences: vec![],
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
            byte_sequences: vec![],
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
            byte_sequences: vec![],
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
            byte_sequences: vec![],
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
fn dsl_accepts_datagram_bytes_at_qualifier() {
    let binding = compile_str(
        r#"
template=snmp_bytes_sequence_match
window=default_5s
reason=udp_datagram_l1
fragment=udp_packet_meta_fragment
param=udp_packet_meta_fragment.sample_payload_offsets=8
program_model=snmp_bytes_sequence_match_model
operation=snmp_get
rule=datagram_observed:udp:remote:snmp:bytes_at:8:0x30,0x82,0x01;datagram_observed;udp_datagram_sent;true
"#,
    )
    .unwrap();

    let rule = &binding.template.program_model.as_ref().unwrap().rules[0];
    assert!(matches!(
        &rule.predicate,
        gewyvern::ir::FlowPredicate::DatagramObserved {
            remote_port: Some(161),
            byte_sequences,
            ..
        } if byte_sequences == &vec![gewyvern::ir::PayloadByteSequenceMatch {
            offset: 8,
            bytes: vec![0x30, 0x82, 0x01],
        }]
    ));
}

#[test]
fn binding_diagnostics_accept_bytes_at_sequence_with_dynamic_offsets() {
    let binding = compile_str(
        r#"
template=snmp_bytes_sequence_runtime
window=default_5s
reason=udp_datagram_l1
fragment=udp_packet_meta_fragment
param=udp_packet_meta_fragment.sample_payload_offsets=8
program_model=snmp_bytes_sequence_runtime_model
operation=snmp_get
rule=datagram_observed:udp:remote:snmp:local_to_remote:bytes_at:8:0x30,0x82,0x01;datagram_observed;udp_datagram_sent;true
"#,
    )
    .unwrap();

    let registry = builtin_registry();
    let diagnostics = registry.binding_diagnostics(&binding).unwrap();
    let rule = &diagnostics.program_model.as_ref().unwrap().rules[0];
    assert!(rule.supported);
    assert_eq!(rule.unsupported_payload_offsets, Vec::<u16>::new());

    let summary = registry.payload_offset_support_summary(&binding, &diagnostics);
    assert!(summary.sampled_offsets.contains(&8));
    assert_eq!(summary.unsupported_offsets, Vec::<u16>::new());
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
fn built_in_quic_crypto_handshake_path_dsl_compiles_into_template_binding() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/quic_crypto_handshake_path.gewy")
            .unwrap();

    assert_eq!(binding.template.id, "quic_crypto_handshake_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("quic_crypto_handshake".into())
    );
}

#[test]
fn built_in_quic_stream_session_path_dsl_compiles_into_template_binding() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/quic_stream_session_path.gewy")
            .unwrap();

    assert_eq!(binding.template.id, "quic_stream_session_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("quic_stream_session".into())
    );
}

#[test]
fn built_in_quic_bidi_stream_path_dsl_compiles_into_template_binding() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/quic_bidi_stream_path.gewy").unwrap();

    assert_eq!(binding.template.id, "quic_bidi_stream_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("quic_bidi_stream".into())
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
fn built_in_ssh_session_path_dsl_compiles_into_template_binding() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/ssh_session_path.gewy").unwrap();

    assert_eq!(binding.template.id, "ssh_session_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("ssh_session".into())
    );
    assert!(matches!(
        binding.template.reason_profile.as_ref().unwrap(),
        ReasonProfile::Declarative(_)
    ));
}

#[test]
fn built_in_socks5_session_path_dsl_compiles_into_template_binding() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/socks5_session_path.gewy").unwrap();

    assert_eq!(binding.template.id, "socks5_session_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("socks5_session".into())
    );
    assert!(matches!(
        binding.template.reason_profile.as_ref().unwrap(),
        ReasonProfile::Declarative(_)
    ));
}

#[test]
fn built_in_socks5_denied_path_dsl_compiles_into_template_binding() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/socks5_denied_path.gewy").unwrap();

    assert_eq!(binding.template.id, "socks5_denied_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("socks5_denied".into())
    );
    assert!(matches!(
        binding.template.reason_profile.as_ref().unwrap(),
        ReasonProfile::Declarative(_)
    ));
}

#[test]
fn built_in_http_connect_tunnel_path_dsl_compiles_into_template_binding() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/http_connect_tunnel_path.gewy")
            .unwrap();

    assert_eq!(binding.template.id, "http_connect_tunnel_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("http_connect_tunnel".into())
    );
    assert!(matches!(
        binding.template.reason_profile.as_ref().unwrap(),
        ReasonProfile::Declarative(_)
    ));
}

#[test]
fn built_in_http_connect_denied_path_dsl_compiles_into_template_binding() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/http_connect_denied_path.gewy")
            .unwrap();

    assert_eq!(binding.template.id, "http_connect_denied_path");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::Custom("http_connect_denied".into())
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
            && finding.network_module_kind == "http_request_response"
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
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        4,
        803,
        220,
        PacketDir::Ingress,
        Some(42310),
        Some(443),
        Some(0xe0),
        None,
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
fn quic_crypto_handshake_path_materializes_quic_crypto_stages() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/quic_crypto_handshake_path.gewy")
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
        Some(0xc3),
        Some(0xc300),
    ));
    session.ingest(udp_quic_meta_fact(
        4,
        804,
        PacketDir::Egress,
        Some(42310),
        Some(443),
        true,
        Some(gewyvern::ir::QuicPacketType::Initial),
        vec![gewyvern::ir::QuicFrameType::Crypto],
    ));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        5,
        804,
        220,
        PacketDir::Ingress,
        Some(42310),
        Some(443),
        Some(0xe0),
        None,
    ));
    session.ingest(udp_quic_meta_fact(
        6,
        804,
        PacketDir::Ingress,
        Some(42310),
        Some(443),
        true,
        Some(gewyvern::ir::QuicPacketType::Handshake),
        vec![gewyvern::ir::QuicFrameType::Crypto],
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(80));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("quic_crypto_handshake".into())
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("send_crypto"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_crypto"))
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
fn quic_crypto_handshake_path_does_not_treat_non_crypto_frames_as_crypto() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/quic_crypto_handshake_path.gewy")
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
        Some(443),
        Some(0xc3),
        Some(0xc300),
    ));
    session.ingest(udp_quic_meta_fact(
        4,
        805,
        PacketDir::Egress,
        Some(42310),
        Some(443),
        true,
        Some(gewyvern::ir::QuicPacketType::Initial),
        vec![gewyvern::ir::QuicFrameType::Ack],
    ));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        5,
        805,
        220,
        PacketDir::Ingress,
        Some(42310),
        Some(443),
        Some(0xe0),
        None,
    ));
    session.ingest(udp_quic_meta_fact(
        6,
        805,
        PacketDir::Ingress,
        Some(42310),
        Some(443),
        true,
        Some(gewyvern::ir::QuicPacketType::Handshake),
        vec![gewyvern::ir::QuicFrameType::Ack],
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(80));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .all(|stage| stage.phase.as_deref() != Some("send_crypto"))
    );
}

#[test]
fn quic_stream_session_path_materializes_stream_and_close_stages() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/quic_stream_session_path.gewy")
            .unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 806, 4242, "curl"));
    session.ingest(route_fact(2, 806, 7));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        3,
        806,
        1280,
        PacketDir::Egress,
        Some(42310),
        Some(443),
        Some(0xc3),
        Some(0xc300),
    ));
    session.ingest(udp_quic_meta_fact(
        4,
        806,
        PacketDir::Egress,
        Some(42310),
        Some(443),
        true,
        Some(gewyvern::ir::QuicPacketType::Initial),
        vec![gewyvern::ir::QuicFrameType::Crypto],
    ));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        5,
        806,
        220,
        PacketDir::Ingress,
        Some(42310),
        Some(443),
        Some(0xe0),
        None,
    ));
    session.ingest(udp_quic_meta_fact(
        6,
        806,
        PacketDir::Ingress,
        Some(42310),
        Some(443),
        true,
        Some(gewyvern::ir::QuicPacketType::Handshake),
        vec![gewyvern::ir::QuicFrameType::Crypto],
    ));
    session.ingest(udp_quic_meta_fact(
        7,
        806,
        PacketDir::Egress,
        Some(42310),
        Some(443),
        false,
        None,
        vec![gewyvern::ir::QuicFrameType::Stream],
    ));
    session.ingest(udp_quic_meta_fact(
        8,
        806,
        PacketDir::Ingress,
        Some(42310),
        Some(443),
        false,
        None,
        vec![gewyvern::ir::QuicFrameType::ConnectionClose],
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(100));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("quic_stream_session".into())
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("send_stream"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_close"))
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
fn quic_stream_session_path_does_not_treat_ack_as_stream() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/quic_stream_session_path.gewy")
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
        Some(0xc3),
        Some(0xc300),
    ));
    session.ingest(udp_quic_meta_fact(
        4,
        807,
        PacketDir::Egress,
        Some(42310),
        Some(443),
        true,
        Some(gewyvern::ir::QuicPacketType::Initial),
        vec![gewyvern::ir::QuicFrameType::Crypto],
    ));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        5,
        807,
        220,
        PacketDir::Ingress,
        Some(42310),
        Some(443),
        Some(0xe0),
        None,
    ));
    session.ingest(udp_quic_meta_fact(
        6,
        807,
        PacketDir::Ingress,
        Some(42310),
        Some(443),
        true,
        Some(gewyvern::ir::QuicPacketType::Handshake),
        vec![gewyvern::ir::QuicFrameType::Crypto],
    ));
    session.ingest(udp_quic_meta_fact(
        7,
        807,
        PacketDir::Egress,
        Some(42310),
        Some(443),
        false,
        None,
        vec![gewyvern::ir::QuicFrameType::Ack],
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(90));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .all(|stage| stage.phase.as_deref() != Some("send_stream"))
    );
}

#[test]
fn quic_bidi_stream_path_materializes_request_response_and_close_stages() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/quic_bidi_stream_path.gewy").unwrap();
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
        Some(0xc300),
    ));
    session.ingest(udp_quic_meta_fact(
        4,
        808,
        PacketDir::Egress,
        Some(42310),
        Some(443),
        true,
        Some(gewyvern::ir::QuicPacketType::Initial),
        vec![gewyvern::ir::QuicFrameType::Crypto],
    ));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        5,
        808,
        220,
        PacketDir::Ingress,
        Some(42310),
        Some(443),
        Some(0xe0),
        None,
    ));
    session.ingest(udp_quic_meta_fact(
        6,
        808,
        PacketDir::Ingress,
        Some(42310),
        Some(443),
        true,
        Some(gewyvern::ir::QuicPacketType::Handshake),
        vec![gewyvern::ir::QuicFrameType::Crypto],
    ));
    session.ingest(udp_quic_meta_fact(
        7,
        808,
        PacketDir::Egress,
        Some(42310),
        Some(443),
        false,
        None,
        vec![gewyvern::ir::QuicFrameType::Stream],
    ));
    session.ingest(udp_quic_meta_fact(
        8,
        808,
        PacketDir::Ingress,
        Some(42310),
        Some(443),
        false,
        None,
        vec![gewyvern::ir::QuicFrameType::Stream],
    ));
    session.ingest(udp_quic_meta_fact(
        9,
        808,
        PacketDir::Ingress,
        Some(42310),
        Some(443),
        false,
        None,
        vec![gewyvern::ir::QuicFrameType::ConnectionClose],
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(110));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("quic_bidi_stream".into())
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("send_request_stream"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_response_stream"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_close"))
    );
    assert_eq!(export.module_findings.len(), 0);
}

#[test]
fn quic_bidi_stream_path_does_not_treat_close_as_response_stream() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/quic_bidi_stream_path.gewy").unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 809, 4242, "curl"));
    session.ingest(route_fact(2, 809, 7));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        3,
        809,
        1280,
        PacketDir::Egress,
        Some(42310),
        Some(443),
        Some(0xc3),
        Some(0xc300),
    ));
    session.ingest(udp_quic_meta_fact(
        4,
        809,
        PacketDir::Egress,
        Some(42310),
        Some(443),
        true,
        Some(gewyvern::ir::QuicPacketType::Initial),
        vec![gewyvern::ir::QuicFrameType::Crypto],
    ));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        5,
        809,
        220,
        PacketDir::Ingress,
        Some(42310),
        Some(443),
        Some(0xe0),
        None,
    ));
    session.ingest(udp_quic_meta_fact(
        6,
        809,
        PacketDir::Ingress,
        Some(42310),
        Some(443),
        true,
        Some(gewyvern::ir::QuicPacketType::Handshake),
        vec![gewyvern::ir::QuicFrameType::Crypto],
    ));
    session.ingest(udp_quic_meta_fact(
        7,
        809,
        PacketDir::Egress,
        Some(42310),
        Some(443),
        false,
        None,
        vec![gewyvern::ir::QuicFrameType::Stream],
    ));
    session.ingest(udp_quic_meta_fact(
        8,
        809,
        PacketDir::Ingress,
        Some(42310),
        Some(443),
        false,
        None,
        vec![gewyvern::ir::QuicFrameType::ConnectionClose],
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(100));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .all(|stage| stage.phase.as_deref() != Some("receive_response_stream"))
    );
}

#[test]
fn http3_request_path_materializes_request_response_and_close_stages() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/http3_request_path.gewy").unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 810, 4242, "curl"));
    session.ingest(route_fact(2, 810, 7));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        3,
        810,
        1280,
        PacketDir::Egress,
        Some(42310),
        Some(443),
        Some(0xc3),
        Some(0xc300),
    ));
    session.ingest(udp_quic_meta_fact(
        4,
        810,
        PacketDir::Egress,
        Some(42310),
        Some(443),
        true,
        Some(gewyvern::ir::QuicPacketType::Initial),
        vec![gewyvern::ir::QuicFrameType::Crypto],
    ));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        5,
        810,
        220,
        PacketDir::Ingress,
        Some(42310),
        Some(443),
        Some(0xe0),
        None,
    ));
    session.ingest(udp_quic_meta_fact(
        6,
        810,
        PacketDir::Ingress,
        Some(42310),
        Some(443),
        true,
        Some(gewyvern::ir::QuicPacketType::Handshake),
        vec![gewyvern::ir::QuicFrameType::Crypto],
    ));
    session.ingest(udp_quic_meta_fact(
        7,
        810,
        PacketDir::Egress,
        Some(42310),
        Some(443),
        false,
        None,
        vec![gewyvern::ir::QuicFrameType::Stream],
    ));
    session.ingest(udp_quic_meta_fact(
        8,
        810,
        PacketDir::Ingress,
        Some(42310),
        Some(443),
        false,
        None,
        vec![gewyvern::ir::QuicFrameType::Stream],
    ));
    session.ingest(udp_quic_meta_fact(
        9,
        810,
        PacketDir::Ingress,
        Some(42310),
        Some(443),
        false,
        None,
        vec![gewyvern::ir::QuicFrameType::ConnectionClose],
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(110));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("http3_request".into())
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("send_request_stream"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_response_stream"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_close"))
    );
    assert_eq!(export.module_findings.len(), 0);
}

#[test]
fn http3_request_path_does_not_treat_close_as_response_stream() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/http3_request_path.gewy").unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 811, 4242, "curl"));
    session.ingest(route_fact(2, 811, 7));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        3,
        811,
        1280,
        PacketDir::Egress,
        Some(42310),
        Some(443),
        Some(0xc3),
        Some(0xc300),
    ));
    session.ingest(udp_quic_meta_fact(
        4,
        811,
        PacketDir::Egress,
        Some(42310),
        Some(443),
        true,
        Some(gewyvern::ir::QuicPacketType::Initial),
        vec![gewyvern::ir::QuicFrameType::Crypto],
    ));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        5,
        811,
        220,
        PacketDir::Ingress,
        Some(42310),
        Some(443),
        Some(0xe0),
        None,
    ));
    session.ingest(udp_quic_meta_fact(
        6,
        811,
        PacketDir::Ingress,
        Some(42310),
        Some(443),
        true,
        Some(gewyvern::ir::QuicPacketType::Handshake),
        vec![gewyvern::ir::QuicFrameType::Crypto],
    ));
    session.ingest(udp_quic_meta_fact(
        7,
        811,
        PacketDir::Egress,
        Some(42310),
        Some(443),
        false,
        None,
        vec![gewyvern::ir::QuicFrameType::Stream],
    ));
    session.ingest(udp_quic_meta_fact(
        8,
        811,
        PacketDir::Ingress,
        Some(42310),
        Some(443),
        false,
        None,
        vec![gewyvern::ir::QuicFrameType::ConnectionClose],
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(100));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .all(|stage| stage.phase.as_deref() != Some("receive_response_stream"))
    );
}

#[test]
fn http3_server_response_path_materializes_request_response_and_close_stages() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/http3_server_response_path.gewy")
            .unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 812, 8080, "nginx"));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        2,
        812,
        1280,
        PacketDir::Ingress,
        Some(443),
        Some(53000),
        Some(0xc3),
        Some(0xc300),
    ));
    session.ingest(udp_quic_meta_fact(
        3,
        812,
        PacketDir::Ingress,
        Some(443),
        Some(53000),
        true,
        Some(gewyvern::ir::QuicPacketType::Initial),
        vec![gewyvern::ir::QuicFrameType::Crypto],
    ));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        4,
        812,
        220,
        PacketDir::Egress,
        Some(443),
        Some(53000),
        Some(0xe0),
        None,
    ));
    session.ingest(udp_quic_meta_fact(
        5,
        812,
        PacketDir::Egress,
        Some(443),
        Some(53000),
        true,
        Some(gewyvern::ir::QuicPacketType::Handshake),
        vec![gewyvern::ir::QuicFrameType::Crypto],
    ));
    session.ingest(udp_quic_meta_fact(
        6,
        812,
        PacketDir::Ingress,
        Some(443),
        Some(53000),
        false,
        None,
        vec![gewyvern::ir::QuicFrameType::Stream],
    ));
    session.ingest(udp_quic_meta_fact(
        7,
        812,
        PacketDir::Egress,
        Some(443),
        Some(53000),
        false,
        None,
        vec![gewyvern::ir::QuicFrameType::Stream],
    ));
    session.ingest(udp_quic_meta_fact(
        8,
        812,
        PacketDir::Egress,
        Some(443),
        Some(53000),
        false,
        None,
        vec![gewyvern::ir::QuicFrameType::ConnectionClose],
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(100));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("http3_server_response".into())
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_request_stream"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("send_response_stream"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("send_close"))
    );
    assert_eq!(export.module_findings.len(), 0);
}

#[test]
fn http3_server_response_path_does_not_treat_close_as_request_stream() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/http3_server_response_path.gewy")
            .unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 813, 8080, "nginx"));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        2,
        813,
        1280,
        PacketDir::Ingress,
        Some(443),
        Some(53000),
        Some(0xc3),
        Some(0xc300),
    ));
    session.ingest(udp_quic_meta_fact(
        3,
        813,
        PacketDir::Ingress,
        Some(443),
        Some(53000),
        true,
        Some(gewyvern::ir::QuicPacketType::Initial),
        vec![gewyvern::ir::QuicFrameType::Crypto],
    ));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        4,
        813,
        220,
        PacketDir::Egress,
        Some(443),
        Some(53000),
        Some(0xe0),
        None,
    ));
    session.ingest(udp_quic_meta_fact(
        5,
        813,
        PacketDir::Egress,
        Some(443),
        Some(53000),
        true,
        Some(gewyvern::ir::QuicPacketType::Handshake),
        vec![gewyvern::ir::QuicFrameType::Crypto],
    ));
    session.ingest(udp_quic_meta_fact(
        6,
        813,
        PacketDir::Egress,
        Some(443),
        Some(53000),
        false,
        None,
        vec![gewyvern::ir::QuicFrameType::ConnectionClose],
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(90));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .all(|stage| stage.phase.as_deref() != Some("receive_request_stream"))
    );
}

#[test]
fn hy2_auth_path_materializes_auth_request_and_ok_stages() {
    let binding = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/hy2_auth_path.gewy").unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 814, 4242, "hysteria"));
    session.ingest(route_fact(2, 814, 7));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        3,
        814,
        1280,
        PacketDir::Egress,
        Some(42310),
        Some(443),
        Some(0xc3),
        Some(0xc300),
    ));
    session.ingest(udp_quic_meta_fact(
        4,
        814,
        PacketDir::Egress,
        Some(42310),
        Some(443),
        true,
        Some(gewyvern::ir::QuicPacketType::Initial),
        vec![gewyvern::ir::QuicFrameType::Crypto],
    ));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        5,
        814,
        220,
        PacketDir::Ingress,
        Some(42310),
        Some(443),
        Some(0xe0),
        None,
    ));
    session.ingest(udp_quic_meta_fact(
        6,
        814,
        PacketDir::Ingress,
        Some(42310),
        Some(443),
        true,
        Some(gewyvern::ir::QuicPacketType::Handshake),
        vec![gewyvern::ir::QuicFrameType::Crypto],
    ));
    session.ingest(udp_quic_meta_fact(
        7,
        814,
        PacketDir::Egress,
        Some(42310),
        Some(443),
        false,
        None,
        vec![gewyvern::ir::QuicFrameType::Stream],
    ));
    session.ingest(udp_quic_meta_fact(
        8,
        814,
        PacketDir::Ingress,
        Some(42310),
        Some(443),
        false,
        None,
        vec![gewyvern::ir::QuicFrameType::Stream],
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(100));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("hy2_auth".into())
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("send_auth_request_stream"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_auth_ok_stream"))
    );
    assert_eq!(export.module_findings.len(), 0);
}

#[test]
fn hy2_auth_path_does_not_treat_close_as_auth_ok_stream() {
    let binding = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/hy2_auth_path.gewy").unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 815, 4242, "hysteria"));
    session.ingest(route_fact(2, 815, 7));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        3,
        815,
        1280,
        PacketDir::Egress,
        Some(42310),
        Some(443),
        Some(0xc3),
        Some(0xc300),
    ));
    session.ingest(udp_quic_meta_fact(
        4,
        815,
        PacketDir::Egress,
        Some(42310),
        Some(443),
        true,
        Some(gewyvern::ir::QuicPacketType::Initial),
        vec![gewyvern::ir::QuicFrameType::Crypto],
    ));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        5,
        815,
        220,
        PacketDir::Ingress,
        Some(42310),
        Some(443),
        Some(0xe0),
        None,
    ));
    session.ingest(udp_quic_meta_fact(
        6,
        815,
        PacketDir::Ingress,
        Some(42310),
        Some(443),
        true,
        Some(gewyvern::ir::QuicPacketType::Handshake),
        vec![gewyvern::ir::QuicFrameType::Crypto],
    ));
    session.ingest(udp_quic_meta_fact(
        7,
        815,
        PacketDir::Egress,
        Some(42310),
        Some(443),
        false,
        None,
        vec![gewyvern::ir::QuicFrameType::Stream],
    ));
    session.ingest(udp_quic_meta_fact(
        8,
        815,
        PacketDir::Ingress,
        Some(42310),
        Some(443),
        false,
        None,
        vec![gewyvern::ir::QuicFrameType::ConnectionClose],
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(100));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .all(|stage| stage.phase.as_deref() != Some("receive_auth_ok_stream"))
    );
}

#[test]
fn hy2_auth_operation_maps_to_proxy_authentication_module_kind() {
    assert_eq!(
        gewyvern::flow::infer_network_module_kind(
            &ProgramOperation::Custom("hy2_auth".into()),
            Some("receive_auth_ok_stream"),
            Some("send_auth_request_stream->receive_auth_ok_stream"),
            "transport_io",
        ),
        "proxy_authentication"
    );
}

#[test]
fn ssh_session_operation_maps_to_remote_access_session_module_kind() {
    assert_eq!(
        gewyvern::flow::infer_network_module_kind(
            &ProgramOperation::Custom("ssh_session".into()),
            Some("send_key_exchange_init"),
            Some("receive_server_banner->send_key_exchange_init"),
            "transport_io",
        ),
        "remote_access_session"
    );
}

#[test]
fn socks5_session_operation_maps_to_proxy_negotiation_module_kind() {
    assert_eq!(
        gewyvern::flow::infer_network_module_kind(
            &ProgramOperation::Custom("socks5_session".into()),
            Some("receive_connect_success"),
            Some("send_connect_request->receive_connect_success"),
            "transport_io",
        ),
        "proxy_negotiation"
    );
}

#[test]
fn http_connect_tunnel_operation_maps_to_proxy_tunnel_establishment_module_kind() {
    assert_eq!(
        gewyvern::flow::infer_network_module_kind(
            &ProgramOperation::Custom("http_connect_tunnel".into()),
            Some("receive_connect_established"),
            Some("send_connect_request->receive_connect_established"),
            "transport_io",
        ),
        "proxy_tunnel_establishment"
    );
}

#[test]
fn hy2_udp_relay_path_materializes_auth_and_datagram_stages() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/hy2_udp_relay_path.gewy").unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 816, 4242, "hysteria"));
    session.ingest(route_fact(2, 816, 7));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        3,
        816,
        1280,
        PacketDir::Egress,
        Some(42310),
        Some(443),
        Some(0xc3),
        Some(0xc300),
    ));
    session.ingest(udp_quic_meta_fact(
        4,
        816,
        PacketDir::Egress,
        Some(42310),
        Some(443),
        true,
        Some(gewyvern::ir::QuicPacketType::Initial),
        vec![gewyvern::ir::QuicFrameType::Crypto],
    ));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        5,
        816,
        220,
        PacketDir::Ingress,
        Some(42310),
        Some(443),
        Some(0xe0),
        None,
    ));
    session.ingest(udp_quic_meta_fact(
        6,
        816,
        PacketDir::Ingress,
        Some(42310),
        Some(443),
        true,
        Some(gewyvern::ir::QuicPacketType::Handshake),
        vec![gewyvern::ir::QuicFrameType::Crypto],
    ));
    session.ingest(udp_quic_meta_fact(
        7,
        816,
        PacketDir::Egress,
        Some(42310),
        Some(443),
        false,
        None,
        vec![gewyvern::ir::QuicFrameType::Stream],
    ));
    session.ingest(udp_quic_meta_fact(
        8,
        816,
        PacketDir::Ingress,
        Some(42310),
        Some(443),
        false,
        None,
        vec![gewyvern::ir::QuicFrameType::Stream],
    ));
    session.ingest(udp_quic_meta_fact(
        9,
        816,
        PacketDir::Egress,
        Some(42310),
        Some(443),
        false,
        None,
        vec![gewyvern::ir::QuicFrameType::Datagram],
    ));
    session.ingest(udp_quic_meta_fact(
        10,
        816,
        PacketDir::Ingress,
        Some(42310),
        Some(443),
        false,
        None,
        vec![gewyvern::ir::QuicFrameType::Datagram],
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(120));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("hy2_udp_relay".into())
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("send_udp_relay_datagram"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_udp_relay_datagram"))
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
fn hy2_udp_relay_path_does_not_treat_stream_as_udp_datagram() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/hy2_udp_relay_path.gewy").unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 817, 4242, "hysteria"));
    session.ingest(route_fact(2, 817, 7));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        3,
        817,
        1280,
        PacketDir::Egress,
        Some(42310),
        Some(443),
        Some(0xc3),
        Some(0xc300),
    ));
    session.ingest(udp_quic_meta_fact(
        4,
        817,
        PacketDir::Egress,
        Some(42310),
        Some(443),
        true,
        Some(gewyvern::ir::QuicPacketType::Initial),
        vec![gewyvern::ir::QuicFrameType::Crypto],
    ));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        5,
        817,
        220,
        PacketDir::Ingress,
        Some(42310),
        Some(443),
        Some(0xe0),
        None,
    ));
    session.ingest(udp_quic_meta_fact(
        6,
        817,
        PacketDir::Ingress,
        Some(42310),
        Some(443),
        true,
        Some(gewyvern::ir::QuicPacketType::Handshake),
        vec![gewyvern::ir::QuicFrameType::Crypto],
    ));
    session.ingest(udp_quic_meta_fact(
        7,
        817,
        PacketDir::Egress,
        Some(42310),
        Some(443),
        false,
        None,
        vec![gewyvern::ir::QuicFrameType::Stream],
    ));
    session.ingest(udp_quic_meta_fact(
        8,
        817,
        PacketDir::Ingress,
        Some(42310),
        Some(443),
        false,
        None,
        vec![gewyvern::ir::QuicFrameType::Stream],
    ));
    session.ingest(udp_quic_meta_fact(
        9,
        817,
        PacketDir::Ingress,
        Some(42310),
        Some(443),
        false,
        None,
        vec![gewyvern::ir::QuicFrameType::Stream],
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(110));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .all(|stage| stage.phase.as_deref() != Some("receive_udp_relay_datagram"))
    );
}

#[test]
fn hy2_tcp_relay_path_materializes_auth_and_tcp_stream_stages() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/hy2_tcp_relay_path.gewy").unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 818, 4242, "hysteria"));
    session.ingest(route_fact(2, 818, 7));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        3,
        818,
        1280,
        PacketDir::Egress,
        Some(42310),
        Some(443),
        Some(0xc3),
        Some(0xc300),
    ));
    session.ingest(udp_quic_meta_fact(
        4,
        818,
        PacketDir::Egress,
        Some(42310),
        Some(443),
        true,
        Some(gewyvern::ir::QuicPacketType::Initial),
        vec![gewyvern::ir::QuicFrameType::Crypto],
    ));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        5,
        818,
        220,
        PacketDir::Ingress,
        Some(42310),
        Some(443),
        Some(0xe0),
        None,
    ));
    session.ingest(udp_quic_meta_fact(
        6,
        818,
        PacketDir::Ingress,
        Some(42310),
        Some(443),
        true,
        Some(gewyvern::ir::QuicPacketType::Handshake),
        vec![gewyvern::ir::QuicFrameType::Crypto],
    ));
    session.ingest(udp_quic_meta_fact(
        7,
        818,
        PacketDir::Egress,
        Some(42310),
        Some(443),
        false,
        None,
        vec![gewyvern::ir::QuicFrameType::Stream],
    ));
    session.ingest(udp_quic_meta_fact(
        8,
        818,
        PacketDir::Ingress,
        Some(42310),
        Some(443),
        false,
        None,
        vec![gewyvern::ir::QuicFrameType::Stream],
    ));
    session.ingest(udp_quic_meta_fact_with_payload_bytes(
        9,
        818,
        PacketDir::Egress,
        Some(42310),
        Some(443),
        false,
        None,
        vec![gewyvern::ir::QuicFrameType::Stream],
        &[(0, 0x44), (1, 0x01)],
    ));
    session.ingest(udp_quic_meta_fact_with_payload_bytes(
        10,
        818,
        PacketDir::Ingress,
        Some(42310),
        Some(443),
        false,
        None,
        vec![gewyvern::ir::QuicFrameType::Stream],
        &[(0, 0x00)],
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(130));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("hy2_tcp_relay".into())
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("send_tcp_request_stream"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_tcp_response_stream"))
    );
    assert_eq!(export.module_findings.len(), 0);
}

#[test]
fn hy2_tcp_relay_path_does_not_treat_auth_stream_as_tcp_request_stream() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/hy2_tcp_relay_path.gewy").unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 819, 4242, "hysteria"));
    session.ingest(route_fact(2, 819, 7));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        3,
        819,
        1280,
        PacketDir::Egress,
        Some(42310),
        Some(443),
        Some(0xc3),
        Some(0xc300),
    ));
    session.ingest(udp_quic_meta_fact(
        4,
        819,
        PacketDir::Egress,
        Some(42310),
        Some(443),
        true,
        Some(gewyvern::ir::QuicPacketType::Initial),
        vec![gewyvern::ir::QuicFrameType::Crypto],
    ));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        5,
        819,
        220,
        PacketDir::Ingress,
        Some(42310),
        Some(443),
        Some(0xe0),
        None,
    ));
    session.ingest(udp_quic_meta_fact(
        6,
        819,
        PacketDir::Ingress,
        Some(42310),
        Some(443),
        true,
        Some(gewyvern::ir::QuicPacketType::Handshake),
        vec![gewyvern::ir::QuicFrameType::Crypto],
    ));
    session.ingest(udp_quic_meta_fact(
        7,
        819,
        PacketDir::Egress,
        Some(42310),
        Some(443),
        false,
        None,
        vec![gewyvern::ir::QuicFrameType::Stream],
    ));
    session.ingest(udp_quic_meta_fact(
        8,
        819,
        PacketDir::Ingress,
        Some(42310),
        Some(443),
        false,
        None,
        vec![gewyvern::ir::QuicFrameType::Stream],
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(100));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .all(|stage| stage.phase.as_deref() != Some("send_tcp_request_stream"))
    );
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
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        4,
        805,
        220,
        PacketDir::Ingress,
        Some(42310),
        Some(53),
        Some(0xe0),
        None,
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
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        4,
        806,
        220,
        PacketDir::Ingress,
        Some(42310),
        Some(443),
        Some(0xe0),
        None,
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
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        4,
        807,
        220,
        PacketDir::Ingress,
        Some(42310),
        Some(443),
        Some(0xe0),
        None,
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
fn quic_client_initial_path_does_not_treat_wrong_quic_packet_type_as_initial() {
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
        Some(0xd0),
        None,
    ));
    session.ingest(udp_packet_fact_with_dir_and_ports_and_payload(
        4,
        808,
        220,
        PacketDir::Ingress,
        Some(42310),
        Some(443),
        Some(0xe0),
        None,
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
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/gtpu_echo_path.gewy").unwrap();
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
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/gtpu_echo_path.gewy").unwrap();
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
fn ssh_session_path_materializes_banner_and_key_exchange_phases() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/ssh_session_path.gewy").unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 8281, 53022, "ssh-client"));
    session.ingest(route_fact(2, 8281, 7));
    session.ingest(tcp_state_fact_with_ports(3, 8281, 1, 2, 53022, 22));
    session.ingest(packet_fact_with_dir_and_payload(
        4,
        8281,
        0x18,
        PacketDir::Ingress,
        Some(53022),
        Some(22),
        Some(0x53),
        Some(0x5353),
        Some(0x5353482d),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        5,
        8281,
        0x18,
        PacketDir::Egress,
        Some(53022),
        Some(22),
        Some(0x53),
        Some(0x5353),
        Some(0x5353482d),
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_bytes4_and5(
        6,
        8281,
        0x18,
        PacketDir::Egress,
        Some(53022),
        Some(22),
        Some(0x00),
        None,
        None,
        Some(0x10),
        Some(0x14),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(80));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("ssh_session".into())
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
            .any(|stage| stage.phase.as_deref() == Some("receive_server_banner"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("send_client_banner"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("send_key_exchange_init"))
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
fn ssh_session_path_does_not_treat_wrong_message_code_as_key_exchange_init() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/ssh_session_path.gewy").unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 8282, 53022, "ssh-client"));
    session.ingest(route_fact(2, 8282, 7));
    session.ingest(tcp_state_fact_with_ports(3, 8282, 1, 2, 53022, 22));
    session.ingest(packet_fact_with_dir_and_payload(
        4,
        8282,
        0x18,
        PacketDir::Ingress,
        Some(53022),
        Some(22),
        Some(0x53),
        Some(0x5353),
        Some(0x5353482d),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        5,
        8282,
        0x18,
        PacketDir::Egress,
        Some(53022),
        Some(22),
        Some(0x53),
        Some(0x5353),
        Some(0x5353482d),
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_bytes4_and5(
        6,
        8282,
        0x18,
        PacketDir::Egress,
        Some(53022),
        Some(22),
        Some(0x00),
        None,
        None,
        Some(0x10),
        Some(0x15),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(80));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .all(|stage| stage.phase.as_deref() != Some("send_key_exchange_init"))
    );
}

#[test]
fn socks5_session_path_materializes_method_and_connect_phases() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/socks5_session_path.gewy").unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 8283, 53180, "proxy-client"));
    session.ingest(route_fact(2, 8283, 7));
    session.ingest(tcp_state_fact_with_ports(3, 8283, 1, 2, 53180, 1080));
    session.ingest(packet_fact_with_dir_and_payload(
        4,
        8283,
        0x18,
        PacketDir::Egress,
        Some(53180),
        Some(1080),
        Some(0x05),
        Some(0x0501),
        None,
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        5,
        8283,
        0x18,
        PacketDir::Ingress,
        Some(53180),
        Some(1080),
        Some(0x05),
        Some(0x0500),
        None,
    ));
    session.ingest(packet_fact_with_dir_and_payload_bytes(
        6,
        8283,
        0x18,
        PacketDir::Egress,
        Some(53180),
        Some(1080),
        &[(0, 0x05), (1, 0x01), (2, 0x00), (3, 0x03)],
    ));
    session.ingest(packet_fact_with_dir_and_payload_bytes(
        7,
        8283,
        0x18,
        PacketDir::Ingress,
        Some(53180),
        Some(1080),
        &[(0, 0x05), (1, 0x00), (2, 0x00), (3, 0x01)],
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(90));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("socks5_session".into())
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
            .any(|stage| stage.phase.as_deref() == Some("send_method_greeting"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_method_selection"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("send_connect_request"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_connect_success"))
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
fn socks5_session_path_does_not_treat_failed_reply_as_connect_success() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/socks5_session_path.gewy").unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 8284, 53180, "proxy-client"));
    session.ingest(route_fact(2, 8284, 7));
    session.ingest(tcp_state_fact_with_ports(3, 8284, 1, 2, 53180, 1080));
    session.ingest(packet_fact_with_dir_and_payload(
        4,
        8284,
        0x18,
        PacketDir::Egress,
        Some(53180),
        Some(1080),
        Some(0x05),
        Some(0x0501),
        None,
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        5,
        8284,
        0x18,
        PacketDir::Ingress,
        Some(53180),
        Some(1080),
        Some(0x05),
        Some(0x0500),
        None,
    ));
    session.ingest(packet_fact_with_dir_and_payload_bytes(
        6,
        8284,
        0x18,
        PacketDir::Egress,
        Some(53180),
        Some(1080),
        &[(0, 0x05), (1, 0x01), (2, 0x00), (3, 0x03)],
    ));
    session.ingest(packet_fact_with_dir_and_payload_bytes(
        7,
        8284,
        0x18,
        PacketDir::Ingress,
        Some(53180),
        Some(1080),
        &[(0, 0x05), (1, 0x05), (2, 0x00), (3, 0x01)],
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(90));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .all(|stage| stage.phase.as_deref() != Some("receive_connect_success"))
    );
}

#[test]
fn socks5_denied_path_materializes_denied_connect_phase() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/socks5_denied_path.gewy").unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 8287, 53182, "proxy-client"));
    session.ingest(route_fact(2, 8287, 7));
    session.ingest(tcp_state_fact_with_ports(3, 8287, 1, 2, 53182, 1080));
    session.ingest(packet_fact_with_dir_and_payload(
        4,
        8287,
        0x18,
        PacketDir::Egress,
        Some(53182),
        Some(1080),
        Some(0x05),
        Some(0x0501),
        None,
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        5,
        8287,
        0x18,
        PacketDir::Ingress,
        Some(53182),
        Some(1080),
        Some(0x05),
        Some(0x0500),
        None,
    ));
    session.ingest(packet_fact_with_dir_and_payload_bytes(
        6,
        8287,
        0x18,
        PacketDir::Egress,
        Some(53182),
        Some(1080),
        &[(0, 0x05), (1, 0x01), (2, 0x00), (3, 0x03)],
    ));
    session.ingest(packet_fact_with_dir_and_payload_bytes(
        7,
        8287,
        0x18,
        PacketDir::Ingress,
        Some(53182),
        Some(1080),
        &[(0, 0x05), (1, 0x05), (2, 0x00), (3, 0x01)],
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(90));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("socks5_denied".into())
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("send_connect_request"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_connect_denied"))
    );
    assert_eq!(export.module_findings.len(), 1);
}

#[test]
fn socks5_denied_path_does_not_match_success_reply() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/socks5_denied_path.gewy").unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 8288, 53182, "proxy-client"));
    session.ingest(route_fact(2, 8288, 7));
    session.ingest(tcp_state_fact_with_ports(3, 8288, 1, 2, 53182, 1080));
    session.ingest(packet_fact_with_dir_and_payload(
        4,
        8288,
        0x18,
        PacketDir::Egress,
        Some(53182),
        Some(1080),
        Some(0x05),
        Some(0x0501),
        None,
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        5,
        8288,
        0x18,
        PacketDir::Ingress,
        Some(53182),
        Some(1080),
        Some(0x05),
        Some(0x0500),
        None,
    ));
    session.ingest(packet_fact_with_dir_and_payload_bytes(
        6,
        8288,
        0x18,
        PacketDir::Egress,
        Some(53182),
        Some(1080),
        &[(0, 0x05), (1, 0x01), (2, 0x00), (3, 0x03)],
    ));
    session.ingest(packet_fact_with_dir_and_payload_bytes(
        7,
        8288,
        0x18,
        PacketDir::Ingress,
        Some(53182),
        Some(1080),
        &[(0, 0x05), (1, 0x00), (2, 0x00), (3, 0x01)],
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(90));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .all(|stage| stage.phase.as_deref() != Some("receive_connect_denied"))
    );
}

#[test]
fn http_connect_tunnel_path_materializes_connect_request_and_established_response() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/http_connect_tunnel_path.gewy")
            .unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 8285, 53181, "proxy-client"));
    session.ingest(route_fact(2, 8285, 7));
    session.ingest(tcp_state_fact_with_ports(3, 8285, 1, 2, 53181, 8080));
    session.ingest(packet_fact_with_dir_and_payload(
        4,
        8285,
        0x18,
        PacketDir::Egress,
        Some(53181),
        Some(8080),
        Some(0x43),
        Some(0x434f),
        Some(0x434f4e4e),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        5,
        8285,
        0x18,
        PacketDir::Ingress,
        Some(53181),
        Some(8080),
        Some(0x32),
        Some(0x3230),
        Some(0x32303020),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(80));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("http_connect_tunnel".into())
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
            .any(|stage| stage.phase.as_deref() == Some("send_connect_request"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_connect_established"))
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
fn http_connect_tunnel_path_does_not_treat_non_200_response_as_established() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/http_connect_tunnel_path.gewy")
            .unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 8286, 53181, "proxy-client"));
    session.ingest(route_fact(2, 8286, 7));
    session.ingest(tcp_state_fact_with_ports(3, 8286, 1, 2, 53181, 8080));
    session.ingest(packet_fact_with_dir_and_payload(
        4,
        8286,
        0x18,
        PacketDir::Egress,
        Some(53181),
        Some(8080),
        Some(0x43),
        Some(0x434f),
        Some(0x434f4e4e),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        5,
        8286,
        0x18,
        PacketDir::Ingress,
        Some(53181),
        Some(8080),
        Some(0x34),
        Some(0x3430),
        Some(0x34303320),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(80));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .all(|stage| stage.phase.as_deref() != Some("receive_connect_established"))
    );
}

#[test]
fn http_connect_denied_path_materializes_denied_tunnel_phase() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/http_connect_denied_path.gewy")
            .unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 8289, 53183, "proxy-client"));
    session.ingest(route_fact(2, 8289, 7));
    session.ingest(tcp_state_fact_with_ports(3, 8289, 1, 2, 53183, 8080));
    session.ingest(packet_fact_with_dir_and_payload(
        4,
        8289,
        0x18,
        PacketDir::Egress,
        Some(53183),
        Some(8080),
        Some(0x43),
        Some(0x434f),
        Some(0x434f4e4e),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        5,
        8289,
        0x18,
        PacketDir::Ingress,
        Some(53183),
        Some(8080),
        Some(0x34),
        Some(0x3430),
        Some(0x34303320),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(80));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("http_connect_denied".into())
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("send_connect_request"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_connect_denied"))
    );
    assert_eq!(export.module_findings.len(), 0);
}

#[test]
fn http_connect_denied_path_does_not_match_200_response() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/http_connect_denied_path.gewy")
            .unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 8290, 53183, "proxy-client"));
    session.ingest(route_fact(2, 8290, 7));
    session.ingest(tcp_state_fact_with_ports(3, 8290, 1, 2, 53183, 8080));
    session.ingest(packet_fact_with_dir_and_payload(
        4,
        8290,
        0x18,
        PacketDir::Egress,
        Some(53183),
        Some(8080),
        Some(0x43),
        Some(0x434f),
        Some(0x434f4e4e),
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        5,
        8290,
        0x18,
        PacketDir::Ingress,
        Some(53183),
        Some(8080),
        Some(0x32),
        Some(0x3230),
        Some(0x32303020),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(80));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .all(|stage| stage.phase.as_deref() != Some("receive_connect_denied"))
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
fn postgres_auth_path_materializes_auth_password_and_ready_phases() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/postgres_auth_path.gewy").unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 508, 7783, "psql"));
    session.ingest(tcp_state_fact_with_ports(2, 508, 1, 2, 43127, 5432));
    session.ingest(tcp_state_fact_with_ports(3, 508, 2, 3, 43127, 5432));
    session.ingest(route_fact(4, 508, 6));
    session.ingest(packet_fact_with_dir_and_payload(
        5,
        508,
        0,
        PacketDir::Ingress,
        Some(43127),
        Some(5432),
        Some(0x52),
        None,
        None,
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        6,
        508,
        0,
        PacketDir::Egress,
        Some(43127),
        Some(5432),
        Some(0x70),
        None,
        None,
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        7,
        508,
        0,
        PacketDir::Ingress,
        Some(43127),
        Some(5432),
        Some(0x5a),
        None,
        None,
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(80));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("postgres_auth".into())
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_auth"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("send_password"))
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
    assert!(phase_kinds.contains(&"receive_payload".to_string()));
    assert!(phase_kinds.contains(&"emit_payload".to_string()));
    assert_eq!(export.module_findings.len(), 0);
}

#[test]
fn postgres_auth_path_does_not_match_wrong_auth_message_type() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/postgres_auth_path.gewy").unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 509, 7784, "psql"));
    session.ingest(tcp_state_fact_with_ports(2, 509, 1, 2, 43128, 5432));
    session.ingest(tcp_state_fact_with_ports(3, 509, 2, 3, 43128, 5432));
    session.ingest(route_fact(4, 509, 6));
    session.ingest(packet_fact_with_dir_and_payload(
        5,
        509,
        0,
        PacketDir::Ingress,
        Some(43128),
        Some(5432),
        Some(0x45),
        None,
        None,
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        6,
        509,
        0,
        PacketDir::Egress,
        Some(43128),
        Some(5432),
        Some(0x70),
        None,
        None,
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        7,
        509,
        0,
        PacketDir::Ingress,
        Some(43128),
        Some(5432),
        Some(0x5a),
        None,
        None,
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(80));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .all(|stage| stage.phase.as_deref() != Some("receive_auth"))
    );
}

#[test]
fn postgres_query_error_path_materializes_query_and_error_phases() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/postgres_query_error_path.gewy")
            .unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 510, 7785, "psql"));
    session.ingest(tcp_state_fact_with_ports(2, 510, 1, 2, 43129, 5432));
    session.ingest(tcp_state_fact_with_ports(3, 510, 2, 3, 43129, 5432));
    session.ingest(route_fact(4, 510, 6));
    session.ingest(packet_fact_with_dir_and_payload(
        5,
        510,
        0,
        PacketDir::Egress,
        Some(43129),
        Some(5432),
        Some(0x51),
        None,
        None,
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        6,
        510,
        0,
        PacketDir::Ingress,
        Some(43129),
        Some(5432),
        Some(0x45),
        None,
        None,
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(70));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("postgres_query_error".into())
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
            .any(|stage| stage.phase.as_deref() == Some("receive_error"))
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
fn postgres_query_error_path_does_not_match_ready_message() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/postgres_query_error_path.gewy")
            .unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 511, 7786, "psql"));
    session.ingest(tcp_state_fact_with_ports(2, 511, 1, 2, 43130, 5432));
    session.ingest(tcp_state_fact_with_ports(3, 511, 2, 3, 43130, 5432));
    session.ingest(route_fact(4, 511, 6));
    session.ingest(packet_fact_with_dir_and_payload(
        5,
        511,
        0,
        PacketDir::Egress,
        Some(43130),
        Some(5432),
        Some(0x51),
        None,
        None,
    ));
    session.ingest(packet_fact_with_dir_and_payload(
        6,
        511,
        0,
        PacketDir::Ingress,
        Some(43130),
        Some(5432),
        Some(0x5a),
        None,
        None,
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(70));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .all(|stage| stage.phase.as_deref() != Some("receive_error"))
    );
}

#[test]
fn mysql_simple_query_path_materializes_connect_query_and_ok_phases() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/mysql_simple_query_path.gewy").unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 512, 7787, "mysql"));
    session.ingest(tcp_state_fact_with_ports(2, 512, 1, 2, 43131, 3306));
    session.ingest(tcp_state_fact_with_ports(3, 512, 2, 3, 43131, 3306));
    session.ingest(route_fact(4, 512, 6));
    session.ingest(packet_fact_with_dir_and_payload_and_byte4(
        5,
        512,
        0,
        PacketDir::Egress,
        Some(43131),
        Some(3306),
        None,
        None,
        None,
        Some(0x03),
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_byte4(
        6,
        512,
        0,
        PacketDir::Ingress,
        Some(43131),
        Some(3306),
        None,
        None,
        None,
        Some(0x00),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(70));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("mysql_simple_query".into())
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
            .any(|stage| stage.phase.as_deref() == Some("receive_ok"))
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
fn mysql_simple_query_path_does_not_match_error_packet_as_ok() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/mysql_simple_query_path.gewy").unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 513, 7788, "mysql"));
    session.ingest(tcp_state_fact_with_ports(2, 513, 1, 2, 43132, 3306));
    session.ingest(tcp_state_fact_with_ports(3, 513, 2, 3, 43132, 3306));
    session.ingest(route_fact(4, 513, 6));
    session.ingest(packet_fact_with_dir_and_payload_and_byte4(
        5,
        513,
        0,
        PacketDir::Egress,
        Some(43132),
        Some(3306),
        None,
        None,
        None,
        Some(0x03),
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_byte4(
        6,
        513,
        0,
        PacketDir::Ingress,
        Some(43132),
        Some(3306),
        None,
        None,
        None,
        Some(0xff),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(70));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .all(|stage| stage.phase.as_deref() != Some("receive_ok"))
    );
}

#[test]
fn mysql_query_session_can_span_connect_query_and_ok_in_one_module() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/mysql_query_session.gewy").unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 514, 7789, "mysql-session"));
    session.ingest(route_fact(2, 514, 6));
    session.ingest(tcp_state_fact_with_ports(3, 514, 1, 2, 43133, 3306));
    session.ingest(tcp_state_fact_with_ports(4, 514, 2, 3, 43133, 3306));
    session.ingest(packet_fact_with_dir_and_payload_and_byte4(
        5,
        514,
        0,
        PacketDir::Egress,
        Some(43133),
        Some(3306),
        None,
        None,
        None,
        Some(0x03),
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_byte4(
        6,
        514,
        0,
        PacketDir::Ingress,
        Some(43133),
        Some(3306),
        None,
        None,
        None,
        Some(0x00),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(80));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("mysql_query_session".into())
    );
    let phases = export.program_flows[0]
        .stages
        .iter()
        .filter_map(|stage| stage.phase.clone())
        .collect::<Vec<_>>();
    assert!(phases.contains(&"connect".to_string()));
    assert!(phases.contains(&"establish".to_string()));
    assert!(phases.contains(&"send_query".to_string()));
    assert!(phases.contains(&"receive_ok".to_string()));
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
fn mysql_query_session_missing_response_produces_query_to_ok_transition() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/mysql_query_session.gewy").unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 515, 7790, "mysql-session"));
    session.ingest(route_fact(2, 515, 6));
    session.ingest(tcp_state_fact_with_ports(3, 515, 1, 2, 43134, 3306));
    session.ingest(tcp_state_fact_with_ports(4, 515, 2, 3, 43134, 3306));
    session.ingest(packet_fact_with_dir_and_payload_and_byte4(
        5,
        515,
        0,
        PacketDir::Egress,
        Some(43134),
        Some(3306),
        None,
        None,
        None,
        Some(0x03),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(80));

    let export = session.export_bundle();
    assert!(export.program_findings.iter().any(|finding| {
        finding.module_label == "mysql_query_session"
            && finding.network_module_kind == "database_query"
            && finding.phase.as_deref() == Some("receive_ok")
            && finding.phase_transition.as_deref() == Some("send_query->receive_ok")
    }));
    assert!(export.module_findings.iter().any(|finding| {
        finding
            .phase_transitions
            .contains(&"send_query->receive_ok".to_string())
    }));
}

#[test]
fn mysql_query_error_path_materializes_query_and_error_phases() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/mysql_query_error_path.gewy").unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 514, 7789, "mysql"));
    session.ingest(tcp_state_fact_with_ports(2, 514, 1, 2, 43133, 3306));
    session.ingest(tcp_state_fact_with_ports(3, 514, 2, 3, 43133, 3306));
    session.ingest(route_fact(4, 514, 6));
    session.ingest(packet_fact_with_dir_and_payload_and_byte4(
        5,
        514,
        0,
        PacketDir::Egress,
        Some(43133),
        Some(3306),
        None,
        None,
        None,
        Some(0x03),
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_byte4(
        6,
        514,
        0,
        PacketDir::Ingress,
        Some(43133),
        Some(3306),
        None,
        None,
        None,
        Some(0xff),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(70));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("mysql_query_error".into())
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
            .any(|stage| stage.phase.as_deref() == Some("receive_error"))
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
fn mysql_query_error_path_does_not_match_ok_packet() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/mysql_query_error_path.gewy").unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 515, 7790, "mysql"));
    session.ingest(tcp_state_fact_with_ports(2, 515, 1, 2, 43134, 3306));
    session.ingest(tcp_state_fact_with_ports(3, 515, 2, 3, 43134, 3306));
    session.ingest(route_fact(4, 515, 6));
    session.ingest(packet_fact_with_dir_and_payload_and_byte4(
        5,
        515,
        0,
        PacketDir::Egress,
        Some(43134),
        Some(3306),
        None,
        None,
        None,
        Some(0x03),
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_byte4(
        6,
        515,
        0,
        PacketDir::Ingress,
        Some(43134),
        Some(3306),
        None,
        None,
        None,
        Some(0x00),
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(70));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .all(|stage| stage.phase.as_deref() != Some("receive_error"))
    );
}

#[test]
fn memcached_get_path_materializes_connect_get_and_value_phases() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/memcached_get_path.gewy").unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 516, 7791, "memcached-client"));
    session.ingest(tcp_state_fact_with_ports(2, 516, 1, 2, 43135, 11211));
    session.ingest(tcp_state_fact_with_ports(3, 516, 2, 3, 43135, 11211));
    session.ingest(route_fact(4, 516, 6));
    session.ingest(packet_fact_with_dir_and_payload_and_byte1(
        5,
        516,
        0,
        PacketDir::Egress,
        Some(43135),
        Some(11211),
        Some(0x80),
        Some(0x00),
        None,
        None,
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_byte1(
        6,
        516,
        0,
        PacketDir::Ingress,
        Some(43135),
        Some(11211),
        Some(0x81),
        Some(0x00),
        None,
        None,
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(70));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("memcached_get".into())
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("send_get"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_value"))
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
fn memcached_get_path_does_not_match_set_opcode() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/memcached_get_path.gewy").unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 517, 7792, "memcached-client"));
    session.ingest(tcp_state_fact_with_ports(2, 517, 1, 2, 43136, 11211));
    session.ingest(tcp_state_fact_with_ports(3, 517, 2, 3, 43136, 11211));
    session.ingest(route_fact(4, 517, 6));
    session.ingest(packet_fact_with_dir_and_payload_and_byte1(
        5,
        517,
        0,
        PacketDir::Egress,
        Some(43136),
        Some(11211),
        Some(0x80),
        Some(0x01),
        None,
        None,
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_byte1(
        6,
        517,
        0,
        PacketDir::Ingress,
        Some(43136),
        Some(11211),
        Some(0x81),
        Some(0x01),
        None,
        None,
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(70));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .all(|stage| stage.phase.as_deref() != Some("receive_value"))
    );
}

#[test]
fn memcached_set_path_materializes_connect_set_and_stored_phases() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/memcached_set_path.gewy").unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 518, 7793, "memcached-client"));
    session.ingest(tcp_state_fact_with_ports(2, 518, 1, 2, 43137, 11211));
    session.ingest(tcp_state_fact_with_ports(3, 518, 2, 3, 43137, 11211));
    session.ingest(route_fact(4, 518, 6));
    session.ingest(packet_fact_with_dir_and_payload_and_byte1(
        5,
        518,
        0,
        PacketDir::Egress,
        Some(43137),
        Some(11211),
        Some(0x80),
        Some(0x01),
        None,
        None,
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_byte1(
        6,
        518,
        0,
        PacketDir::Ingress,
        Some(43137),
        Some(11211),
        Some(0x81),
        Some(0x01),
        None,
        None,
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(70));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("memcached_set".into())
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("send_set"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_stored"))
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
fn memcached_set_path_does_not_match_get_opcode() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/memcached_set_path.gewy").unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 519, 7794, "memcached-client"));
    session.ingest(tcp_state_fact_with_ports(2, 519, 1, 2, 43138, 11211));
    session.ingest(tcp_state_fact_with_ports(3, 519, 2, 3, 43138, 11211));
    session.ingest(route_fact(4, 519, 6));
    session.ingest(packet_fact_with_dir_and_payload_and_byte1(
        5,
        519,
        0,
        PacketDir::Egress,
        Some(43138),
        Some(11211),
        Some(0x80),
        Some(0x00),
        None,
        None,
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_byte1(
        6,
        519,
        0,
        PacketDir::Ingress,
        Some(43138),
        Some(11211),
        Some(0x81),
        Some(0x00),
        None,
        None,
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(70));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .all(|stage| stage.phase.as_deref() != Some("receive_stored"))
    );
}

#[test]
fn amqp_connection_start_path_materializes_header_start_and_start_ok_phases() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/amqp_connection_start_path.gewy")
            .unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 520, 7795, "amqp-client"));
    session.ingest(tcp_state_fact_with_ports(2, 520, 1, 2, 43139, 5672));
    session.ingest(tcp_state_fact_with_ports(3, 520, 2, 3, 43139, 5672));
    session.ingest(route_fact(4, 520, 6));
    session.ingest(packet_fact_with_dir_and_payload(
        5,
        520,
        0,
        PacketDir::Egress,
        Some(43139),
        Some(5672),
        None,
        None,
        Some(0x414d5150),
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_byte10(
        6,
        520,
        0,
        PacketDir::Ingress,
        Some(43139),
        Some(5672),
        Some(0x01),
        Some(0x0a),
        None,
        None,
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_byte10(
        7,
        520,
        0,
        PacketDir::Egress,
        Some(43139),
        Some(5672),
        Some(0x01),
        Some(0x0b),
        None,
        None,
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(80));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("amqp_connection_start".into())
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("send_protocol_header"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_start"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("send_start_ok"))
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
fn amqp_connection_start_path_does_not_match_wrong_server_method_id() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/amqp_connection_start_path.gewy")
            .unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 521, 7796, "amqp-client"));
    session.ingest(tcp_state_fact_with_ports(2, 521, 1, 2, 43140, 5672));
    session.ingest(tcp_state_fact_with_ports(3, 521, 2, 3, 43140, 5672));
    session.ingest(route_fact(4, 521, 6));
    session.ingest(packet_fact_with_dir_and_payload(
        5,
        521,
        0,
        PacketDir::Egress,
        Some(43140),
        Some(5672),
        None,
        None,
        Some(0x414d5150),
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_byte10(
        6,
        521,
        0,
        PacketDir::Ingress,
        Some(43140),
        Some(5672),
        Some(0x01),
        Some(0x14),
        None,
        None,
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_byte10(
        7,
        521,
        0,
        PacketDir::Egress,
        Some(43140),
        Some(5672),
        Some(0x01),
        Some(0x0b),
        None,
        None,
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(80));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .all(|stage| stage.phase.as_deref() != Some("receive_start"))
    );
}

#[test]
fn amqp_basic_publish_path_materializes_publish_and_ack_phases() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/amqp_basic_publish_path.gewy").unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 522, 7797, "amqp-publisher"));
    session.ingest(tcp_state_fact_with_ports(2, 522, 1, 2, 43141, 5672));
    session.ingest(tcp_state_fact_with_ports(3, 522, 2, 3, 43141, 5672));
    session.ingest(route_fact(4, 522, 6));
    session.ingest(packet_fact_with_dir_and_payload_and_byte10(
        5,
        522,
        0,
        PacketDir::Egress,
        Some(43141),
        Some(5672),
        Some(0x01),
        Some(0x28),
        None,
        None,
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_byte10(
        6,
        522,
        0,
        PacketDir::Ingress,
        Some(43141),
        Some(5672),
        Some(0x01),
        Some(0x50),
        None,
        None,
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(70));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("amqp_basic_publish".into())
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("send_publish"))
    );
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_ack"))
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
fn amqp_basic_publish_path_does_not_match_wrong_ack_method_id() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/amqp_basic_publish_path.gewy").unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 523, 7798, "amqp-publisher"));
    session.ingest(tcp_state_fact_with_ports(2, 523, 1, 2, 43142, 5672));
    session.ingest(tcp_state_fact_with_ports(3, 523, 2, 3, 43142, 5672));
    session.ingest(route_fact(4, 523, 6));
    session.ingest(packet_fact_with_dir_and_payload_and_byte10(
        5,
        523,
        0,
        PacketDir::Egress,
        Some(43142),
        Some(5672),
        Some(0x01),
        Some(0x28),
        None,
        None,
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_byte10(
        6,
        523,
        0,
        PacketDir::Ingress,
        Some(43142),
        Some(5672),
        Some(0x01),
        Some(0x0a),
        None,
        None,
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(70));

    let export = session.export_bundle();
    assert!(
        export.program_flows[0]
            .stages
            .iter()
            .all(|stage| stage.phase.as_deref() != Some("receive_ack"))
    );
}

#[test]
fn amqp_publish_session_can_span_startup_and_publish_in_one_module() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/amqp_publish_session.gewy").unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 524, 7799, "amqp-publisher"));
    session.ingest(route_fact(2, 524, 6));
    session.ingest(tcp_state_fact_with_ports(3, 524, 1, 2, 43143, 5672));
    session.ingest(tcp_state_fact_with_ports(4, 524, 2, 3, 43143, 5672));
    session.ingest(packet_fact_with_dir_and_payload(
        5,
        524,
        0,
        PacketDir::Egress,
        Some(43143),
        Some(5672),
        None,
        None,
        Some(0x414d5150),
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_byte10(
        6,
        524,
        0,
        PacketDir::Ingress,
        Some(43143),
        Some(5672),
        Some(0x01),
        Some(0x0a),
        None,
        None,
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_byte10(
        7,
        524,
        0,
        PacketDir::Egress,
        Some(43143),
        Some(5672),
        Some(0x01),
        Some(0x0b),
        None,
        None,
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_byte10(
        8,
        524,
        0,
        PacketDir::Egress,
        Some(43143),
        Some(5672),
        Some(0x01),
        Some(0x28),
        None,
        None,
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_byte10(
        9,
        524,
        0,
        PacketDir::Ingress,
        Some(43143),
        Some(5672),
        Some(0x01),
        Some(0x50),
        None,
        None,
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(100));

    let export = session.export_bundle();
    assert_eq!(
        export.program_flows[0].operation,
        ProgramOperation::Custom("amqp_publish_session".into())
    );
    let phases = export.program_flows[0]
        .stages
        .iter()
        .filter_map(|stage| stage.phase.clone())
        .collect::<Vec<_>>();
    assert!(phases.contains(&"connect".to_string()));
    assert!(phases.contains(&"establish".to_string()));
    assert!(phases.contains(&"send_protocol_header".to_string()));
    assert!(phases.contains(&"receive_start".to_string()));
    assert!(phases.contains(&"send_start_ok".to_string()));
    assert!(phases.contains(&"send_publish".to_string()));
    assert!(phases.contains(&"receive_ack".to_string()));
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
fn amqp_publish_session_missing_publish_produces_start_ok_to_publish_transition() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/amqp_publish_session.gewy").unwrap();
    let config = SessionConfig::for_binding(binding).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(sock_lineage_fact(1, 525, 7800, "amqp-publisher"));
    session.ingest(route_fact(2, 525, 6));
    session.ingest(tcp_state_fact_with_ports(3, 525, 1, 2, 43144, 5672));
    session.ingest(tcp_state_fact_with_ports(4, 525, 2, 3, 43144, 5672));
    session.ingest(packet_fact_with_dir_and_payload(
        5,
        525,
        0,
        PacketDir::Egress,
        Some(43144),
        Some(5672),
        None,
        None,
        Some(0x414d5150),
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_byte10(
        6,
        525,
        0,
        PacketDir::Ingress,
        Some(43144),
        Some(5672),
        Some(0x01),
        Some(0x0a),
        None,
        None,
    ));
    session.ingest(packet_fact_with_dir_and_payload_and_byte10(
        7,
        525,
        0,
        PacketDir::Egress,
        Some(43144),
        Some(5672),
        Some(0x01),
        Some(0x0b),
        None,
        None,
    ));
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(90));

    let export = session.export_bundle();
    assert!(export.program_findings.iter().any(|finding| {
        finding.module_label == "amqp_publish_session"
            && finding.phase.as_deref() == Some("send_publish")
            && finding.phase_transition.as_deref() == Some("send_start_ok->send_publish")
    }));
    assert!(export.module_findings.iter().any(|finding| {
        finding
            .phase_transitions
            .contains(&"send_start_ok->send_publish".to_string())
    }));
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
fn dsl_accepts_pipeline_template_calls() {
    let binding = compile_str(
        r#"
template(:pipeline_udp_debug)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> fragment(:udp_packet_meta_fragment)
|> fragment(:route_meta_fragment)
|> fragment(:sock_lineage_fragment)
|> operation(:datagram_exchange)
|> program_model(:pipeline_udp_debug_model)
|> program_rule(predicate: :process_bound, stage: :process_bound, narrative: :process_bound, dedupe: true, module: :pipeline_udp_debug, phase: :bind)
|> program_rule(predicate: "datagram_observed:udp:local_to_remote", stage: :datagram_observed, narrative: :udp_datagram_sent, dedupe: true, module: :pipeline_udp_debug, phase: :send_request)
|> param(:sock_lineage_fragment.capture_comm, true)
"#,
    )
    .unwrap();

    assert_eq!(binding.template.id, "pipeline_udp_debug");
    assert_eq!(
        binding.template.fragment_set,
        vec![
            "udp_packet_meta_fragment",
            "route_meta_fragment",
            "sock_lineage_fragment"
        ]
    );
    let model = binding.template.program_model.as_ref().unwrap();
    assert_eq!(model.id, "pipeline_udp_debug_model");
    assert_eq!(model.operation, ProgramOperation::DatagramExchange);
    assert_eq!(model.rules.len(), 2);
    assert_eq!(
        binding.fragment_params["sock_lineage_fragment"]["capture_comm"],
        FragmentParamValue::Bool(true)
    );
}

#[test]
fn dsl_accepts_pipeline_reason_rule_calls() {
    let binding = compile_str(
        r#"
template(:pipeline_reason_udp)
|> window(duration_ms: 5000, lateness_ms: 200)
|> fragment(:udp_packet_meta_fragment)
|> fragment(:route_meta_fragment)
|> fragment(:sock_lineage_fragment)
|> operation(:datagram_exchange)
|> program_model(:pipeline_reason_udp_model)
|> program_rule(predicate: :process_bound, stage: :process_bound, narrative: :process_bound, dedupe: true, module: :pipeline_reason_udp, phase: :bind)
|> reason_model(:pipeline_reason_udp_reason)
|> reason_rule(predicate: :process_bound, key_event: :process_identified, narrative: :process_bound, dedupe: true, module: :pipeline_reason_udp, phase: :bind)
"#,
    )
    .unwrap();

    assert!(matches!(
        binding.template.reason_profile.as_ref().unwrap(),
        ReasonProfile::Declarative(reason) if reason.id == "pipeline_reason_udp_reason"
    ));
}

#[test]
fn dsl_accepts_pipeline_function_units_without_global_state() {
    let binding = compile_str(
        r#"
fn udp_core() {
  |> fragment(:udp_packet_meta_fragment)
  |> fragment(:route_meta_fragment)
  |> fragment(:sock_lineage_fragment)
  |> operation(:datagram_exchange)
  |> program_model(:pipeline_function_udp_model)
  |> program_rule(predicate: :process_bound, stage: :process_bound, narrative: :process_bound, dedupe: true, module: :pipeline_function_udp, phase: :bind)
}

template(:pipeline_function_udp)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> use(:udp_core)
|> param(:sock_lineage_fragment.capture_comm, true)
"#,
    )
    .unwrap();

    assert_eq!(binding.template.id, "pipeline_function_udp");
    assert_eq!(
        binding.template.fragment_set,
        vec![
            "udp_packet_meta_fragment",
            "route_meta_fragment",
            "sock_lineage_fragment"
        ]
    );
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().id,
        "pipeline_function_udp_model"
    );
    assert_eq!(
        binding.fragment_params["sock_lineage_fragment"]["capture_comm"],
        FragmentParamValue::Bool(true)
    );
}

#[test]
fn dsl_accepts_parameterized_pipeline_function_units() {
    let binding = compile_str(
        r#"
fn udp_core(model_name, op_name) {
  |> fragment(:udp_packet_meta_fragment)
  |> fragment(:route_meta_fragment)
  |> fragment(:sock_lineage_fragment)
  |> operation(${op_name})
  |> program_model(${model_name})
  |> program_rule(predicate: :process_bound, stage: :process_bound, narrative: :process_bound, dedupe: true, module: ${model_name}, phase: :bind)
}

template(:pipeline_parameter_fn_udp)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> use(:udp_core, :pipeline_parameter_fn_udp_model, :datagram_exchange)
"#,
    )
    .unwrap();

    assert_eq!(binding.template.id, "pipeline_parameter_fn_udp");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().id,
        "pipeline_parameter_fn_udp_model"
    );
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::DatagramExchange
    );
}

#[test]
fn dsl_accepts_nested_pipeline_function_use_units() {
    let binding = compile_str(
        r#"
fn udp_rules() {
  |> operation(:datagram_exchange)
  |> program_model(:pipeline_nested_fn_udp_model)
  |> program_rule(predicate: :process_bound, stage: :process_bound, narrative: :process_bound, dedupe: true, module: :pipeline_nested_fn_udp, phase: :bind)
}

fn udp_core() {
  |> fragment(:udp_packet_meta_fragment)
  |> fragment(:route_meta_fragment)
  |> fragment(:sock_lineage_fragment)
  |> use(:udp_rules)
}

template(:pipeline_nested_fn_udp)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> use(:udp_core)
"#,
    )
    .unwrap();

    assert_eq!(binding.template.id, "pipeline_nested_fn_udp");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().id,
        "pipeline_nested_fn_udp_model"
    );
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::DatagramExchange
    );
}

#[test]
fn dsl_package_entry_compiles_from_manifest_directory_and_merges_pipeline_includes() {
    let package_dir =
        std::env::temp_dir().join(format!("gewy-package-{}-manifest-dir", std::process::id()));
    fs::create_dir_all(&package_dir).unwrap();
    fs::write(
        package_dir.join("gewy.pkg"),
        "name=package_udp_debug\nversion=0.1.0\nentry=main.gewy\n",
    )
    .unwrap();
    fs::write(
        package_dir.join("main.gewy"),
        r#"
template(:package_udp_debug)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> include("./partials.gewy")
"#,
    )
    .unwrap();
    fs::write(
        package_dir.join("partials.gewy"),
        r#"
|> fragment(:udp_packet_meta_fragment)
|> fragment(:route_meta_fragment)
|> fragment(:sock_lineage_fragment)
|> operation(:datagram_exchange)
|> program_model(:package_udp_debug_model)
|> program_rule(predicate: :process_bound, stage: :process_bound, narrative: :process_bound, dedupe: true, module: :package_udp_debug, phase: :bind)
|> param(:sock_lineage_fragment.capture_comm, true)
"#,
    )
    .unwrap();

    let binding = compile_file(package_dir.to_str().unwrap()).unwrap();
    assert_eq!(binding.template.id, "package_udp_debug");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().id,
        "package_udp_debug_model"
    );
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::DatagramExchange
    );
    assert_eq!(
        binding.fragment_params["sock_lineage_fragment"]["capture_comm"],
        FragmentParamValue::Bool(true)
    );
}

#[test]
fn dsl_package_entry_can_include_pipeline_module_from_local_dependency() {
    let root = std::env::temp_dir().join(format!("gewy-package-{}-deps", std::process::id()));
    let app_dir = root.join("app");
    let dep_dir = root.join("udp_stdlib");
    fs::create_dir_all(&app_dir).unwrap();
    fs::create_dir_all(&dep_dir).unwrap();

    fs::write(
        app_dir.join("gewy.pkg"),
        format!(
            "name=app_with_dep\nversion=0.1.0\nentry=main.gewy\ndep.std={}\n",
            dep_dir.to_string_lossy()
        ),
    )
    .unwrap();
    fs::write(
        app_dir.join("main.gewy"),
        r#"
template(:app_with_dep)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> include("std:udp_module.gewy")
"#,
    )
    .unwrap();
    fs::write(
        dep_dir.join("udp_module.gewy"),
        r#"
|> fragment(:udp_packet_meta_fragment)
|> fragment(:route_meta_fragment)
|> fragment(:sock_lineage_fragment)
|> operation(:datagram_exchange)
|> program_model(:app_with_dep_model)
|> program_rule(predicate: :process_bound, stage: :process_bound, narrative: :process_bound, dedupe: true, module: :app_with_dep, phase: :bind)
"#,
    )
    .unwrap();

    let binding = compile_file(app_dir.to_str().unwrap()).unwrap();
    assert_eq!(binding.template.id, "app_with_dep");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().id,
        "app_with_dep_model"
    );
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::DatagramExchange
    );
}

#[test]
fn dsl_package_entry_can_include_pipeline_module_from_named_source_dependency() {
    let root =
        std::env::temp_dir().join(format!("gewy-package-{}-source-deps", std::process::id()));
    let app_dir = root.join("app");
    let registry_dir = root.join("registry");
    let dep_dir = registry_dir.join("udp_stdlib");
    fs::create_dir_all(&app_dir).unwrap();
    fs::create_dir_all(&dep_dir).unwrap();

    fs::write(
        app_dir.join("gewy.pkg"),
        format!(
            "name=app_with_source_dep\nversion=0.1.0\nentry=main.gewy\nsource.local={}\ndep.std=source:local/udp_stdlib\n",
            registry_dir.to_string_lossy()
        ),
    )
    .unwrap();
    fs::write(
        app_dir.join("main.gewy"),
        r#"
template(:app_with_source_dep)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> include("std:udp_module.gewy")
"#,
    )
    .unwrap();
    fs::write(
        dep_dir.join("udp_module.gewy"),
        r#"
|> fragment(:udp_packet_meta_fragment)
|> fragment(:route_meta_fragment)
|> fragment(:sock_lineage_fragment)
|> operation(:datagram_exchange)
|> program_model(:app_with_source_dep_model)
|> program_rule(predicate: :process_bound, stage: :process_bound, narrative: :process_bound, dedupe: true, module: :app_with_source_dep, phase: :bind)
"#,
    )
    .unwrap();

    let binding = compile_file(app_dir.to_str().unwrap()).unwrap();
    assert_eq!(binding.template.id, "app_with_source_dep");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().id,
        "app_with_source_dep_model"
    );
}

#[test]
fn dsl_package_entry_can_use_function_defined_in_included_module() {
    let package_dir =
        std::env::temp_dir().join(format!("gewy-package-{}-functions", std::process::id()));
    fs::create_dir_all(&package_dir).unwrap();
    fs::write(
        package_dir.join("gewy.pkg"),
        "name=package_fn_udp\nversion=0.1.0\nentry=main.gewy\n",
    )
    .unwrap();
    fs::write(
        package_dir.join("main.gewy"),
        r#"
template(:package_fn_udp)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> include("./module.gewy")
|> use(:udp_core)
"#,
    )
    .unwrap();
    fs::write(
        package_dir.join("module.gewy"),
        r#"
fn udp_core() {
  |> fragment(:udp_packet_meta_fragment)
  |> fragment(:route_meta_fragment)
  |> fragment(:sock_lineage_fragment)
  |> operation(:datagram_exchange)
  |> program_model(:package_fn_udp_model)
  |> program_rule(predicate: :process_bound, stage: :process_bound, narrative: :process_bound, dedupe: true, module: :package_fn_udp, phase: :bind)
}
"#,
    )
    .unwrap();

    let binding = compile_file(package_dir.to_str().unwrap()).unwrap();
    assert_eq!(binding.template.id, "package_fn_udp");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().id,
        "package_fn_udp_model"
    );
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::DatagramExchange
    );
}

#[test]
fn dsl_package_entry_rejects_include_that_escapes_package_root() {
    let root = std::env::temp_dir().join(format!("gewy-package-{}-escape", std::process::id()));
    let package_dir = root.join("app");
    fs::create_dir_all(&package_dir).unwrap();
    fs::write(
        package_dir.join("gewy.pkg"),
        "name=escape_guard\nversion=0.1.0\nentry=main.gewy\n",
    )
    .unwrap();
    fs::write(
        package_dir.join("main.gewy"),
        r#"
template(:escape_guard)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> include("../outside.gewy")
"#,
    )
    .unwrap();
    fs::write(
        root.join("outside.gewy"),
        "|> fragment(:udp_packet_meta_fragment)\n",
    )
    .unwrap();

    let err = compile_file(package_dir.to_str().unwrap()).unwrap_err();
    assert!(
        format!("{err:?}").contains("escapes package root"),
        "unexpected error: {err:?}"
    );
}

#[test]
fn dsl_package_dependency_include_rejects_escape_from_dependency_root() {
    let root = std::env::temp_dir().join(format!("gewy-package-{}-dep-escape", std::process::id()));
    let app_dir = root.join("app");
    let dep_dir = root.join("dep");
    fs::create_dir_all(&app_dir).unwrap();
    fs::create_dir_all(&dep_dir).unwrap();
    fs::write(
        app_dir.join("gewy.pkg"),
        format!(
            "name=dep_escape_guard\nversion=0.1.0\nentry=main.gewy\ndep.std={}\n",
            dep_dir.to_string_lossy()
        ),
    )
    .unwrap();
    fs::write(
        app_dir.join("main.gewy"),
        r#"
template(:dep_escape_guard)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> include("std:../outside.gewy")
"#,
    )
    .unwrap();
    fs::write(
        root.join("outside.gewy"),
        "|> fragment(:udp_packet_meta_fragment)\n",
    )
    .unwrap();

    let err = compile_file(app_dir.to_str().unwrap()).unwrap_err();
    assert!(
        format!("{err:?}").contains("escapes package root"),
        "unexpected error: {err:?}"
    );
}

#[test]
fn dsl_rejects_pipeline_include_cycles() {
    let package_dir =
        std::env::temp_dir().join(format!("gewy-package-{}-include-cycle", std::process::id()));
    fs::create_dir_all(&package_dir).unwrap();
    fs::write(
        package_dir.join("gewy.pkg"),
        "name=include_cycle\nversion=0.1.0\nentry=main.gewy\n",
    )
    .unwrap();
    fs::write(
        package_dir.join("main.gewy"),
        r#"
template(:include_cycle)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> include("./module.gewy")
"#,
    )
    .unwrap();
    fs::write(
        package_dir.join("module.gewy"),
        r#"
|> include("./main.gewy")
"#,
    )
    .unwrap();

    let err = compile_file(package_dir.to_str().unwrap()).unwrap_err();
    assert!(
        format!("{err:?}").contains("include cycle detected"),
        "unexpected error: {err:?}"
    );
}

#[test]
fn dsl_rejects_pipeline_use_cycles() {
    let err = compile_str(
        r#"
fn alpha() {
  |> use(:beta)
}

fn beta() {
  |> use(:alpha)
}

template(:use_cycle)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> use(:alpha)
"#,
    )
    .unwrap_err();
    assert!(
        format!("{err:?}").contains("use cycle detected"),
        "unexpected error: {err:?}"
    );
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
fn binding_diagnostics_reports_expanded_sequence_offsets() {
    let binding = parse_str_unvalidated(
        r#"
template=unsupported_payload_sequence
window=default_5s
reason=udp_datagram_l1
fragment=udp_packet_meta_fragment
program_model=unsupported_payload_sequence_model
operation=snmp_get
rule=datagram_observed:udp:remote:snmp:bytes_at:8:0x30,0x82,0x01,0x00;datagram_observed;udp_datagram_sent;true
"#,
    )
    .unwrap();

    let diagnostics = collect_binding_diagnostics(&binding).unwrap();
    let rule = &diagnostics.program_model.as_ref().unwrap().rules[0];
    assert!(!rule.supported);
    assert_eq!(rule.unsupported_payload_offsets, vec![8, 11]);
}

#[test]
fn binding_diagnostics_accept_dynamic_sample_payload_offsets_from_fragment_params() {
    let binding = parse_str_unvalidated(
        r#"
template=dynamic_payload_offset_support
window=default_5s
reason=udp_datagram_l1
fragment=udp_packet_meta_fragment
param=udp_packet_meta_fragment.sample_payload_offsets=8
program_model=dynamic_payload_offset_support_model
operation=snmp_get
rule=datagram_observed:udp:remote:snmp:byte_at:8:0xff:0xa0;datagram_observed;udp_datagram_sent;true
"#,
    )
    .unwrap();

    let registry = builtin_registry();
    let diagnostics = registry.binding_diagnostics(&binding).unwrap();
    let rule = &diagnostics.program_model.as_ref().unwrap().rules[0];
    assert!(rule.supported);
    assert_eq!(rule.unsupported_payload_offsets, Vec::<u16>::new());

    let summary = registry.payload_offset_support_summary(&binding, &diagnostics);
    assert!(summary.sampled_offsets.contains(&8));
    assert_eq!(summary.unsupported_offsets, Vec::<u16>::new());
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
            byte_sequences: vec![],
        }
    );
}
