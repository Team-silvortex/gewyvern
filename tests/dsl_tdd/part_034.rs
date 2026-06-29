use super::*;

#[test]
fn binding_diagnostics_reports_unsupported_quic_frame_payload_offsets() {
    let binding = parse_str_unvalidated(
        r#"
template(:unsupported_quic_frame_payload_offset)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> fragment(:udp_packet_meta_fragment)
|> program_model(:unsupported_quic_frame_payload_offset_model)
|> operation(:quic_crypto_handshake)
|> program_rule(predicate: "quic_frame_observed:remote:quic:local_to_remote:frame:crypto:byte_at:8:0xff:0xa0", stage: :datagram_observed, narrative: :udp_datagram_sent, dedupe: true)
"#,
    )
    .unwrap();

    let diagnostics = collect_binding_diagnostics(&binding).unwrap();
    let rule = &diagnostics.program_model.as_ref().unwrap().rules[0];
    assert!(!rule.supported);
    assert_eq!(rule.unsupported_payload_offsets, vec![8]);
}

#[test]
fn binding_diagnostics_accept_dynamic_sample_payload_offsets_from_fragment_params() {
    let binding = parse_str_unvalidated(
        r#"
template(:dynamic_payload_offset_support)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> fragment(:udp_packet_meta_fragment)
|> param(:udp_packet_meta_fragment.sample_payload_offsets, 8)
|> program_model(:dynamic_payload_offset_support_model)
|> operation(:snmp_get)
|> program_rule(predicate: "datagram_observed:udp:remote:snmp:byte_at:8:0xff:0xa0", stage: :datagram_observed, narrative: :udp_datagram_sent, dedupe: true)
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
template(:unsupported_payload_offset_compile)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> fragment(:udp_packet_meta_fragment)
|> program_model(:unsupported_payload_offset_compile_model)
|> operation(:snmp_get)
|> program_rule(predicate: "datagram_observed:udp:remote:snmp:byte_at:8:0xff:0xa0", stage: :datagram_observed, narrative: :udp_datagram_sent, dedupe: true)
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
template(:udp_process_core_lineage)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> fragment(:udp_packet_meta_fragment)
|> fragment(:route_meta_fragment)
|> fragment(:sock_lineage_fragment)
|> operation(:datagram_exchange)
|> program_rule(predicate: :process_bound, stage: :process_bound, narrative: :process_bound, dedupe: true)
|> program_rule(predicate: "datagram_observed:udp", stage: :datagram_observed, narrative: :udp_datagram_observed, dedupe: true)
|> program_rule(predicate: :route_resolved, stage: :route_resolved, narrative: :route_changed, dedupe: true)
|> evidence(:sock_lineage, :core_requirement)
|> evidence(:packet_meta, :optional_enhancement)
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
template(:udp_process_debug)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> fragment(:udp_packet_meta_fragment)
|> fragment(:route_meta_fragment)
|> fragment(:sock_lineage_fragment)
|> program_model(:datagram_exchange_v1)
|> operation(:datagram_exchange)
|> program_rule(predicate: :process_bound, stage: :process_bound, narrative: :process_bound, dedupe: true)
|> param(:sock_lineage_fragment.not_a_real_param, true)
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
template(:udp_process_debug)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> fragment(:udp_packet_meta_fragment)
|> fragment(:route_meta_fragment)
|> fragment(:sock_lineage_fragment)
|> program_model(:datagram_exchange_v1)
|> operation(:datagram_exchange)
|> program_rule(predicate: :process_bound, stage: :process_bound, narrative: :process_bound, dedupe: true)
|> param(:udp_packet_meta_fragment.min_len, false)
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
template(:quic_initial_prefix2_match)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> fragment(:udp_packet_meta_fragment)
|> program_model(:quic_initial_prefix2_match_model)
|> operation(:quic_client_initial)
|> program_rule(predicate: "datagram_observed:udp:remote:quic:local_to_remote:min_len:1200:byte0_mask:0xf0:0xc0:prefix2:0xc300", stage: :datagram_observed, narrative: :udp_datagram_sent, dedupe: true)
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
