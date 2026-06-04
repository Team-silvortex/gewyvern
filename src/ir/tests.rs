use super::{
    FlowPredicate, NarrativeTemplate, ObservationScope, PayloadByteMatch, PayloadByteSequenceMatch,
    PayloadMatcherSetRef, QuicFrameType, QuicPacketType, SignalKind, render_phase_transition_kind,
};
use crate::ledger::{FactKindTag, PacketDir, PacketMetaFact, QuicMetaFact};
use std::collections::BTreeMap;

#[test]
fn payload_matcher_set_ref_matches_packet_and_quic_payloads() {
    let byte_matches = [PayloadByteMatch {
        offset: 8,
        mask: 0xff,
        value: 0xa0,
    }];
    let byte_sequences = [PayloadByteSequenceMatch {
        offset: 10,
        bytes: vec![0xde, 0xad],
    }];
    let matchers = PayloadMatcherSetRef::new(&byte_matches, &byte_sequences);
    let packet = PacketMetaFact {
        netns: 1,
        sk_cookie: Some(7),
        dir: PacketDir::Egress,
        local_port: Some(12345),
        remote_port: Some(443),
        payload_byte0: None,
        payload_byte1: None,
        payload_prefix2: None,
        payload_prefix4: None,
        payload_byte4: None,
        payload_byte5: None,
        payload_byte9: None,
        payload_byte10: Some(0xde),
        payload_byte13: None,
        payload_bytes: BTreeMap::from([(8, 0xa0), (10, 0xde), (11, 0xad)]),
        l3_proto: 0x0800,
        l4_proto: 17,
        tot_len: 1280,
        tcp_flags: 0,
        seq: None,
        ack: None,
        window: None,
    };
    let quic = QuicMetaFact {
        netns: 1,
        sk_cookie: Some(7),
        dir: PacketDir::Egress,
        local_port: Some(12345),
        remote_port: Some(443),
        long_header: true,
        packet_type: None,
        frame_types: vec![QuicFrameType::Crypto],
        payload_bytes: BTreeMap::from([(8, 0xa0), (10, 0xde), (11, 0xad)]),
    };

    assert!(matchers.matches_packet(&packet));
    assert!(matchers.matches_payload_bytes(&quic.payload_bytes));
    assert_eq!(matchers.required_offsets(), vec![8, 10, 11]);
}

#[test]
fn flow_predicate_required_payload_offsets_include_quic_frame_matchers() {
    let predicate = FlowPredicate::QuicFrameObserved {
        dir: Some(PacketDir::Egress),
        local_port: None,
        remote_port: Some(443),
        packet_type: None,
        frame_type: QuicFrameType::Crypto,
        byte_matches: vec![PayloadByteMatch {
            offset: 8,
            mask: 0xff,
            value: 0xa0,
        }],
        byte_sequences: vec![PayloadByteSequenceMatch {
            offset: 10,
            bytes: vec![0xde, 0xad],
        }],
    };

    assert_eq!(predicate.required_payload_offsets(), vec![8, 10, 11]);
}

#[test]
fn flow_predicate_exposes_observation_scope_for_transport_predicates() {
    let predicate = FlowPredicate::DatagramObserved {
        l4_proto: 17,
        dir: Some(PacketDir::Ingress),
        local_port: Some(53),
        remote_port: Some(42000),
        min_len: Some(64),
        first_byte_mask: None,
        first_byte_value: None,
        prefix2: None,
        prefix4: None,
        byte13_mask: None,
        byte13_value: None,
        byte_matches: vec![],
        byte_sequences: vec![],
    };

    assert_eq!(
        predicate.observation_scope(),
        Some(ObservationScope {
            dir: Some(PacketDir::Ingress),
            local_port: Some(53),
            remote_port: Some(42000),
        })
    );
}

#[test]
fn flow_predicate_required_fact_kinds_cover_transport_schema() {
    let packet = FlowPredicate::packet_observed(
        6,
        ObservationScope {
            dir: Some(PacketDir::Egress),
            local_port: Some(8080),
            remote_port: Some(443),
        },
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        Vec::new(),
        Vec::new(),
    );
    let quic_frame = FlowPredicate::quic_frame_observed(
        ObservationScope {
            dir: Some(PacketDir::Ingress),
            local_port: Some(443),
            remote_port: Some(53000),
        },
        Some(QuicPacketType::Handshake),
        QuicFrameType::Crypto,
        Vec::new(),
        Vec::new(),
    );
    let composite = FlowPredicate::All(vec![packet.clone(), quic_frame.clone()]);

    assert_eq!(packet.required_fact_kinds(), vec![FactKindTag::PacketMeta]);
    assert_eq!(
        quic_frame.required_fact_kinds(),
        vec![FactKindTag::QuicMeta]
    );
    assert_eq!(
        composite.required_fact_kinds(),
        vec![FactKindTag::PacketMeta, FactKindTag::QuicMeta]
    );
}

#[test]
fn signal_and_narrative_required_fact_kinds_cover_runtime_schema() {
    assert_eq!(
        SignalKind::ProcessIdentified.required_fact_kinds(),
        vec![FactKindTag::SockLineage]
    );
    assert_eq!(
        SignalKind::PacketObserved.required_fact_kinds(),
        vec![FactKindTag::PacketMeta]
    );
    assert_eq!(
        SignalKind::RouteChanged.required_fact_kinds(),
        vec![FactKindTag::RouteDecision]
    );
    assert_eq!(
        NarrativeTemplate::TransportPayloadReceived.required_fact_kinds(),
        vec![FactKindTag::PacketMeta]
    );
    assert_eq!(
        NarrativeTemplate::TcpStateTransition.required_fact_kinds(),
        vec![FactKindTag::TcpState]
    );
    assert_eq!(
        NarrativeTemplate::Static("ok").required_fact_kinds(),
        Vec::new()
    );
}

#[test]
fn signal_kind_phase_kind_covers_transport_and_socket_paths() {
    assert_eq!(
        SignalKind::PacketObserved.phase_kind(Some("send_query")),
        Some("emit_payload")
    );
    assert_eq!(
        SignalKind::PacketObserved.phase_kind(Some("receive_auth_success")),
        Some("receive_payload")
    );
    assert_eq!(
        SignalKind::DatagramObserved.phase_kind(Some("send_register")),
        Some("emit_datagram")
    );
    assert_eq!(
        SignalKind::SocketStateTransition.phase_kind(Some("establish")),
        Some("establish_connection")
    );
    assert_eq!(SignalKind::PacketObserved.phase_kind(None), None);
}

#[test]
fn render_phase_transition_kind_uses_signal_phase_schema() {
    assert_eq!(
        render_phase_transition_kind(
            Some((&SignalKind::SocketStateTransition, Some("connect"))),
            (&SignalKind::PacketObserved, Some("send_query"))
        ),
        "initiate_connection->emit_payload"
    );
    assert_eq!(
        render_phase_transition_kind(None, (&SignalKind::DatagramObserved, Some("receive_ok"))),
        "start->receive_datagram"
    );
}

#[test]
fn signal_kind_suspect_area_covers_runtime_schema() {
    assert_eq!(SignalKind::ProcessBound.suspect_area(), "process_binding");
    assert_eq!(
        SignalKind::SocketStateTransition.suspect_area(),
        "socket_state"
    );
    assert_eq!(SignalKind::PacketObserved.suspect_area(), "transport_io");
    assert_eq!(SignalKind::DatagramObserved.suspect_area(), "datagram_io");
    assert_eq!(SignalKind::RouteChanged.suspect_area(), "route_resolution");
}
