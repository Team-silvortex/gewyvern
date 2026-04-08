use gewyvern::dsl::{compile_file, compile_str, DslError};
use gewyvern::fragment::{RegistryError, RuleTier};
use gewyvern::flow::ProgramOperation;
use gewyvern::reason::{KeyEventKind, ReasonProfile};
use gewyvern::runtime::{RuntimeSession, SessionConfig};
use gewyvern::template::FragmentParamValue;

mod support;

use support::{route_fact, sock_lineage_fact, tcp_state_fact, udp_packet_fact};
use std::time::{Duration, SystemTime};

#[test]
fn built_in_udp_process_dsl_compiles_into_template_binding() {
    let binding = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy")
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
        binding.template.window_profile.as_ref().unwrap().duration_ms,
        5_000
    );
    assert_eq!(
        binding.template.window_profile.as_ref().unwrap().lateness_ms,
        200
    );
    assert_eq!(
        binding.fragment_params["sock_lineage_fragment"]["capture_comm"],
        FragmentParamValue::Bool(true)
    );
}

#[test]
fn udp_process_dsl_binding_drives_runtime_session() {
    let binding = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy")
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
    let binding = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/handshake_debug.gewy")
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

    assert_eq!(binding.template.window_profile.as_ref().unwrap().id, "inline");
    assert_eq!(
        binding.template.window_profile.as_ref().unwrap().duration_ms,
        9_000
    );
    assert_eq!(
        binding.template.window_profile.as_ref().unwrap().lateness_ms,
        450
    );
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().id,
        "udp_inline_debug_dsl_model"
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
    assert_eq!(export.reasons[0].l1.key_events[0].kind, KeyEventKind::ProcessIdentified);
    assert_eq!(export.reasons[0].l1.key_events[1].kind, KeyEventKind::UdpDatagramSeen);
    assert_eq!(export.reasons[0].l1.key_events[2].kind, KeyEventKind::RouteChanged);

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
    assert!(export.program_flows[0]
        .narrative
        .iter()
        .any(|line| line == "process dig (pid=5353) bound this network flow"));
    assert!(export.program_flows[0]
        .narrative
        .iter()
        .any(|line| line == "program emitted or received a UDP datagram"));
    assert!(export.program_flows[0]
        .narrative
        .iter()
        .any(|line| line == "program resolved a route for this network flow"));
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
    assert_eq!(export.reasons[0].l1.key_events[0].kind, KeyEventKind::ProcessIdentified);
    assert_eq!(export.reasons[0].l1.key_events[1].kind, KeyEventKind::UdpDatagramSeen);
    assert_eq!(export.reasons[0].l1.key_events[2].kind, KeyEventKind::RouteChanged);
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
    let binding = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy")
        .unwrap();
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
    assert_eq!(diagnostics.rules[0].required_facts, vec![gewyvern::ledger::FactKindTag::SockLineage]);
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
