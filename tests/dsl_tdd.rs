use gewyvern::dsl::{compile_file, compile_str, DslError};
use gewyvern::fragment::RegistryError;
use gewyvern::flow::ProgramOperation;
use gewyvern::runtime::{RuntimeSession, SessionConfig};
use gewyvern::template::FragmentParamValue;

mod support;

use support::{route_fact, sock_lineage_fact, tcp_state_fact, udp_packet_fact};
use std::time::{Duration, SystemTime};

#[test]
fn built_in_udp_process_dsl_compiles_into_template_binding() {
    let binding = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gwy")
        .unwrap();

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
        binding.fragment_params["sock_lineage_fragment"]["capture_comm"],
        FragmentParamValue::Bool(true)
    );
}

#[test]
fn udp_process_dsl_binding_drives_runtime_session() {
    let binding = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gwy")
        .unwrap();
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
    assert_eq!(export.rejected_fact_summary[0].reason, "filtered_by_fragment_param");
}

#[test]
fn handshake_dsl_compiles_and_preserves_tcp_shape() {
    let binding = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/handshake_debug.gwy")
        .unwrap();
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
