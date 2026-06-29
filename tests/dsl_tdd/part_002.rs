use super::*;

#[test]
fn dsl_accepts_flow_direction_predicates() {
    let packet_binding = compile_str(
        r#"
template(:http_client_direction)
|> window(:default_5s)
|> reason(:handshake_l1)
|> fragment(:tcp_packet_meta_fragment)
|> program_model(:http_client_direction_model)
|> operation(:http_request)
|> program_rule(predicate: "packet_observed:tcp:local_to_remote", stage: :packet_observed, narrative: "static:outbound http payload observed", dedupe: true)
"#,
    )
    .unwrap();
    let datagram_binding = compile_str(
        r#"
template(:dns_direction)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> fragment(:udp_packet_meta_fragment)
|> program_model(:dns_direction_model)
|> operation(:dns_lookup)
|> program_rule(predicate: "datagram_observed:udp:remote_to_local", stage: :datagram_observed, narrative: "static:inbound dns datagram observed", dedupe: true)
"#,
    )
    .unwrap();

    let packet_rule = &packet_binding
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
template(:redis_resp_packet_match)
|> window(:default_5s)
|> reason(:handshake_l1)
|> fragment(:tcp_packet_meta_fragment)
|> program_model(:redis_resp_packet_match_model)
|> operation(:redis_ping)
|> program_rule(predicate: "packet_observed:tcp:remote:redis:local_to_remote:byte0_mask:0xff:0x2a", stage: :packet_observed, narrative: :transport_payload_sent, dedupe: true)
|> program_rule(predicate: "packet_observed:tcp:remote:redis:remote_to_local:prefix4:0x2b504f4e", stage: :packet_observed, narrative: :transport_payload_received, dedupe: true)
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
template(:dns_tcp_packet_match)
|> window(:default_5s)
|> reason(:handshake_l1)
|> fragment(:tcp_packet_meta_fragment)
|> program_model(:dns_tcp_packet_match_model)
|> operation(:dns_tcp_query)
|> program_rule(predicate: "packet_observed:tcp:remote:53:remote_to_local:byte4_mask:0x80:0x80", stage: :packet_observed, narrative: :transport_payload_received, dedupe: true)
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
template(:quic_port_match)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> fragment(:udp_packet_meta_fragment)
|> program_model(:quic_port_match_model)
|> operation(:quic_client_initial)
|> program_rule(predicate: "datagram_observed:udp:remote:quic:local_to_remote", stage: :datagram_observed, narrative: :udp_datagram_sent, dedupe: true)
"#,
    )
    .unwrap();

    let rule = &remote_quic_binding
        .template
        .program_model
        .as_ref()
        .unwrap()
        .rules[0];
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
template(:quic_packet_match)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> fragment(:udp_packet_meta_fragment)
|> program_model(:quic_packet_match_model)
|> operation(:quic_client_initial)
|> program_rule(predicate: "quic_packet_observed:remote:quic:local_to_remote:min_len:1200:long_header:true:type:initial", stage: :datagram_observed, narrative: :udp_datagram_sent, dedupe: true)
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
template(:quic_frame_match)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> fragment(:udp_packet_meta_fragment)
|> program_model(:quic_frame_match_model)
|> operation(:quic_crypto_handshake)
|> program_rule(predicate: "quic_frame_observed:remote:quic:remote_to_local:type:handshake:frame:crypto", stage: :packet_observed, narrative: :transport_payload_received, dedupe: true)
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
template(:quic_initial_len_match)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> fragment(:udp_packet_meta_fragment)
|> program_model(:quic_initial_len_match_model)
|> operation(:quic_client_initial)
|> program_rule(predicate: "datagram_observed:udp:remote:quic:local_to_remote:min_len:1200", stage: :datagram_observed, narrative: :udp_datagram_sent, dedupe: true)
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
template(:quic_initial_byte_match)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> fragment(:udp_packet_meta_fragment)
|> program_model(:quic_initial_byte_match_model)
|> operation(:quic_client_initial)
|> program_rule(predicate: "datagram_observed:udp:remote:quic:local_to_remote:min_len:1200:byte0_mask:0xf0:0xc0", stage: :datagram_observed, narrative: :udp_datagram_sent, dedupe: true)
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
template(:mdns_response_prefix4_match)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> fragment(:udp_packet_meta_fragment)
|> program_model(:mdns_response_prefix4_match_model)
|> operation(:mdns_query)
|> program_rule(predicate: "datagram_observed:udp:remote:mdns:remote_to_local:prefix4:0x00008400", stage: :datagram_observed, narrative: :udp_datagram_received, dedupe: true)
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
template(:snmp_byte13_match)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> fragment(:udp_packet_meta_fragment)
|> program_model(:snmp_byte13_match_model)
|> operation(:snmp_get)
|> program_rule(predicate: "datagram_observed:udp:remote:snmp:byte_at:13:0xff:0xa0", stage: :datagram_observed, narrative: :udp_datagram_sent, dedupe: true)
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
template(:snmp_bytes_sequence_match)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> fragment(:udp_packet_meta_fragment)
|> param(:udp_packet_meta_fragment.sample_payload_offsets, 8)
|> program_model(:snmp_bytes_sequence_match_model)
|> operation(:snmp_get)
|> program_rule(predicate: "datagram_observed:udp:remote:snmp:bytes_at:8:0x30,0x82,0x01", stage: :datagram_observed, narrative: :udp_datagram_sent, dedupe: true)
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
template(:snmp_bytes_sequence_runtime)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> fragment(:udp_packet_meta_fragment)
|> param(:udp_packet_meta_fragment.sample_payload_offsets, 8)
|> program_model(:snmp_bytes_sequence_runtime_model)
|> operation(:snmp_get)
|> program_rule(predicate: "datagram_observed:udp:remote:snmp:local_to_remote:bytes_at:8:0x30,0x82,0x01", stage: :datagram_observed, narrative: :udp_datagram_sent, dedupe: true)
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
template(:directional_narrative_templates)
|> window(duration_ms: 5000, lateness_ms: 200)
|> fragment(:udp_packet_meta_fragment)
|> fragment(:sock_lineage_fragment)
|> operation(:datagram_exchange)
|> program_rule(predicate: :process_bound, stage: :process_bound, narrative: :process_bound, dedupe: true)
|> program_rule(predicate: "datagram_observed:udp:remote_to_local", stage: :datagram_observed, narrative: :udp_datagram_received, dedupe: true)
|> reason_rule(predicate: :process_bound, key_event: :process_identified, narrative: :process_bound, dedupe: true)
|> reason_rule(predicate: "datagram_observed:udp:remote_to_local", key_event: :udp_datagram_seen, narrative: :udp_datagram_received, dedupe: true)
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
