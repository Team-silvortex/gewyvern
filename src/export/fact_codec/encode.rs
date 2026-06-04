use super::{
    BTreeMap, FactEnvelope, FactKind, FactKindTag, JsonValue, RejectedFact,
    RejectedFactSummaryItem, comm_to_string, quic_frame_type_id, quic_packet_type_id,
    system_time_to_millis,
};

pub(crate) fn fact_json(fact: &FactEnvelope) -> JsonValue {
    let kind = match &fact.kind {
        FactKind::TcpState(value) => JsonValue::Object(BTreeMap::from([
            (
                "tag".into(),
                JsonValue::String(FactKindTag::TcpState.to_string()),
            ),
            ("netns".into(), JsonValue::Number(value.netns as i64)),
            (
                "sk_cookie".into(),
                JsonValue::Number(value.sk_cookie as i64),
            ),
            ("sport".into(), JsonValue::Number(value.sport as i64)),
            ("dport".into(), JsonValue::Number(value.dport as i64)),
            ("family".into(), JsonValue::Number(value.family as i64)),
            ("old".into(), JsonValue::Number(value.old as i64)),
            ("new".into(), JsonValue::Number(value.new as i64)),
        ])),
        FactKind::PacketMeta(value) => JsonValue::Object(BTreeMap::from([
            (
                "tag".into(),
                JsonValue::String(FactKindTag::PacketMeta.to_string()),
            ),
            ("netns".into(), JsonValue::Number(value.netns as i64)),
            (
                "sk_cookie".into(),
                value
                    .sk_cookie
                    .map_or(JsonValue::Null, |v| JsonValue::Number(v as i64)),
            ),
            ("dir".into(), JsonValue::String(value.dir.as_str().into())),
            (
                "local_port".into(),
                value
                    .local_port
                    .map_or(JsonValue::Null, |v| JsonValue::Number(v as i64)),
            ),
            (
                "remote_port".into(),
                value
                    .remote_port
                    .map_or(JsonValue::Null, |v| JsonValue::Number(v as i64)),
            ),
            (
                "payload_byte0".into(),
                value
                    .payload_byte0
                    .map_or(JsonValue::Null, |v| JsonValue::Number(v as i64)),
            ),
            (
                "payload_byte1".into(),
                value
                    .payload_byte1
                    .map_or(JsonValue::Null, |v| JsonValue::Number(v as i64)),
            ),
            (
                "payload_prefix2".into(),
                value
                    .payload_prefix2
                    .map_or(JsonValue::Null, |v| JsonValue::Number(v as i64)),
            ),
            (
                "payload_prefix4".into(),
                value
                    .payload_prefix4
                    .map_or(JsonValue::Null, |v| JsonValue::Number(v as i64)),
            ),
            (
                "payload_byte4".into(),
                value
                    .payload_byte4
                    .map_or(JsonValue::Null, |v| JsonValue::Number(v as i64)),
            ),
            (
                "payload_byte5".into(),
                value
                    .payload_byte5
                    .map_or(JsonValue::Null, |v| JsonValue::Number(v as i64)),
            ),
            (
                "payload_byte9".into(),
                value
                    .payload_byte9
                    .map_or(JsonValue::Null, |v| JsonValue::Number(v as i64)),
            ),
            (
                "payload_byte10".into(),
                value
                    .payload_byte10
                    .map_or(JsonValue::Null, |v| JsonValue::Number(v as i64)),
            ),
            (
                "payload_byte13".into(),
                value
                    .payload_byte13
                    .map_or(JsonValue::Null, |v| JsonValue::Number(v as i64)),
            ),
            (
                "payload_bytes".into(),
                JsonValue::Array(
                    value
                        .payload_bytes
                        .iter()
                        .map(|(offset, byte)| {
                            JsonValue::Object(BTreeMap::from([
                                ("offset".into(), JsonValue::Number(*offset as i64)),
                                ("value".into(), JsonValue::Number(*byte as i64)),
                            ]))
                        })
                        .collect(),
                ),
            ),
            ("l3_proto".into(), JsonValue::Number(value.l3_proto as i64)),
            ("l4_proto".into(), JsonValue::Number(value.l4_proto as i64)),
            ("tot_len".into(), JsonValue::Number(value.tot_len as i64)),
            (
                "tcp_flags".into(),
                JsonValue::Number(value.tcp_flags as i64),
            ),
            (
                "seq".into(),
                value
                    .seq
                    .map_or(JsonValue::Null, |v| JsonValue::Number(v as i64)),
            ),
            (
                "ack".into(),
                value
                    .ack
                    .map_or(JsonValue::Null, |v| JsonValue::Number(v as i64)),
            ),
            (
                "window".into(),
                value
                    .window
                    .map_or(JsonValue::Null, |v| JsonValue::Number(v as i64)),
            ),
        ])),
        FactKind::QuicMeta(value) => JsonValue::Object(BTreeMap::from([
            (
                "tag".into(),
                JsonValue::String(FactKindTag::QuicMeta.to_string()),
            ),
            ("netns".into(), JsonValue::Number(value.netns as i64)),
            (
                "sk_cookie".into(),
                value
                    .sk_cookie
                    .map_or(JsonValue::Null, |v| JsonValue::Number(v as i64)),
            ),
            ("dir".into(), JsonValue::String(value.dir.as_str().into())),
            (
                "local_port".into(),
                value
                    .local_port
                    .map_or(JsonValue::Null, |v| JsonValue::Number(v as i64)),
            ),
            (
                "remote_port".into(),
                value
                    .remote_port
                    .map_or(JsonValue::Null, |v| JsonValue::Number(v as i64)),
            ),
            ("long_header".into(), JsonValue::Bool(value.long_header)),
            (
                "packet_type".into(),
                value.packet_type.map_or(JsonValue::Null, |packet_type| {
                    JsonValue::String(quic_packet_type_id(&packet_type).into())
                }),
            ),
            (
                "frame_types".into(),
                JsonValue::Array(
                    value
                        .frame_types
                        .iter()
                        .map(|frame_type| JsonValue::String(quic_frame_type_id(frame_type).into()))
                        .collect(),
                ),
            ),
            (
                "payload_bytes".into(),
                JsonValue::Array(
                    value
                        .payload_bytes
                        .iter()
                        .map(|(offset, byte)| {
                            JsonValue::Object(BTreeMap::from([
                                ("offset".into(), JsonValue::Number(*offset as i64)),
                                ("value".into(), JsonValue::Number(*byte as i64)),
                            ]))
                        })
                        .collect(),
                ),
            ),
        ])),
        FactKind::RouteDecision(value) => JsonValue::Object(BTreeMap::from([
            (
                "tag".into(),
                JsonValue::String(FactKindTag::RouteDecision.to_string()),
            ),
            ("netns".into(), JsonValue::Number(value.netns as i64)),
            (
                "sk_cookie".into(),
                value
                    .sk_cookie
                    .map_or(JsonValue::Null, |v| JsonValue::Number(v as i64)),
            ),
            (
                "fib_table".into(),
                value
                    .fib_table
                    .map_or(JsonValue::Null, |v| JsonValue::Number(v as i64)),
            ),
            ("oif".into(), JsonValue::Number(value.oif as i64)),
        ])),
        FactKind::SockLineage(value) => JsonValue::Object(BTreeMap::from([
            (
                "tag".into(),
                JsonValue::String(FactKindTag::SockLineage.to_string()),
            ),
            ("netns".into(), JsonValue::Number(value.netns as i64)),
            (
                "sk_cookie".into(),
                JsonValue::Number(value.sk_cookie as i64),
            ),
            ("pid".into(), JsonValue::Number(value.pid as i64)),
            ("tid".into(), JsonValue::Number(value.tid as i64)),
            (
                "cgroup_id".into(),
                JsonValue::Number(value.cgroup_id as i64),
            ),
            (
                "comm".into(),
                JsonValue::String(comm_to_string(&value.comm)),
            ),
        ])),
        FactKind::DropAction(value) => JsonValue::Object(BTreeMap::from([
            (
                "tag".into(),
                JsonValue::String(FactKindTag::DropAction.to_string()),
            ),
            ("flow".into(), JsonValue::Number(value.flow as i64)),
            (
                "reason_id".into(),
                JsonValue::Number(value.reason_id as i64),
            ),
            (
                "packet_fact".into(),
                JsonValue::Number(value.packet_fact.0 as i64),
            ),
            (
                "verdict".into(),
                JsonValue::String(value.verdict.as_str().into()),
            ),
        ])),
        FactKind::AttachScope(value) => JsonValue::Object(BTreeMap::from([
            (
                "tag".into(),
                JsonValue::String(FactKindTag::AttachScope.to_string()),
            ),
            (
                "scope_hash".into(),
                JsonValue::Number(value.scope_hash as i64),
            ),
            ("complete".into(), JsonValue::Bool(value.complete)),
        ])),
    };

    JsonValue::Object(BTreeMap::from([
        ("id".into(), JsonValue::Number(fact.id.0 as i64)),
        (
            "ts_ms".into(),
            JsonValue::Number(system_time_to_millis(fact.ts) as i64),
        ),
        ("cpu".into(), JsonValue::Number(fact.cpu.0 as i64)),
        (
            "ifindex".into(),
            fact.ifindex
                .map_or(JsonValue::Null, |v| JsonValue::Number(v as i64)),
        ),
        ("session".into(), JsonValue::Number(fact.session.0 as i64)),
        (
            "fragment_id".into(),
            JsonValue::String(fact.fragment_id.clone()),
        ),
        ("kind".into(), kind),
    ]))
}

pub(crate) fn rejected_fact_json(rejected: &RejectedFact) -> JsonValue {
    JsonValue::Object(BTreeMap::from([
        ("id".into(), JsonValue::Number(rejected.id.0 as i64)),
        (
            "fragment_id".into(),
            JsonValue::String(rejected.fragment_id.clone()),
        ),
        (
            "reason".into(),
            JsonValue::String(rejected.reason.label().into()),
        ),
    ]))
}

pub(crate) fn rejected_fact_summary_json(summary: &RejectedFactSummaryItem) -> JsonValue {
    JsonValue::Object(BTreeMap::from([
        (
            "fragment_id".into(),
            JsonValue::String(summary.fragment_id.clone()),
        ),
        ("reason".into(), JsonValue::String(summary.reason.clone())),
        ("count".into(), JsonValue::Number(summary.count as i64)),
    ]))
}
