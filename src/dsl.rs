use crate::fragment::{builtin_registry, EvidenceTier, RegistryError};
use crate::flow::{ProgramOperation, ProgramStageKind};
use crate::ir::{FlowPredicate, NarrativeTemplate, SignalKind};
use crate::ledger::{FactKindTag, PacketDir};
use crate::program::{ProgramModel, ProgramNarrative, ProgramRule};
use crate::reason::{
    ReasonKeyEvent, ReasonModel, ReasonNarrative, ReasonProfile, ReasonRule,
};
use crate::template::{
    default_5s_window, default_program_model_for_reason_profile, FragmentParamValue, Template,
    TemplateBinding, WindowProfile,
};
use std::fs;

#[derive(Debug, Eq, PartialEq)]
pub enum DslError {
    Located {
        line: usize,
        inner: Box<DslError>,
    },
    InvalidLine(String),
    MissingField(&'static str),
    InvalidValue(String),
    Registry(RegistryError),
    Io(String),
}

pub fn read_file(path: &str) -> Result<String, DslError> {
    fs::read_to_string(path).map_err(|err| DslError::Io(err.to_string()))
}

pub fn parse_file_unvalidated(path: &str) -> Result<TemplateBinding, DslError> {
    let input = read_file(path)?;
    parse_str_unvalidated(&input)
}

pub fn compile_file(path: &str) -> Result<TemplateBinding, DslError> {
    let binding = parse_file_unvalidated(path)?;
    validate_compiled_binding(&binding).map_err(DslError::Registry)?;
    Ok(binding)
}

pub fn parse_str_unvalidated(input: &str) -> Result<TemplateBinding, DslError> {
    let mut template_id = None;
    let mut window_profile = None;
    let mut inline_window_duration_ms = None;
    let mut inline_window_lateness_ms = None;
    let mut reason_profile = None;
    let mut reason_model_id = None;
    let mut reason_rules = Vec::new();
    let mut fragment_set = Vec::new();
    let mut program_model_id = None;
    let mut operation = None;
    let mut rules = Vec::new();
    let mut fragment_params = Vec::new();
    let mut evidence_overrides = Vec::new();

    for (line_no, raw_line) in input.lines().enumerate() {
        let line_no = line_no + 1;
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (key, value) = line
            .split_once('=')
            .ok_or_else(|| DslError::InvalidLine(line.into()).at_line(line_no))?;
        let key = key.trim();
        let value = value.trim();

        match key {
            "template" => template_id = Some(value.to_string()),
            "window" => window_profile = Some(parse_window_profile(value).map_err(|err| err.at_line(line_no))?),
            "window.duration_ms" => {
                inline_window_duration_ms = Some(parse_u64(value, key).map_err(|err| err.at_line(line_no))?);
            }
            "window.lateness_ms" => {
                inline_window_lateness_ms = Some(parse_u64(value, key).map_err(|err| err.at_line(line_no))?);
            }
            "reason" => {
                reason_profile = Some(
                    ReasonProfile::from_id(value)
                        .ok_or_else(|| DslError::InvalidValue(format!("unknown reason profile '{value}'")).at_line(line_no))?,
                )
            }
            "reason_model" => reason_model_id = Some(value.to_string()),
            "reason.rule" => reason_rules.push(parse_reason_rule(value).map_err(|err| err.at_line(line_no))?),
            "fragment" => fragment_set.push(value.to_string()),
            "program_model" => program_model_id = Some(value.to_string()),
            "operation" => operation = Some(parse_operation(value)),
            "rule" => rules.push(parse_rule(value).map_err(|err| err.at_line(line_no))?),
            "param" => fragment_params.push(parse_param_entry(value).map_err(|err| err.at_line(line_no))?),
            "evidence" => {
                evidence_overrides.push(parse_evidence_override(value).map_err(|err| err.at_line(line_no))?)
            }
            other => return Err(DslError::InvalidValue(format!("unknown DSL key '{other}'")).at_line(line_no)),
        }
    }

    let template_id = template_id.ok_or(DslError::MissingField("template"))?;
    let window_profile = build_window_profile(
        window_profile,
        inline_window_duration_ms,
        inline_window_lateness_ms,
    )?;
    let reason_profile = build_reason_profile(
        &template_id,
        reason_profile,
        reason_model_id,
        reason_rules,
    )?;
    let program_model = build_program_model(
        &template_id,
        &reason_profile,
        program_model_id,
        operation,
        rules,
    )?;

    let template = Template {
        id: Box::leak(template_id.into_boxed_str()),
        fragment_set: fragment_set
            .into_iter()
            .map(|item| Box::leak(item.into_boxed_str()) as &'static str)
            .collect(),
        window_profile: Some(window_profile),
        reason_profile: Some(reason_profile),
        program_model: Some(program_model),
    };

    let mut binding = template.bind();
    for (fragment_id, key, value) in fragment_params {
        binding = binding.with_fragment_param(fragment_id, key, value);
    }
    for (fact_kind, tier) in evidence_overrides {
        binding = binding.with_evidence_tier(fact_kind, tier);
    }
    Ok(binding)
}

pub fn compile_str(input: &str) -> Result<TemplateBinding, DslError> {
    let binding = parse_str_unvalidated(input)?;
    validate_compiled_binding(&binding).map_err(DslError::Registry)?;
    Ok(binding)
}

pub fn validate_compiled_binding(binding: &TemplateBinding) -> Result<(), RegistryError> {
    builtin_registry().validate_binding(binding)
}

impl DslError {
    pub fn at_line(self, line: usize) -> Self {
        match self {
            Self::Located { .. } => self,
            other => Self::Located {
                line,
                inner: Box::new(other),
            },
        }
    }

    pub fn line(&self) -> Option<usize> {
        match self {
            Self::Located { line, .. } => Some(*line),
            _ => None,
        }
    }

    pub fn root(&self) -> &DslError {
        match self {
            Self::Located { inner, .. } => inner.root(),
            other => other,
        }
    }
}

fn parse_window_profile(value: &str) -> Result<WindowProfile, DslError> {
    match value {
        "default_5s" => Ok(default_5s_window()),
        other => Err(DslError::InvalidValue(format!("unknown window profile '{other}'"))),
    }
}

fn build_window_profile(
    profile: Option<WindowProfile>,
    duration_ms: Option<u64>,
    lateness_ms: Option<u64>,
) -> Result<WindowProfile, DslError> {
    if let Some(profile) = profile {
        return Ok(profile);
    }
    match (duration_ms, lateness_ms) {
        (Some(duration_ms), Some(lateness_ms)) => Ok(WindowProfile {
            id: "inline",
            duration_ms,
            lateness_ms,
        }),
        (None, None) => Err(DslError::MissingField("window")),
        _ => Err(DslError::MissingField("window")),
    }
}

fn build_program_model(
    template_id: &str,
    reason_profile: &ReasonProfile,
    program_model_id: Option<String>,
    operation: Option<ProgramOperation>,
    rules: Vec<ProgramRule>,
) -> Result<ProgramModel, DslError> {
    match (program_model_id, operation, rules.is_empty()) {
        (None, None, true) => Ok(default_program_model_for_reason_profile(reason_profile)),
        (program_model_id, operation, _) => {
            let operation = operation.ok_or(DslError::MissingField("operation"))?;
            let id = program_model_id.unwrap_or_else(|| format!("{template_id}_dsl_model"));
            Ok(ProgramModel {
                id: Box::leak(id.into_boxed_str()),
                operation,
                rules,
            })
        }
    }
}

fn build_reason_profile(
    template_id: &str,
    profile: Option<ReasonProfile>,
    reason_model_id: Option<String>,
    reason_rules: Vec<ReasonRule>,
) -> Result<ReasonProfile, DslError> {
    if reason_rules.is_empty() {
        return profile.ok_or(DslError::MissingField("reason"));
    }

    let id = reason_model_id.unwrap_or_else(|| format!("{template_id}_reason_model"));
    Ok(ReasonProfile::Declarative(ReasonModel {
        id: Box::leak(id.into_boxed_str()),
        rules: reason_rules,
    }))
}

fn parse_operation(value: &str) -> ProgramOperation {
    match value {
        "connect_flow" => ProgramOperation::ConnectFlow,
        "datagram_exchange" => ProgramOperation::DatagramExchange,
        "unknown" => ProgramOperation::Unknown,
        other => ProgramOperation::Custom(other.into()),
    }
}

fn parse_rule(value: &str) -> Result<ProgramRule, DslError> {
    let parts = split_top_level(value, ';');
    if !(4..=6).contains(&parts.len()) {
        return Err(DslError::InvalidValue(format!("invalid rule '{value}'")));
    }

    Ok(ProgramRule {
        predicate: parse_flow_predicate(parts[0].trim())?,
        signal: parse_stage(parts[1].trim())?,
        narrative: parse_narrative(parts[2].trim()),
        dedupe: parse_bool(parts[3].trim())?,
        module: parts.get(4).map(|value| value.trim().to_string()),
        phase: parts.get(5).map(|value| value.trim().to_string()),
    })
}

fn parse_reason_rule(value: &str) -> Result<ReasonRule, DslError> {
    let parts = split_top_level(value, ';');
    if !(4..=6).contains(&parts.len()) {
        return Err(DslError::InvalidValue(format!("invalid reason rule '{value}'")));
    }

    Ok(ReasonRule {
        predicate: parse_flow_predicate(parts[0].trim())?,
        signal: parse_reason_key_event(parts[1].trim())?,
        narrative: parse_reason_narrative(parts[2].trim()),
        dedupe: parse_bool(parts[3].trim())?,
        module: parts.get(4).map(|value| value.trim().to_string()),
        phase: parts.get(5).map(|value| value.trim().to_string()),
    })
}

fn parse_stage(value: &str) -> Result<Option<ProgramStageKind>, DslError> {
    Ok(match value {
        "none" => None,
        other => Some(
            SignalKind::from_id(other)
                .ok_or_else(|| DslError::InvalidValue(format!("unknown stage '{other}'")))?,
        ),
    })
}

fn parse_narrative(value: &str) -> ProgramNarrative {
    parse_narrative_template(value)
}

fn parse_flow_predicate(value: &str) -> Result<FlowPredicate, DslError> {
    if let Some(inner) = value
        .strip_prefix("all(")
        .and_then(|rest| rest.strip_suffix(')'))
    {
        return Ok(FlowPredicate::All(
            split_top_level(inner, ',')
                .into_iter()
                .map(|part| parse_flow_predicate(part.trim()))
                .collect::<Result<Vec<_>, _>>()?,
        ));
    }
    if let Some(inner) = value
        .strip_prefix("any(")
        .and_then(|rest| rest.strip_suffix(')'))
    {
        return Ok(FlowPredicate::Any(
            split_top_level(inner, ',')
                .into_iter()
                .map(|part| parse_flow_predicate(part.trim()))
                .collect::<Result<Vec<_>, _>>()?,
        ));
    }

    match value {
        "process_bound" => Ok(FlowPredicate::ProcessBound),
        "socket_state_observed" => Ok(FlowPredicate::SocketStateObserved {
            local_port: None,
            remote_port: None,
            min_new_state: None,
        }),
        other if other.starts_with("socket_state_observed:") => {
            let suffix = &other["socket_state_observed:".len()..];
            let mut parts = suffix.split(':');
            let first = parts.next().unwrap_or_default();
            let (local_port, remote_port, port) = match first {
                "local" | "sport" => (true, false, parts.next().unwrap_or_default()),
                "remote" | "dport" => (false, true, parts.next().unwrap_or_default()),
                _ => (false, true, first),
            };
            let port = match port {
                "https" => 443,
                "http" => 80,
                "postgres" => 5432,
                "mysql" => 3306,
                "redis" => 6379,
                other => other.parse::<u16>().map_err(|_| {
                    DslError::InvalidValue(format!(
                        "unknown socket_state_observed port '{other}'"
                    ))
                })?,
            };
            let min_new_state = match parts.next() {
                None => None,
                Some("established") => Some(3),
                Some(other) => {
                    return Err(DslError::InvalidValue(format!(
                        "unknown socket_state_observed state qualifier '{other}'"
                    )))
                }
            };
            if let Some(extra) = parts.next() {
                return Err(DslError::InvalidValue(format!(
                    "unexpected socket_state_observed suffix '{extra}'"
                )));
            }
            Ok(FlowPredicate::SocketStateObserved {
                local_port: local_port.then_some(port),
                remote_port: remote_port.then_some(port),
                min_new_state,
            })
        }
        "route_resolved" => Ok(FlowPredicate::RouteResolved),
        other if other.starts_with("datagram_observed:") => {
            let suffix = &other["datagram_observed:".len()..];
            let mut parts = suffix.split(':');
            let proto = parts.next().unwrap_or_default();
            let l4_proto = match proto {
                "udp" => 17,
                "tcp" => 6,
                _ => proto
                    .parse::<u8>()
                    .map_err(|_| DslError::InvalidValue(format!("unknown datagram proto '{proto}'")))?,
            };
            let mut dir = None;
            let mut local_port = None;
            let mut remote_port = None;
            let mut min_len = None;
            let mut first_byte_mask = None;
            let mut first_byte_value = None;
            let mut prefix2 = None;
            let mut prefix4 = None;
            while let Some(part) = parts.next() {
                match part {
                    "egress" | "local_to_remote" => dir = Some(PacketDir::Egress),
                    "ingress" | "remote_to_local" => dir = Some(PacketDir::Ingress),
                    "local" | "sport" => {
                        let port = parts.next().ok_or_else(|| {
                            DslError::InvalidValue("missing datagram local port qualifier".into())
                        })?;
                        local_port = Some(parse_named_port(port, "datagram_observed")?);
                    }
                    "remote" | "dport" => {
                        let port = parts.next().ok_or_else(|| {
                            DslError::InvalidValue("missing datagram remote port qualifier".into())
                        })?;
                        remote_port = Some(parse_named_port(port, "datagram_observed")?);
                    }
                    "min_len" => {
                        let value = parts.next().ok_or_else(|| {
                            DslError::InvalidValue(
                                "missing datagram min_len qualifier".into(),
                            )
                        })?;
                        min_len = Some(value.parse::<u32>().map_err(|_| {
                            DslError::InvalidValue(format!(
                                "invalid datagram min_len '{value}'"
                            ))
                        })?);
                    }
                    "byte0_mask" => {
                        let mask = parts.next().ok_or_else(|| {
                            DslError::InvalidValue(
                                "missing datagram byte0_mask mask qualifier".into(),
                            )
                        })?;
                        let value = parts.next().ok_or_else(|| {
                            DslError::InvalidValue(
                                "missing datagram byte0_mask value qualifier".into(),
                            )
                        })?;
                        first_byte_mask =
                            Some(parse_u8_literal(mask, "datagram_observed", "byte0_mask")?);
                        first_byte_value =
                            Some(parse_u8_literal(value, "datagram_observed", "byte0_value")?);
                    }
                    "prefix2" => {
                        let value = parts.next().ok_or_else(|| {
                            DslError::InvalidValue("missing datagram prefix2 qualifier".into())
                        })?;
                        prefix2 = Some(parse_u16_literal(
                            value,
                            "datagram_observed",
                            "prefix2",
                        )?);
                    }
                    "prefix4" => {
                        let value = parts.next().ok_or_else(|| {
                            DslError::InvalidValue("missing datagram prefix4 qualifier".into())
                        })?;
                        prefix4 = Some(parse_u32_literal(
                            value,
                            "datagram_observed",
                            "prefix4",
                        )?);
                    }
                    other => {
                        return Err(DslError::InvalidValue(format!(
                            "unknown datagram predicate suffix '{other}'"
                        )))
                    }
                }
            }
            Ok(FlowPredicate::DatagramObserved {
                l4_proto,
                dir,
                local_port,
                remote_port,
                min_len,
                first_byte_mask,
                first_byte_value,
                prefix2,
                prefix4,
            })
        }
        other if other.starts_with("packet_observed:") => {
            let suffix = &other["packet_observed:".len()..];
            let mut parts = suffix.split(':');
            let proto = parts.next().unwrap_or_default();
            let l4_proto = match proto {
                "udp" => 17,
                "tcp" => 6,
                _ => proto
                    .parse::<u8>()
                    .map_err(|_| DslError::InvalidValue(format!("unknown packet proto '{proto}'")))?,
            };
            let mut dir = None;
            let mut local_port = None;
            let mut remote_port = None;
            let mut first_byte_mask = None;
            let mut first_byte_value = None;
            let mut prefix4 = None;
            let mut byte4_mask = None;
            let mut byte4_value = None;
            while let Some(part) = parts.next() {
                match part {
                    "egress" | "local_to_remote" => dir = Some(PacketDir::Egress),
                    "ingress" | "remote_to_local" => dir = Some(PacketDir::Ingress),
                    "local" | "sport" => {
                        let port = parts.next().ok_or_else(|| {
                            DslError::InvalidValue("missing packet local port qualifier".into())
                        })?;
                        local_port = Some(parse_named_port(port, "packet_observed")?);
                    }
                    "remote" | "dport" => {
                        let port = parts.next().ok_or_else(|| {
                            DslError::InvalidValue("missing packet remote port qualifier".into())
                        })?;
                        remote_port = Some(parse_named_port(port, "packet_observed")?);
                    }
                    "byte0_mask" => {
                        let mask = parts.next().ok_or_else(|| {
                            DslError::InvalidValue(
                                "missing packet byte0_mask mask qualifier".into(),
                            )
                        })?;
                        let value = parts.next().ok_or_else(|| {
                            DslError::InvalidValue(
                                "missing packet byte0_mask value qualifier".into(),
                            )
                        })?;
                        first_byte_mask =
                            Some(parse_u8_literal(mask, "packet_observed", "byte0_mask")?);
                        first_byte_value =
                            Some(parse_u8_literal(value, "packet_observed", "byte0_value")?);
                    }
                    "prefix4" => {
                        let value = parts.next().ok_or_else(|| {
                            DslError::InvalidValue("missing packet prefix4 qualifier".into())
                        })?;
                        prefix4 = Some(parse_u32_literal(value, "packet_observed", "prefix4")?);
                    }
                    "byte4_mask" => {
                        let mask = parts.next().ok_or_else(|| {
                            DslError::InvalidValue(
                                "missing packet byte4_mask mask qualifier".into(),
                            )
                        })?;
                        let value = parts.next().ok_or_else(|| {
                            DslError::InvalidValue(
                                "missing packet byte4_mask value qualifier".into(),
                            )
                        })?;
                        byte4_mask =
                            Some(parse_u8_literal(mask, "packet_observed", "byte4_mask")?);
                        byte4_value =
                            Some(parse_u8_literal(value, "packet_observed", "byte4_value")?);
                    }
                    other => {
                        return Err(DslError::InvalidValue(format!(
                            "unexpected packet predicate suffix '{other}'"
                        )))
                    }
                }
            }
            Ok(FlowPredicate::PacketObserved {
                l4_proto,
                dir,
                local_port,
                remote_port,
                first_byte_mask,
                first_byte_value,
                prefix4,
                byte4_mask,
                byte4_value,
            })
        }
        other => Err(DslError::InvalidValue(format!("unknown predicate '{other}'"))),
    }
}

fn parse_reason_key_event(value: &str) -> Result<Option<ReasonKeyEvent>, DslError> {
    Ok(match value {
        "none" => None,
        other => Some(
            SignalKind::from_id(other).ok_or_else(|| {
                DslError::InvalidValue(format!("unknown reason key event '{other}'"))
            })?,
        ),
    })
}

fn parse_reason_narrative(value: &str) -> ReasonNarrative {
    parse_narrative_template(value)
}

fn parse_named_port(value: &str, predicate: &str) -> Result<u16, DslError> {
    match value {
        "quic" | "https" => Ok(443),
        "http" => Ok(80),
        "dhcp_client" | "bootpc" => Ok(68),
        "dhcp_server" | "bootps" | "dhcp" => Ok(67),
        "mdns" => Ok(5353),
        "ssdp" => Ok(1900),
        "wireguard" => Ok(51820),
        "coap" => Ok(5683),
        "ntp" => Ok(123),
        "stun" => Ok(3478),
        "postgres" => Ok(5432),
        "mysql" => Ok(3306),
        "redis" => Ok(6379),
        "mqtt" => Ok(1883),
        "radius" => Ok(1812),
        other => other
            .parse::<u16>()
            .map_err(|_| DslError::InvalidValue(format!("unknown {predicate} port '{other}'"))),
    }
}

fn parse_u8_literal(value: &str, predicate: &str, field: &str) -> Result<u8, DslError> {
    let parsed = if let Some(hex) = value.strip_prefix("0x") {
        u8::from_str_radix(hex, 16)
    } else {
        value.parse::<u8>()
    };
    parsed.map_err(|_| {
        DslError::InvalidValue(format!("invalid {predicate} {field} '{value}'"))
    })
}

fn parse_u16_literal(value: &str, predicate: &str, field: &str) -> Result<u16, DslError> {
    let parsed = if let Some(hex) = value.strip_prefix("0x") {
        u16::from_str_radix(hex, 16)
    } else {
        value.parse::<u16>()
    };
    parsed.map_err(|_| {
        DslError::InvalidValue(format!("invalid {predicate} {field} '{value}'"))
    })
}

fn parse_u32_literal(value: &str, predicate: &str, field: &str) -> Result<u32, DslError> {
    let parsed = if let Some(hex) = value.strip_prefix("0x") {
        u32::from_str_radix(hex, 16)
    } else {
        value.parse::<u32>()
    };
    parsed.map_err(|_| {
        DslError::InvalidValue(format!("invalid {predicate} {field} '{value}'"))
    })
}

fn parse_narrative_template(value: &str) -> NarrativeTemplate {
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
        other if other.starts_with("static:") => {
            NarrativeTemplate::Static(Box::leak(other[7..].to_string().into_boxed_str()))
        }
        other => NarrativeTemplate::Static(Box::leak(other.to_string().into_boxed_str())),
    }
}

fn parse_param_entry(
    value: &str,
) -> Result<(&'static str, &'static str, FragmentParamValue), DslError> {
    let (lhs, rhs) = value
        .split_once('=')
        .ok_or_else(|| DslError::InvalidValue(format!("invalid param '{value}'")))?;
    let (fragment_id, key) = lhs
        .split_once('.')
        .ok_or_else(|| DslError::InvalidValue(format!("invalid param target '{lhs}'")))?;

    Ok((
        Box::leak(fragment_id.trim().to_string().into_boxed_str()),
        Box::leak(key.trim().to_string().into_boxed_str()),
        parse_param_value(rhs.trim())?,
    ))
}

fn parse_evidence_override(value: &str) -> Result<(FactKindTag, EvidenceTier), DslError> {
    let (fact_kind, tier) = value
        .split_once(':')
        .ok_or_else(|| DslError::InvalidValue(format!("invalid evidence override '{value}'")))?;
    let fact_kind = FactKindTag::from_str(fact_kind.trim()).ok_or_else(|| {
        DslError::InvalidValue(format!("unknown evidence fact kind '{}'", fact_kind.trim()))
    })?;
    let tier = match tier.trim() {
        "core_requirement" => EvidenceTier::CoreRequirement,
        "optional_enhancement" => EvidenceTier::OptionalEnhancement,
        other => {
            return Err(DslError::InvalidValue(format!(
                "unknown evidence tier '{other}'"
            )))
        }
    };
    Ok((fact_kind, tier))
}

fn parse_param_value(value: &str) -> Result<FragmentParamValue, DslError> {
    if matches!(value, "true" | "false") {
        return Ok(FragmentParamValue::Bool(parse_bool(value)?));
    }
    if let Ok(value) = value.parse::<u64>() {
        return Ok(FragmentParamValue::U64(value));
    }
    Ok(FragmentParamValue::String(value.trim_matches('"').to_string()))
}

fn parse_bool(value: &str) -> Result<bool, DslError> {
    match value {
        "true" => Ok(true),
        "false" => Ok(false),
        other => Err(DslError::InvalidValue(format!("invalid bool '{other}'"))),
    }
}

fn parse_u64(value: &str, key: &str) -> Result<u64, DslError> {
    value
        .parse::<u64>()
        .map_err(|_| DslError::InvalidValue(format!("invalid u64 for '{key}': '{value}'")))
}

fn split_top_level(input: &str, delimiter: char) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0usize;
    let mut start = 0usize;

    for (idx, ch) in input.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            _ if ch == delimiter && depth == 0 => {
                parts.push(input[start..idx].trim());
                start = idx + ch.len_utf8();
            }
            _ => {}
        }
    }

    parts.push(input[start..].trim());
    parts
}
