use crate::ir::{FlowPredicate, NarrativeTemplate, ObservationScope, SignalKind};
use crate::reason::{ReasonKeyEvent, ReasonNarrative};

use super::{DslError, parse_bool, split_top_level_with_columns};

mod qualifier;

use self::qualifier::{
    QualifierPartsCursor, parse_named_port, parse_payload_byte_match,
    parse_payload_byte_sequence_match, parse_quic_frame_type, parse_quic_packet_type,
    parse_scope_qualifier, parse_u8_mask_value_qualifier, parse_u16_qualifier, parse_u32_qualifier,
    qualifier_part_at, qualifier_part_opt, split_qualifier_parts_with_columns,
};

pub(crate) fn parse_flow_predicate(value: &str) -> Result<FlowPredicate, DslError> {
    if let Some(inner) = value
        .strip_prefix("all(")
        .and_then(|rest| rest.strip_suffix(')'))
    {
        let base_column = value.find(inner).unwrap_or(0) + 1;
        return Ok(FlowPredicate::All(
            split_top_level_with_columns(inner, ',', base_column)
                .into_iter()
                .map(|(column, part)| {
                    parse_flow_predicate(&part).map_err(|err| err.reanchor_line_column(0, column))
                })
                .collect::<Result<Vec<_>, _>>()?,
        ));
    }
    if let Some(inner) = value
        .strip_prefix("any(")
        .and_then(|rest| rest.strip_suffix(')'))
    {
        let base_column = value.find(inner).unwrap_or(0) + 1;
        return Ok(FlowPredicate::Any(
            split_top_level_with_columns(inner, ',', base_column)
                .into_iter()
                .map(|(column, part)| {
                    parse_flow_predicate(&part).map_err(|err| err.reanchor_line_column(0, column))
                })
                .collect::<Result<Vec<_>, _>>()?,
        ));
    }

    match value {
        "process_bound" => Ok(FlowPredicate::ProcessBound),
        "socket_state_observed" => Ok(FlowPredicate::socket_state_observed(None, None, None)),
        other if other.starts_with("socket_state_observed:") => {
            let suffix = &other["socket_state_observed:".len()..];
            let parts =
                split_qualifier_parts_with_columns(suffix, "socket_state_observed:".len() + 1);
            let mut part_index = 0usize;
            let (first_column, first) = qualifier_part_at(&parts, part_index);
            part_index += 1;
            let (local_port, remote_port, port, port_column) = match first {
                "local" | "sport" => {
                    let (column, value) =
                        qualifier_part_opt(&parts, part_index).ok_or_else(|| {
                            DslError::InvalidValue("missing socket local port qualifier".into())
                                .at_line_column(0, Some(first_column))
                        })?;
                    part_index += 1;
                    (true, false, value, column)
                }
                "remote" | "dport" => {
                    let (column, value) =
                        qualifier_part_opt(&parts, part_index).ok_or_else(|| {
                            DslError::InvalidValue("missing socket remote port qualifier".into())
                                .at_line_column(0, Some(first_column))
                        })?;
                    part_index += 1;
                    (false, true, value, column)
                }
                _ => (false, true, first, first_column),
            };
            let port = parse_named_port(port, "socket_state_observed")
                .map_err(|err| err.reanchor_line_column(0, port_column))?;
            let min_new_state = match qualifier_part_opt(&parts, part_index).map(|(_, value)| value)
            {
                None => None,
                Some("established") => {
                    part_index += 1;
                    Some(3)
                }
                Some(other) => {
                    return Err(DslError::InvalidValue(format!(
                        "unknown socket_state_observed state qualifier '{other}'"
                    ))
                    .at_line_column(0, Some(qualifier_part_at(&parts, part_index).0)));
                }
            };
            if let Some((extra_column, extra)) = qualifier_part_opt(&parts, part_index) {
                return Err(DslError::InvalidValue(format!(
                    "unexpected socket_state_observed suffix '{extra}'"
                ))
                .at_line_column(0, Some(extra_column)));
            }
            Ok(FlowPredicate::socket_state_observed(
                local_port.then_some(port),
                remote_port.then_some(port),
                min_new_state,
            ))
        }
        "route_resolved" => Ok(FlowPredicate::RouteResolved),
        other if other.starts_with("quic_packet_observed:") => {
            let suffix = &other["quic_packet_observed:".len()..];
            let parts =
                split_qualifier_parts_with_columns(suffix, "quic_packet_observed:".len() + 1);
            let mut part_index = 0usize;
            let mut dir = None;
            let mut local_port = None;
            let mut remote_port = None;
            let mut min_len = None;
            let mut long_header = None;
            let mut packet_type = None;
            while let Some((part_column, part)) = qualifier_part_opt(&parts, part_index) {
                part_index += 1;
                if !parse_scope_qualifier(
                    part,
                    &mut QualifierPartsCursor::new(&parts, &mut part_index),
                    "quic_packet_observed",
                    "QUIC",
                    &mut dir,
                    &mut local_port,
                    &mut remote_port,
                )? {
                    match part {
                        "min_len" => {
                            min_len = Some(
                                parse_u32_qualifier(
                                    &mut QualifierPartsCursor::new(&parts, &mut part_index),
                                    "quic_packet_observed",
                                    "QUIC min_len",
                                    "min_len",
                                )
                                .map_err(|err| err.reanchor_line_column(0, part_column))?,
                            );
                        }
                        "long_header" => {
                            let (value_column, value) = qualifier_part_opt(&parts, part_index)
                                .ok_or_else(|| {
                                    DslError::InvalidValue(
                                        "missing QUIC long_header qualifier".into(),
                                    )
                                    .at_line_column(0, Some(part_column))
                                })?;
                            part_index += 1;
                            long_header = Some(
                                parse_bool(value)
                                    .map_err(|err| err.reanchor_line_column(0, value_column))?,
                            );
                        }
                        "type" => {
                            let (value_column, value) = qualifier_part_opt(&parts, part_index)
                                .ok_or_else(|| {
                                    DslError::InvalidValue("missing QUIC type qualifier".into())
                                        .at_line_column(0, Some(part_column))
                                })?;
                            part_index += 1;
                            packet_type = Some(
                                parse_quic_packet_type(value)
                                    .map_err(|err| err.reanchor_line_column(0, value_column))?,
                            );
                        }
                        other => {
                            return Err(DslError::InvalidValue(format!(
                                "unexpected QUIC predicate suffix '{other}'"
                            ))
                            .at_line_column(0, Some(part_column)));
                        }
                    }
                }
            }
            Ok(FlowPredicate::quic_packet_observed(
                ObservationScope {
                    dir,
                    local_port,
                    remote_port,
                },
                min_len,
                long_header,
                packet_type,
            ))
        }
        other if other.starts_with("quic_frame_observed:") => {
            let suffix = &other["quic_frame_observed:".len()..];
            let parts =
                split_qualifier_parts_with_columns(suffix, "quic_frame_observed:".len() + 1);
            let mut part_index = 0usize;
            let mut dir = None;
            let mut local_port = None;
            let mut remote_port = None;
            let mut packet_type = None;
            let mut frame_type = None;
            let mut byte_matches = Vec::new();
            let mut byte_sequences = Vec::new();
            while let Some((part_column, part)) = qualifier_part_opt(&parts, part_index) {
                part_index += 1;
                if !parse_scope_qualifier(
                    part,
                    &mut QualifierPartsCursor::new(&parts, &mut part_index),
                    "quic_frame_observed",
                    "QUIC",
                    &mut dir,
                    &mut local_port,
                    &mut remote_port,
                )? {
                    match part {
                        "type" => {
                            let (value_column, value) = qualifier_part_opt(&parts, part_index)
                                .ok_or_else(|| {
                                    DslError::InvalidValue("missing QUIC type qualifier".into())
                                        .at_line_column(0, Some(part_column))
                                })?;
                            part_index += 1;
                            packet_type = Some(
                                parse_quic_packet_type(value)
                                    .map_err(|err| err.reanchor_line_column(0, value_column))?,
                            );
                        }
                        "frame" => {
                            let (value_column, value) = qualifier_part_opt(&parts, part_index)
                                .ok_or_else(|| {
                                    DslError::InvalidValue("missing QUIC frame qualifier".into())
                                        .at_line_column(0, Some(part_column))
                                })?;
                            part_index += 1;
                            frame_type = Some(
                                parse_quic_frame_type(value)
                                    .map_err(|err| err.reanchor_line_column(0, value_column))?,
                            );
                        }
                        "byte_at" => {
                            byte_matches.push(
                                parse_payload_byte_match(
                                    &mut QualifierPartsCursor::new(&parts, &mut part_index),
                                    "quic_frame_observed",
                                    "QUIC",
                                )
                                .map_err(|err| err.reanchor_line_column(0, part_column))?,
                            );
                        }
                        "bytes_at" => {
                            byte_sequences.push(
                                parse_payload_byte_sequence_match(
                                    &mut QualifierPartsCursor::new(&parts, &mut part_index),
                                    "quic_frame_observed",
                                    "QUIC",
                                )
                                .map_err(|err| err.reanchor_line_column(0, part_column))?,
                            );
                        }
                        other => {
                            return Err(DslError::InvalidValue(format!(
                                "unexpected QUIC frame predicate suffix '{other}'"
                            ))
                            .at_line_column(0, Some(part_column)));
                        }
                    }
                }
            }
            Ok(FlowPredicate::quic_frame_observed(
                ObservationScope {
                    dir,
                    local_port,
                    remote_port,
                },
                packet_type,
                frame_type.ok_or_else(|| {
                    DslError::InvalidValue(
                        "quic_frame_observed requires a frame:<type> qualifier".into(),
                    )
                    .at_line_column(0, Some("quic_frame_observed:".len() + 1))
                })?,
                byte_matches,
                byte_sequences,
            ))
        }
        other if other.starts_with("datagram_observed:") => {
            let suffix = &other["datagram_observed:".len()..];
            let parts = split_qualifier_parts_with_columns(suffix, "datagram_observed:".len() + 1);
            let mut part_index = 0usize;
            let (proto_column, proto) = qualifier_part_at(&parts, part_index);
            part_index += 1;
            let l4_proto = match proto {
                "udp" => 17,
                "tcp" => 6,
                _ => proto.parse::<u8>().map_err(|_| {
                    DslError::InvalidValue(format!("unknown datagram proto '{proto}'"))
                        .at_line_column(0, Some(proto_column))
                })?,
            };
            let mut dir = None;
            let mut local_port = None;
            let mut remote_port = None;
            let mut min_len = None;
            let mut first_byte_mask = None;
            let mut first_byte_value = None;
            let mut prefix2 = None;
            let mut prefix4 = None;
            let mut byte13_mask = None;
            let mut byte13_value = None;
            let mut byte_matches = Vec::new();
            let mut byte_sequences = Vec::new();
            while let Some((part_column, part)) = qualifier_part_opt(&parts, part_index) {
                part_index += 1;
                if !parse_scope_qualifier(
                    part,
                    &mut QualifierPartsCursor::new(&parts, &mut part_index),
                    "datagram_observed",
                    "datagram",
                    &mut dir,
                    &mut local_port,
                    &mut remote_port,
                )? {
                    match part {
                        "min_len" => {
                            min_len = Some(parse_u32_qualifier(
                                &mut QualifierPartsCursor::new(&parts, &mut part_index),
                                "datagram_observed",
                                "datagram min_len",
                                "min_len",
                            )?);
                        }
                        "byte0_mask" => {
                            let (mask, value) = parse_u8_mask_value_qualifier(
                                &mut QualifierPartsCursor::new(&parts, &mut part_index),
                                "datagram_observed",
                                "datagram byte0_mask",
                                "byte0_mask",
                            )?;
                            first_byte_mask = Some(mask);
                            first_byte_value = Some(value);
                        }
                        "prefix2" => {
                            prefix2 = Some(parse_u16_qualifier(
                                &mut QualifierPartsCursor::new(&parts, &mut part_index),
                                "datagram_observed",
                                "datagram prefix2",
                                "prefix2",
                            )?);
                        }
                        "prefix4" => {
                            prefix4 = Some(parse_u32_qualifier(
                                &mut QualifierPartsCursor::new(&parts, &mut part_index),
                                "datagram_observed",
                                "datagram prefix4",
                                "prefix4",
                            )?);
                        }
                        "byte13_mask" => {
                            let (mask, value) = parse_u8_mask_value_qualifier(
                                &mut QualifierPartsCursor::new(&parts, &mut part_index),
                                "datagram_observed",
                                "datagram byte13_mask",
                                "byte13_mask",
                            )?;
                            byte13_mask = Some(mask);
                            byte13_value = Some(value);
                        }
                        "byte_at" => {
                            byte_matches.push(
                                parse_payload_byte_match(
                                    &mut QualifierPartsCursor::new(&parts, &mut part_index),
                                    "datagram_observed",
                                    "datagram",
                                )
                                .map_err(|err| err.reanchor_line_column(0, part_column))?,
                            );
                        }
                        "bytes_at" => {
                            byte_sequences.push(
                                parse_payload_byte_sequence_match(
                                    &mut QualifierPartsCursor::new(&parts, &mut part_index),
                                    "datagram_observed",
                                    "datagram",
                                )
                                .map_err(|err| err.reanchor_line_column(0, part_column))?,
                            );
                        }
                        other => {
                            return Err(DslError::InvalidValue(format!(
                                "unknown datagram predicate suffix '{other}'"
                            ))
                            .at_line_column(0, Some(part_column)));
                        }
                    }
                }
            }
            Ok(FlowPredicate::datagram_observed(
                l4_proto,
                ObservationScope {
                    dir,
                    local_port,
                    remote_port,
                },
                min_len,
                first_byte_mask,
                first_byte_value,
                prefix2,
                prefix4,
                byte13_mask,
                byte13_value,
                byte_matches,
                byte_sequences,
            ))
        }
        other if other.starts_with("packet_observed:") => {
            let suffix = &other["packet_observed:".len()..];
            let parts = split_qualifier_parts_with_columns(suffix, "packet_observed:".len() + 1);
            let mut part_index = 0usize;
            let (proto_column, proto) = qualifier_part_at(&parts, part_index);
            part_index += 1;
            let l4_proto = match proto {
                "udp" => 17,
                "tcp" => 6,
                _ => proto.parse::<u8>().map_err(|_| {
                    DslError::InvalidValue(format!("unknown packet proto '{proto}'"))
                        .at_line_column(0, Some(proto_column))
                })?,
            };
            let mut dir = None;
            let mut local_port = None;
            let mut remote_port = None;
            let mut first_byte_mask = None;
            let mut first_byte_value = None;
            let mut prefix4 = None;
            let mut byte4_mask = None;
            let mut byte4_value = None;
            let mut byte13_mask = None;
            let mut byte13_value = None;
            let mut byte_matches = Vec::new();
            let mut byte_sequences = Vec::new();
            while let Some((part_column, part)) = qualifier_part_opt(&parts, part_index) {
                part_index += 1;
                if !parse_scope_qualifier(
                    part,
                    &mut QualifierPartsCursor::new(&parts, &mut part_index),
                    "packet_observed",
                    "packet",
                    &mut dir,
                    &mut local_port,
                    &mut remote_port,
                )? {
                    match part {
                        "byte0_mask" => {
                            let (mask, value) = parse_u8_mask_value_qualifier(
                                &mut QualifierPartsCursor::new(&parts, &mut part_index),
                                "packet_observed",
                                "packet byte0_mask",
                                "byte0_mask",
                            )?;
                            first_byte_mask = Some(mask);
                            first_byte_value = Some(value);
                        }
                        "prefix4" => {
                            prefix4 = Some(parse_u32_qualifier(
                                &mut QualifierPartsCursor::new(&parts, &mut part_index),
                                "packet_observed",
                                "packet prefix4",
                                "prefix4",
                            )?);
                        }
                        "byte4_mask" => {
                            let (mask, value) = parse_u8_mask_value_qualifier(
                                &mut QualifierPartsCursor::new(&parts, &mut part_index),
                                "packet_observed",
                                "packet byte4_mask",
                                "byte4_mask",
                            )?;
                            byte4_mask = Some(mask);
                            byte4_value = Some(value);
                        }
                        "byte13_mask" => {
                            let (mask, value) = parse_u8_mask_value_qualifier(
                                &mut QualifierPartsCursor::new(&parts, &mut part_index),
                                "packet_observed",
                                "packet byte13_mask",
                                "byte13_mask",
                            )?;
                            byte13_mask = Some(mask);
                            byte13_value = Some(value);
                        }
                        "byte_at" => {
                            byte_matches.push(
                                parse_payload_byte_match(
                                    &mut QualifierPartsCursor::new(&parts, &mut part_index),
                                    "packet_observed",
                                    "packet",
                                )
                                .map_err(|err| err.reanchor_line_column(0, part_column))?,
                            );
                        }
                        "bytes_at" => {
                            byte_sequences.push(
                                parse_payload_byte_sequence_match(
                                    &mut QualifierPartsCursor::new(&parts, &mut part_index),
                                    "packet_observed",
                                    "packet",
                                )
                                .map_err(|err| err.reanchor_line_column(0, part_column))?,
                            );
                        }
                        other => {
                            return Err(DslError::InvalidValue(format!(
                                "unexpected packet predicate suffix '{other}'"
                            ))
                            .at_line_column(0, Some(part_column)));
                        }
                    }
                }
            }
            Ok(FlowPredicate::packet_observed(
                l4_proto,
                ObservationScope {
                    dir,
                    local_port,
                    remote_port,
                },
                first_byte_mask,
                first_byte_value,
                prefix4,
                byte4_mask,
                byte4_value,
                byte13_mask,
                byte13_value,
                byte_matches,
                byte_sequences,
            ))
        }
        other => Err(DslError::InvalidValue(format!(
            "unknown predicate '{other}'"
        ))),
    }
}

pub(crate) fn parse_reason_key_event(value: &str) -> Result<Option<ReasonKeyEvent>, DslError> {
    Ok(match value {
        "none" => None,
        other => Some(SignalKind::from_id(other).ok_or_else(|| {
            DslError::InvalidValue(format!("unknown reason key event '{other}'"))
        })?),
    })
}

pub(crate) fn parse_reason_narrative(value: &str) -> ReasonNarrative {
    parse_narrative_template(value)
}

pub(crate) fn parse_narrative_template(value: &str) -> NarrativeTemplate {
    match value {
        "none" => NarrativeTemplate::None,
        "process_bound" => NarrativeTemplate::ProcessBound,
        "packet_observed" => NarrativeTemplate::PacketObserved,
        "transport_payload_sent" => NarrativeTemplate::TransportPayloadSent,
        "transport_payload_received" => NarrativeTemplate::TransportPayloadReceived,
        "tcp_state_transition" => NarrativeTemplate::TcpStateTransition,
        "route_changed" => NarrativeTemplate::RouteChanged,
        "udp_datagram_observed" => NarrativeTemplate::UdpDatagramObserved,
        "udp_datagram_sent" => NarrativeTemplate::UdpDatagramSent,
        "udp_datagram_received" => NarrativeTemplate::UdpDatagramReceived,
        other if other.starts_with("static:") => NarrativeTemplate::Static(other[7..].to_string()),
        other => NarrativeTemplate::Static(other.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::parse_narrative_template;
    use crate::ir::NarrativeTemplate;

    #[test]
    fn parsed_static_narrative_owns_dynamic_source_text() {
        let source = String::from("static:dynamic protocol narrative");
        let parsed = parse_narrative_template(&source);
        drop(source);

        assert_eq!(
            parsed,
            NarrativeTemplate::Static("dynamic protocol narrative".to_string())
        );
    }
}
