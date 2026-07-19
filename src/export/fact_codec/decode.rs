use super::{
    AttachScopeFact, BTreeMap, CpuId, DropActionFact, DropVerdict, ExportError, FactEnvelope,
    FactId, FactKind, JsonValue, PacketDir, PacketMetaFact, QuicMetaFact, RejectedFact,
    RejectedFactReason, RejectedFactSummaryItem, RouteDecisionFact, SessionId, SockLineageFact,
    TcpStateFact, millis_to_system_time, parse_optional_u64, string_to_comm,
};
use crate::export::reason_codec::{parse_quic_frame_type, parse_quic_packet_type};

pub(crate) fn parse_fact(value: &JsonValue) -> Result<FactEnvelope, ExportError> {
    let object = value.as_object()?;
    let kind = object
        .get("kind")
        .ok_or_else(|| ExportError::InvalidShape("fact.kind".into()))?
        .as_object()?;
    let tag = kind
        .get("tag")
        .ok_or_else(|| ExportError::InvalidShape("fact.kind.tag".into()))?
        .as_str()?;

    let fact_kind = match tag {
        "tcp_state" => FactKind::TcpState(TcpStateFact {
            netns: kind
                .get("netns")
                .ok_or_else(|| ExportError::InvalidShape("fact.tcp_state.netns".into()))?
                .as_i64()? as u32,
            sk_cookie: kind
                .get("sk_cookie")
                .ok_or_else(|| ExportError::InvalidShape("fact.tcp_state.sk_cookie".into()))?
                .as_i64()? as u64,
            saddr: [0; 16],
            daddr: [0; 16],
            sport: kind
                .get("sport")
                .ok_or_else(|| ExportError::InvalidShape("fact.tcp_state.sport".into()))?
                .as_i64()? as u16,
            dport: kind
                .get("dport")
                .ok_or_else(|| ExportError::InvalidShape("fact.tcp_state.dport".into()))?
                .as_i64()? as u16,
            family: kind
                .get("family")
                .ok_or_else(|| ExportError::InvalidShape("fact.tcp_state.family".into()))?
                .as_i64()? as u8,
            old: kind
                .get("old")
                .ok_or_else(|| ExportError::InvalidShape("fact.tcp_state.old".into()))?
                .as_i64()? as u8,
            new: kind
                .get("new")
                .ok_or_else(|| ExportError::InvalidShape("fact.tcp_state.new".into()))?
                .as_i64()? as u8,
        }),
        "packet_meta" => FactKind::PacketMeta(PacketMetaFact {
            netns: kind
                .get("netns")
                .ok_or_else(|| ExportError::InvalidShape("fact.packet_meta.netns".into()))?
                .as_i64()? as u32,
            sk_cookie: parse_optional_u64(kind.get("sk_cookie").unwrap_or(&JsonValue::Null))?,
            dir: PacketDir::from_str(
                kind.get("dir")
                    .ok_or_else(|| ExportError::InvalidShape("fact.packet_meta.dir".into()))?
                    .as_str()?,
            )
            .ok_or_else(|| ExportError::InvalidValue("unknown packet dir".into()))?,
            local_port: parse_optional_u16(
                kind.get("local_port")
                    .or_else(|| kind.get("sport"))
                    .unwrap_or(&JsonValue::Null),
            )?,
            remote_port: parse_optional_u16(
                kind.get("remote_port")
                    .or_else(|| kind.get("dport"))
                    .unwrap_or(&JsonValue::Null),
            )?,
            payload_byte0: match kind.get("payload_byte0").unwrap_or(&JsonValue::Null) {
                JsonValue::Null => None,
                value => Some(value.as_i64()? as u8),
            },
            payload_byte1: match kind.get("payload_byte1").unwrap_or(&JsonValue::Null) {
                JsonValue::Null => None,
                value => Some(value.as_i64()? as u8),
            },
            payload_prefix2: match kind.get("payload_prefix2").unwrap_or(&JsonValue::Null) {
                JsonValue::Null => None,
                value => Some(value.as_i64()? as u16),
            },
            payload_prefix4: match kind.get("payload_prefix4").unwrap_or(&JsonValue::Null) {
                JsonValue::Null => None,
                value => Some(value.as_i64()? as u32),
            },
            payload_byte4: match kind.get("payload_byte4").unwrap_or(&JsonValue::Null) {
                JsonValue::Null => None,
                value => Some(value.as_i64()? as u8),
            },
            payload_byte5: match kind.get("payload_byte5").unwrap_or(&JsonValue::Null) {
                JsonValue::Null => None,
                value => Some(value.as_i64()? as u8),
            },
            payload_byte9: match kind.get("payload_byte9").unwrap_or(&JsonValue::Null) {
                JsonValue::Null => None,
                value => Some(value.as_i64()? as u8),
            },
            payload_byte10: match kind.get("payload_byte10").unwrap_or(&JsonValue::Null) {
                JsonValue::Null => None,
                value => Some(value.as_i64()? as u8),
            },
            payload_byte13: match kind.get("payload_byte13").unwrap_or(&JsonValue::Null) {
                JsonValue::Null => None,
                value => Some(value.as_i64()? as u8),
            },
            payload_bytes: match kind.get("payload_bytes").unwrap_or(&JsonValue::Null) {
                JsonValue::Null => BTreeMap::new(),
                JsonValue::Array(items) => {
                    let mut map = BTreeMap::new();
                    for item in items {
                        let JsonValue::Object(object) = item else {
                            return Err(ExportError::InvalidShape(
                                "fact.packet_meta.payload_bytes".into(),
                            ));
                        };
                        let offset = object
                            .get("offset")
                            .ok_or_else(|| {
                                ExportError::InvalidShape(
                                    "fact.packet_meta.payload_bytes.offset".into(),
                                )
                            })?
                            .as_i64()? as u16;
                        let value = object
                            .get("value")
                            .ok_or_else(|| {
                                ExportError::InvalidShape(
                                    "fact.packet_meta.payload_bytes.value".into(),
                                )
                            })?
                            .as_i64()? as u8;
                        map.insert(offset, value);
                    }
                    map
                }
                _ => {
                    return Err(ExportError::InvalidShape(
                        "fact.packet_meta.payload_bytes".into(),
                    ));
                }
            },
            l3_proto: kind
                .get("l3_proto")
                .ok_or_else(|| ExportError::InvalidShape("fact.packet_meta.l3_proto".into()))?
                .as_i64()? as u16,
            l4_proto: kind
                .get("l4_proto")
                .ok_or_else(|| ExportError::InvalidShape("fact.packet_meta.l4_proto".into()))?
                .as_i64()? as u8,
            tot_len: kind
                .get("tot_len")
                .ok_or_else(|| ExportError::InvalidShape("fact.packet_meta.tot_len".into()))?
                .as_i64()? as u32,
            tcp_flags: kind
                .get("tcp_flags")
                .ok_or_else(|| ExportError::InvalidShape("fact.packet_meta.tcp_flags".into()))?
                .as_i64()? as u16,
            seq: parse_optional_u32(kind.get("seq").unwrap_or(&JsonValue::Null))?,
            ack: parse_optional_u32(kind.get("ack").unwrap_or(&JsonValue::Null))?,
            window: parse_optional_u16(kind.get("window").unwrap_or(&JsonValue::Null))?,
        }),
        "quic_meta" => FactKind::QuicMeta(QuicMetaFact {
            netns: kind
                .get("netns")
                .ok_or_else(|| ExportError::InvalidShape("fact.quic_meta.netns".into()))?
                .as_i64()? as u32,
            sk_cookie: parse_optional_u64(kind.get("sk_cookie").unwrap_or(&JsonValue::Null))?,
            dir: PacketDir::from_str(
                kind.get("dir")
                    .ok_or_else(|| ExportError::InvalidShape("fact.quic_meta.dir".into()))?
                    .as_str()?,
            )
            .ok_or_else(|| ExportError::InvalidValue("unknown quic packet dir".into()))?,
            local_port: parse_optional_u16(kind.get("local_port").unwrap_or(&JsonValue::Null))?,
            remote_port: parse_optional_u16(kind.get("remote_port").unwrap_or(&JsonValue::Null))?,
            long_header: kind
                .get("long_header")
                .unwrap_or(&JsonValue::Bool(false))
                .as_bool()?,
            packet_type: match kind.get("packet_type").unwrap_or(&JsonValue::Null) {
                JsonValue::Null => None,
                JsonValue::String(value) => Some(parse_quic_packet_type(value)?),
                _ => {
                    return Err(ExportError::InvalidShape(
                        "fact.quic_meta.packet_type".into(),
                    ));
                }
            },
            frame_types: match kind.get("frame_types").unwrap_or(&JsonValue::Null) {
                JsonValue::Null => Vec::new(),
                JsonValue::Array(items) => items
                    .iter()
                    .map(|item| match item {
                        JsonValue::String(value) => parse_quic_frame_type(value),
                        _ => Err(ExportError::InvalidShape(
                            "fact.quic_meta.frame_types".into(),
                        )),
                    })
                    .collect::<Result<Vec<_>, _>>()?,
                _ => {
                    return Err(ExportError::InvalidShape(
                        "fact.quic_meta.frame_types".into(),
                    ));
                }
            },
            payload_bytes: match kind.get("payload_bytes").unwrap_or(&JsonValue::Null) {
                JsonValue::Null => BTreeMap::new(),
                JsonValue::Array(items) => items
                    .iter()
                    .map(|item| {
                        let object = item.as_object()?;
                        Ok((
                            object
                                .get("offset")
                                .ok_or_else(|| {
                                    ExportError::InvalidShape(
                                        "fact.quic_meta.payload_bytes.offset".into(),
                                    )
                                })?
                                .as_i64()? as u16,
                            object
                                .get("value")
                                .ok_or_else(|| {
                                    ExportError::InvalidShape(
                                        "fact.quic_meta.payload_bytes.value".into(),
                                    )
                                })?
                                .as_i64()? as u8,
                        ))
                    })
                    .collect::<Result<BTreeMap<_, _>, _>>()?,
                _ => {
                    return Err(ExportError::InvalidShape(
                        "fact.quic_meta.payload_bytes".into(),
                    ));
                }
            },
        }),
        "route_decision" => FactKind::RouteDecision(RouteDecisionFact {
            netns: kind
                .get("netns")
                .ok_or_else(|| ExportError::InvalidShape("fact.route_decision.netns".into()))?
                .as_i64()? as u32,
            sk_cookie: parse_optional_u64(kind.get("sk_cookie").unwrap_or(&JsonValue::Null))?,
            fib_table: parse_optional_u32(kind.get("fib_table").unwrap_or(&JsonValue::Null))?,
            oif: kind
                .get("oif")
                .ok_or_else(|| ExportError::InvalidShape("fact.route_decision.oif".into()))?
                .as_i64()? as u32,
            gw: None,
        }),
        "sock_lineage" => FactKind::SockLineage(SockLineageFact {
            netns: kind
                .get("netns")
                .ok_or_else(|| ExportError::InvalidShape("fact.sock_lineage.netns".into()))?
                .as_i64()? as u32,
            sk_cookie: kind
                .get("sk_cookie")
                .ok_or_else(|| ExportError::InvalidShape("fact.sock_lineage.sk_cookie".into()))?
                .as_i64()? as u64,
            pid: kind
                .get("pid")
                .ok_or_else(|| ExportError::InvalidShape("fact.sock_lineage.pid".into()))?
                .as_i64()? as u32,
            tid: kind
                .get("tid")
                .ok_or_else(|| ExportError::InvalidShape("fact.sock_lineage.tid".into()))?
                .as_i64()? as u32,
            cgroup_id: kind
                .get("cgroup_id")
                .ok_or_else(|| ExportError::InvalidShape("fact.sock_lineage.cgroup_id".into()))?
                .as_i64()? as u64,
            comm: string_to_comm(
                kind.get("comm")
                    .unwrap_or(&JsonValue::String(String::new()))
                    .as_str()?,
            ),
        }),
        "drop_action" => FactKind::DropAction(DropActionFact {
            flow: kind
                .get("flow")
                .ok_or_else(|| ExportError::InvalidShape("fact.drop_action.flow".into()))?
                .as_i64()? as u64,
            reason_id: kind
                .get("reason_id")
                .ok_or_else(|| ExportError::InvalidShape("fact.drop_action.reason_id".into()))?
                .as_i64()? as u64,
            packet_fact: FactId(
                kind.get("packet_fact")
                    .ok_or_else(|| {
                        ExportError::InvalidShape("fact.drop_action.packet_fact".into())
                    })?
                    .as_i64()? as u64,
            ),
            verdict: DropVerdict::from_str(
                kind.get("verdict")
                    .ok_or_else(|| ExportError::InvalidShape("fact.drop_action.verdict".into()))?
                    .as_str()?,
            )
            .ok_or_else(|| ExportError::InvalidValue("unknown verdict".into()))?,
        }),
        "attach_scope" => FactKind::AttachScope(AttachScopeFact {
            scope_hash: kind
                .get("scope_hash")
                .ok_or_else(|| ExportError::InvalidShape("fact.attach_scope.scope_hash".into()))?
                .as_i64()? as u64,
            complete: kind
                .get("complete")
                .ok_or_else(|| ExportError::InvalidShape("fact.attach_scope.complete".into()))?
                .as_bool()?,
        }),
        _ => return Err(ExportError::InvalidValue("unknown fact tag".into())),
    };

    Ok(FactEnvelope {
        id: FactId(
            object
                .get("id")
                .ok_or_else(|| ExportError::InvalidShape("fact.id".into()))?
                .as_i64()? as u64,
        ),
        ts: millis_to_system_time(
            object
                .get("ts_ms")
                .ok_or_else(|| ExportError::InvalidShape("fact.ts_ms".into()))?
                .as_i64()? as u64,
        ),
        cpu: CpuId(
            object
                .get("cpu")
                .ok_or_else(|| ExportError::InvalidShape("fact.cpu".into()))?
                .as_i64()? as u16,
        ),
        ifindex: parse_optional_u32(object.get("ifindex").unwrap_or(&JsonValue::Null))?,
        session: SessionId(
            object
                .get("session")
                .ok_or_else(|| ExportError::InvalidShape("fact.session".into()))?
                .as_i64()? as u64,
        ),
        fragment_id: object
            .get("fragment_id")
            .ok_or_else(|| ExportError::InvalidShape("fact.fragment_id".into()))?
            .as_str()?
            .to_string(),
        kind: fact_kind,
    })
}

pub(crate) fn parse_rejected_fact(value: &JsonValue) -> Result<RejectedFact, ExportError> {
    let object = value.as_object()?;
    let reason = match object
        .get("reason")
        .ok_or_else(|| ExportError::InvalidShape("rejected_fact.reason".into()))?
        .as_str()?
    {
        "fragment_not_loaded" => RejectedFactReason::FragmentNotLoaded,
        "filtered_by_fragment_param" => RejectedFactReason::FilteredByFragmentParam,
        "before_window_start" => RejectedFactReason::BeforeWindowStart,
        "after_lateness_cutoff" => RejectedFactReason::AfterLatenessCutoff,
        other => {
            return Err(ExportError::InvalidValue(format!(
                "unknown rejected fact reason: {other}"
            )));
        }
    };

    Ok(RejectedFact {
        id: FactId(
            object
                .get("id")
                .ok_or_else(|| ExportError::InvalidShape("rejected_fact.id".into()))?
                .as_i64()? as u64,
        ),
        fragment_id: object
            .get("fragment_id")
            .ok_or_else(|| ExportError::InvalidShape("rejected_fact.fragment_id".into()))?
            .as_str()?
            .to_string(),
        reason,
    })
}

pub(crate) fn parse_rejected_fact_summary(
    value: &JsonValue,
) -> Result<RejectedFactSummaryItem, ExportError> {
    let object = value.as_object()?;
    Ok(RejectedFactSummaryItem {
        fragment_id: object
            .get("fragment_id")
            .ok_or_else(|| ExportError::InvalidShape("rejected_fact_summary.fragment_id".into()))?
            .as_str()?
            .to_string(),
        reason: object
            .get("reason")
            .ok_or_else(|| ExportError::InvalidShape("rejected_fact_summary.reason".into()))?
            .as_str()?
            .to_string(),
        count: object
            .get("count")
            .ok_or_else(|| ExportError::InvalidShape("rejected_fact_summary.count".into()))?
            .as_i64()? as u64,
    })
}

pub(crate) fn parse_fact_ids(value: &JsonValue) -> Result<Vec<FactId>, ExportError> {
    value
        .as_array()?
        .iter()
        .map(|item| Ok(FactId(item.as_i64()? as u64)))
        .collect()
}

pub(crate) fn parse_optional_u32(value: &JsonValue) -> Result<Option<u32>, ExportError> {
    parse_optional_u64(value).map(|value| value.map(|item| item as u32))
}

pub(crate) fn parse_optional_u16(value: &JsonValue) -> Result<Option<u16>, ExportError> {
    parse_optional_u64(value).map(|value| value.map(|item| item as u16))
}

pub(crate) fn parse_optional_u8(value: &JsonValue) -> Result<Option<u8>, ExportError> {
    parse_optional_u64(value).map(|value| value.map(|item| item as u8))
}

pub(crate) fn parse_optional_fact_id(value: &JsonValue) -> Result<Option<FactId>, ExportError> {
    parse_optional_u64(value).map(|value| value.map(FactId))
}
