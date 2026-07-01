use super::super::super::ExportError;
use super::super::super::fact_codec::parse_fact_ids;
use super::super::super::json::JsonValue;
use crate::flow::FlowId;
use crate::ledger::FactId;
use crate::reason::{KeyEvent, KeyEventKind, NarrLine, ReasonChain, ReasonId, ReasonL1, ReasonL3};

pub(crate) fn parse_reason(value: &JsonValue) -> Result<ReasonChain, ExportError> {
    let object = value.as_object()?;
    let l1 = object
        .get("l1")
        .ok_or_else(|| ExportError::InvalidShape("reason.l1".into()))?
        .as_object()?;
    let l3 = object
        .get("l3")
        .ok_or_else(|| ExportError::InvalidShape("reason.l3".into()))?
        .as_object()?;

    Ok(ReasonChain {
        id: ReasonId(
            object
                .get("id")
                .ok_or_else(|| ExportError::InvalidShape("reason.id".into()))?
                .as_i64()? as u64,
        ),
        flow: FlowId(
            object
                .get("flow")
                .ok_or_else(|| ExportError::InvalidShape("reason.flow".into()))?
                .as_i64()? as u64,
        ),
        l0_facts: parse_fact_ids(
            object
                .get("l0_facts")
                .ok_or_else(|| ExportError::InvalidShape("reason.l0_facts".into()))?,
        )?,
        l1: ReasonL1 {
            tcp_state_timeline: parse_fact_ids(l1.get("tcp_state_timeline").ok_or_else(|| {
                ExportError::InvalidShape("reason.l1.tcp_state_timeline".into())
            })?)?,
            path_segments: parse_fact_ids(
                l1.get("path_segments")
                    .ok_or_else(|| ExportError::InvalidShape("reason.l1.path_segments".into()))?,
            )?,
            key_events: l1
                .get("key_events")
                .ok_or_else(|| ExportError::InvalidShape("reason.l1.key_events".into()))?
                .as_array()?
                .iter()
                .map(parse_key_event)
                .collect::<Result<Vec<_>, _>>()?,
        },
        l3: ReasonL3 {
            narrative: l3
                .get("narrative")
                .ok_or_else(|| ExportError::InvalidShape("reason.l3.narrative".into()))?
                .as_array()?
                .iter()
                .map(parse_narrative_line)
                .collect::<Result<Vec<_>, _>>()?,
        },
    })
}

fn parse_narrative_line(value: &JsonValue) -> Result<NarrLine, ExportError> {
    let object = value.as_object()?;
    Ok(NarrLine {
        at: FactId(
            object
                .get("at")
                .ok_or_else(|| ExportError::InvalidShape("reason.l3.narrative.at".into()))?
                .as_i64()? as u64,
        ),
        text: object
            .get("text")
            .ok_or_else(|| ExportError::InvalidShape("reason.l3.narrative.text".into()))?
            .as_str()?
            .to_string(),
    })
}

fn parse_key_event(value: &JsonValue) -> Result<KeyEvent, ExportError> {
    let object = value.as_object()?;
    let kind = object
        .get("kind")
        .ok_or_else(|| ExportError::InvalidShape("reason.key_event.kind".into()))?
        .as_str()?;

    Ok(KeyEvent {
        at: FactId(
            object
                .get("at")
                .ok_or_else(|| ExportError::InvalidShape("reason.key_event.at".into()))?
                .as_i64()? as u64,
        ),
        kind: match kind {
            "syn_seen" => KeyEventKind::SynSeen,
            "packet_observed" => KeyEventKind::PacketObserved,
            "udp_datagram_seen" => KeyEventKind::UdpDatagramSeen,
            "process_identified" => KeyEventKind::ProcessIdentified,
            "retrans_suspected" => KeyEventKind::RetransSuspected,
            "route_changed" => KeyEventKind::RouteChanged,
            "fin_or_rst" => KeyEventKind::FinOrRst,
            "state_change" => KeyEventKind::StateChange {
                old: object
                    .get("old")
                    .ok_or_else(|| ExportError::InvalidShape("reason.key_event.old".into()))?
                    .as_i64()? as u8,
                new: object
                    .get("new")
                    .ok_or_else(|| ExportError::InvalidShape("reason.key_event.new".into()))?
                    .as_i64()? as u8,
            },
            other => {
                return Err(ExportError::InvalidValue(format!(
                    "unknown key event kind '{other}'"
                )));
            }
        },
    })
}
