use crate::flow::{
    EvidenceIndex, FlowId, FlowLifecycleView, FlowSnapshot, PathSegment, PathView, ProgramFlow,
    ProgramFlowId, ProgramOperation, ProgramStage,
};
use crate::fragment::{
    AttachPlan, AttachReport, CapabilityFlag, CoverageReport, DependencyEdge, FactBinding,
    BindingDiagnostics, EvidenceClassSpec, EvidenceTier, FragmentDescriptor, FragmentParamSpec,
    FragmentParamType, HookBinding, HookPoint, MapKind, MapSpec, ModelDiagnostics, RingBufStats,
    RuleDiagnostics, RuleTier,
};
use crate::ir::{NarrativeTemplate, SignalKind};
use crate::ledger::{
    millis_to_system_time, system_time_to_millis, AttachScopeFact, CpuId, DropActionFact,
    DropVerdict, FactEnvelope, FactId, FactKind, FactKindTag, PacketDir, PacketMetaFact,
    RouteDecisionFact, SessionId, SockLineageFact, TcpStateFact,
};
use crate::reason::{
    KeyEvent, KeyEventKind, NarrLine, ReasonChain, ReasonId, ReasonKeyEvent, ReasonL1, ReasonL3,
    ReasonModel, ReasonNarrative, ReasonPredicate, ReasonProfile, ReasonRule,
};
use crate::runtime::{
    summarize_rejected_facts, RejectedFact, RejectedFactReason, RuntimeError, RuntimeSession,
    SessionConfig,
};
use crate::template::{default_program_model_for_reason_profile, FragmentParamValue, Template, WindowProfile};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq)]
pub struct FragmentInventoryItem {
    pub id: String,
    pub version: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ExportBundle {
    pub template_id: String,
    pub fragment_inventory: Vec<FragmentInventoryItem>,
    pub attach_plan: AttachPlan,
    pub attach_report: AttachReport,
    pub binding_diagnostics: BindingDiagnostics,
    pub attach_failure_summary: Vec<AttachFailureSummaryItem>,
    pub debug_summary: DebugSummary,
    pub window_profile: WindowProfile,
    pub reason_profile_id: String,
    pub reason_profile: ReasonProfile,
    pub fragment_params: BTreeMap<String, BTreeMap<String, FragmentParamValue>>,
    pub evidence_overrides: BTreeMap<FactKindTag, EvidenceTier>,
    pub facts: Vec<FactEnvelope>,
    pub rejected_facts: Vec<RejectedFact>,
    pub rejected_fact_summary: Vec<RejectedFactSummaryItem>,
    pub flows: Vec<FlowSnapshot>,
    pub program_flows: Vec<ProgramFlow>,
    pub reasons: Vec<ReasonChain>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachFailureSummaryItem {
    pub hookpoint_kind: String,
    pub count: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DebugSummary {
    pub fragments_loaded: u64,
    pub hookpoints_failed: u64,
    pub accepted_facts: u64,
    pub rejected_facts: u64,
    pub flows: u64,
    pub program_flows: u64,
    pub reasons: u64,
    pub degraded: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RejectedFactSummaryItem {
    pub fragment_id: String,
    pub reason: String,
    pub count: u64,
}

#[derive(Debug, PartialEq)]
pub enum ExportError {
    InvalidJson(String),
    InvalidShape(String),
    InvalidValue(String),
    Runtime(RuntimeError),
}

impl ExportBundle {
    pub fn replay(&self) -> Result<Self, ExportError> {
        let reason_profile = self.reason_profile.clone();
        let template = Template {
            id: Box::leak(self.template_id.clone().into_boxed_str()),
            fragment_set: self
                .fragment_inventory
                .iter()
                .map(|item| Box::leak(item.id.clone().into_boxed_str()) as &'static str)
                .collect(),
            window_profile: Some(self.window_profile.clone()),
            reason_profile: Some(reason_profile.clone()),
            program_model: Some(default_program_model_for_reason_profile(&reason_profile)),
        };

        let config = SessionConfig::for_binding(crate::template::TemplateBinding {
            template,
            fragment_params: self.fragment_params.clone(),
            evidence_overrides: self.evidence_overrides.clone(),
        })
        .map_err(ExportError::Runtime)?;
        let mut session = RuntimeSession::start(config).map_err(ExportError::Runtime)?;
        for fact in &self.facts {
            session.ingest(fact.clone());
        }
        session.seed_rejected_facts(self.rejected_facts.clone());
        let mut replay = session.export_bundle();
        replay.binding_diagnostics = self.binding_diagnostics.clone();
        replay.attach_failure_summary = self.attach_failure_summary.clone();
        replay.debug_summary = self.debug_summary.clone();
        replay.rejected_fact_summary = summarize_rejected_facts(&replay.rejected_facts);
        replay.program_flows = self.program_flows.clone();
        Ok(replay)
    }

    pub fn to_json(&self) -> String {
        let root = JsonValue::Object(BTreeMap::from([
            ("template_id".into(), JsonValue::String(self.template_id.clone())),
            (
                "fragment_inventory".into(),
                JsonValue::Array(
                    self.fragment_inventory
                        .iter()
                        .map(|item| {
                            JsonValue::Object(BTreeMap::from([
                                ("id".into(), JsonValue::String(item.id.clone())),
                                ("version".into(), JsonValue::Number(item.version as i64)),
                            ]))
                        })
                        .collect(),
                ),
            ),
            ("attach_plan".into(), attach_plan_json(&self.attach_plan)),
            ("attach_report".into(), attach_report_json(&self.attach_report)),
            (
                "binding_diagnostics".into(),
                binding_diagnostics_json(&self.binding_diagnostics),
            ),
            (
                "attach_failure_summary".into(),
                JsonValue::Array(
                    self.attach_failure_summary
                        .iter()
                        .map(attach_failure_summary_json)
                        .collect(),
                ),
            ),
            ("debug_summary".into(), debug_summary_json(&self.debug_summary)),
            (
                "window_profile".into(),
                JsonValue::Object(BTreeMap::from([
                    ("id".into(), JsonValue::String(self.window_profile.id.into())),
                    (
                        "duration_ms".into(),
                        JsonValue::Number(self.window_profile.duration_ms as i64),
                    ),
                    (
                        "lateness_ms".into(),
                        JsonValue::Number(self.window_profile.lateness_ms as i64),
                    ),
                ])),
            ),
            (
                "reason_profile_id".into(),
                JsonValue::String(self.reason_profile_id.clone()),
            ),
            (
                "reason_profile".into(),
                reason_profile_json(&self.reason_profile),
            ),
            (
                "fragment_params".into(),
                fragment_params_json(&self.fragment_params),
            ),
            (
                "evidence_overrides".into(),
                evidence_overrides_json(&self.evidence_overrides),
            ),
            (
                "facts".into(),
                JsonValue::Array(self.facts.iter().map(fact_json).collect()),
            ),
            (
                "rejected_facts".into(),
                JsonValue::Array(self.rejected_facts.iter().map(rejected_fact_json).collect()),
            ),
            (
                "rejected_fact_summary".into(),
                JsonValue::Array(
                    self.rejected_fact_summary
                        .iter()
                        .map(rejected_fact_summary_json)
                        .collect(),
                ),
            ),
            (
                "flows".into(),
                JsonValue::Array(self.flows.iter().map(flow_json).collect()),
            ),
            (
                "program_flows".into(),
                JsonValue::Array(self.program_flows.iter().map(program_flow_json).collect()),
            ),
            (
                "reasons".into(),
                JsonValue::Array(self.reasons.iter().map(reason_json).collect()),
            ),
        ]));
        root.render()
    }

    pub fn from_json(input: &str) -> Result<Self, ExportError> {
        let value = JsonParser::new(input).parse()?;
        let root = value.into_object()?;

        Ok(Self {
            template_id: root
                .get("template_id")
                .ok_or_else(|| ExportError::InvalidShape("missing template_id".into()))?
                .as_str()?
                .to_string(),
            fragment_inventory: root
                .get("fragment_inventory")
                .ok_or_else(|| ExportError::InvalidShape("missing fragment_inventory".into()))?
                .as_array()?
                .iter()
                .map(parse_fragment_inventory)
                .collect::<Result<Vec<_>, _>>()?,
            attach_plan: parse_attach_plan(
                root.get("attach_plan")
                    .ok_or_else(|| ExportError::InvalidShape("missing attach_plan".into()))?,
            )?,
            attach_report: parse_attach_report(
                root.get("attach_report")
                    .ok_or_else(|| ExportError::InvalidShape("missing attach_report".into()))?,
            )?,
            binding_diagnostics: parse_binding_diagnostics(
                root.get("binding_diagnostics")
                    .ok_or_else(|| ExportError::InvalidShape("missing binding_diagnostics".into()))?,
            )?,
            attach_failure_summary: root
                .get("attach_failure_summary")
                .ok_or_else(|| ExportError::InvalidShape("missing attach_failure_summary".into()))?
                .as_array()?
                .iter()
                .map(parse_attach_failure_summary)
                .collect::<Result<Vec<_>, _>>()?,
            debug_summary: parse_debug_summary(
                root.get("debug_summary")
                    .ok_or_else(|| ExportError::InvalidShape("missing debug_summary".into()))?,
            )?,
            window_profile: parse_window_profile(
                root.get("window_profile")
                    .ok_or_else(|| ExportError::InvalidShape("missing window_profile".into()))?,
            )?,
            reason_profile_id: root
                .get("reason_profile_id")
                .ok_or_else(|| ExportError::InvalidShape("missing reason_profile_id".into()))?
                .as_str()?
                .to_string(),
            reason_profile: if let Some(value) = root.get("reason_profile") {
                parse_reason_profile(value)?
            } else {
                let id = root
                    .get("reason_profile_id")
                    .ok_or_else(|| ExportError::InvalidShape("missing reason_profile_id".into()))?
                    .as_str()?;
                ReasonProfile::from_id(id)
                    .ok_or_else(|| ExportError::InvalidValue("unknown reason profile".into()))?
            },
            fragment_params: parse_fragment_params(
                root.get("fragment_params")
                    .unwrap_or(&JsonValue::Object(BTreeMap::new())),
            )?,
            evidence_overrides: parse_evidence_overrides(
                root.get("evidence_overrides")
                    .unwrap_or(&JsonValue::Object(BTreeMap::new())),
            )?,
            facts: root
                .get("facts")
                .ok_or_else(|| ExportError::InvalidShape("missing facts".into()))?
                .as_array()?
                .iter()
                .map(parse_fact)
                .collect::<Result<Vec<_>, _>>()?,
            rejected_facts: root
                .get("rejected_facts")
                .ok_or_else(|| ExportError::InvalidShape("missing rejected_facts".into()))?
                .as_array()?
                .iter()
                .map(parse_rejected_fact)
                .collect::<Result<Vec<_>, _>>()?,
            rejected_fact_summary: root
                .get("rejected_fact_summary")
                .ok_or_else(|| ExportError::InvalidShape("missing rejected_fact_summary".into()))?
                .as_array()?
                .iter()
                .map(parse_rejected_fact_summary)
                .collect::<Result<Vec<_>, _>>()?,
            flows: root
                .get("flows")
                .ok_or_else(|| ExportError::InvalidShape("missing flows".into()))?
                .as_array()?
                .iter()
                .map(parse_flow)
                .collect::<Result<Vec<_>, _>>()?,
            program_flows: root
                .get("program_flows")
                .ok_or_else(|| ExportError::InvalidShape("missing program_flows".into()))?
                .as_array()?
                .iter()
                .map(parse_program_flow)
                .collect::<Result<Vec<_>, _>>()?,
            reasons: root
                .get("reasons")
                .ok_or_else(|| ExportError::InvalidShape("missing reasons".into()))?
                .as_array()?
                .iter()
                .map(parse_reason)
                .collect::<Result<Vec<_>, _>>()?,
        })
    }
}

pub fn fact_to_json(fact: &FactEnvelope) -> String {
    fact_json(fact).render()
}

pub fn fact_from_json(input: &str) -> Result<FactEnvelope, ExportError> {
    let value = JsonParser::new(input).parse()?;
    parse_fact(&value)
}

fn fragment_params_json(
    fragment_params: &BTreeMap<String, BTreeMap<String, FragmentParamValue>>,
) -> JsonValue {
    JsonValue::Object(BTreeMap::from_iter(fragment_params.iter().map(|(fragment_id, params)| {
        (
            fragment_id.clone(),
            JsonValue::Object(BTreeMap::from_iter(params.iter().map(|(key, value)| {
                let json = match value {
                    FragmentParamValue::Bool(value) => JsonValue::Bool(*value),
                    FragmentParamValue::U64(value) => JsonValue::Number(*value as i64),
                    FragmentParamValue::String(value) => JsonValue::String(value.clone()),
                };
                (key.clone(), json)
            }))),
        )
    })))
}

fn evidence_overrides_json(
    evidence_overrides: &BTreeMap<FactKindTag, EvidenceTier>,
) -> JsonValue {
    JsonValue::Object(BTreeMap::from_iter(evidence_overrides.iter().map(|(fact_kind, tier)| {
        (
            fact_kind.to_string(),
            JsonValue::String(match tier {
                EvidenceTier::CoreRequirement => "core_requirement".into(),
                EvidenceTier::OptionalEnhancement => "optional_enhancement".into(),
            }),
        )
    })))
}

fn parse_fragment_params(
    value: &JsonValue,
) -> Result<BTreeMap<String, BTreeMap<String, FragmentParamValue>>, ExportError> {
    let object = value.as_object()?;
    object
        .iter()
        .map(|(fragment_id, params)| {
            let params = params.as_object()?;
            let parsed = params
                .iter()
                .map(|(key, value)| {
                    let value = match value {
                        JsonValue::Bool(value) => FragmentParamValue::Bool(*value),
                        JsonValue::Number(value) => FragmentParamValue::U64(*value as u64),
                        JsonValue::String(value) => FragmentParamValue::String(value.clone()),
                        _ => {
                            return Err(ExportError::InvalidShape(format!(
                                "fragment_params.{fragment_id}.{key}"
                            )))
                        }
                    };
                    Ok((key.clone(), value))
                })
                .collect::<Result<BTreeMap<_, _>, _>>()?;
            Ok((fragment_id.clone(), parsed))
        })
        .collect()
}

fn parse_evidence_overrides(
    value: &JsonValue,
) -> Result<BTreeMap<FactKindTag, EvidenceTier>, ExportError> {
    value
        .as_object()?
        .iter()
        .map(|(fact_kind, value)| {
            let fact_kind = FactKindTag::from_str(fact_kind).ok_or_else(|| {
                ExportError::InvalidValue(format!("unknown fact kind '{fact_kind}'"))
            })?;
            let tier = match value.as_str()? {
                "core_requirement" => EvidenceTier::CoreRequirement,
                "optional_enhancement" => EvidenceTier::OptionalEnhancement,
                other => {
                    return Err(ExportError::InvalidValue(format!(
                        "unknown evidence tier '{other}'"
                    )))
                }
            };
            Ok((fact_kind, tier))
        })
        .collect()
}

#[derive(Clone, Debug, PartialEq)]
enum JsonValue {
    Null,
    Bool(bool),
    Number(i64),
    String(String),
    Array(Vec<JsonValue>),
    Object(BTreeMap<String, JsonValue>),
}

impl JsonValue {
    fn render(&self) -> String {
        match self {
            Self::Null => "null".into(),
            Self::Bool(value) => value.to_string(),
            Self::Number(value) => value.to_string(),
            Self::String(value) => format!("\"{}\"", escape_json(value)),
            Self::Array(items) => {
                let inner = items.iter().map(JsonValue::render).collect::<Vec<_>>().join(",");
                format!("[{inner}]")
            }
            Self::Object(map) => {
                let inner = map
                    .iter()
                    .map(|(key, value)| format!("\"{}\":{}", escape_json(key), value.render()))
                    .collect::<Vec<_>>()
                    .join(",");
                format!("{{{inner}}}")
            }
        }
    }

    fn as_str(&self) -> Result<&str, ExportError> {
        match self {
            Self::String(value) => Ok(value),
            _ => Err(ExportError::InvalidShape("expected string".into())),
        }
    }

    fn as_i64(&self) -> Result<i64, ExportError> {
        match self {
            Self::Number(value) => Ok(*value),
            _ => Err(ExportError::InvalidShape("expected number".into())),
        }
    }

    fn as_bool(&self) -> Result<bool, ExportError> {
        match self {
            Self::Bool(value) => Ok(*value),
            _ => Err(ExportError::InvalidShape("expected bool".into())),
        }
    }

    fn as_array(&self) -> Result<&[JsonValue], ExportError> {
        match self {
            Self::Array(value) => Ok(value),
            _ => Err(ExportError::InvalidShape("expected array".into())),
        }
    }

    fn as_object(&self) -> Result<&BTreeMap<String, JsonValue>, ExportError> {
        match self {
            Self::Object(value) => Ok(value),
            _ => Err(ExportError::InvalidShape("expected object".into())),
        }
    }

    fn into_object(self) -> Result<BTreeMap<String, JsonValue>, ExportError> {
        match self {
            Self::Object(value) => Ok(value),
            _ => Err(ExportError::InvalidShape("expected object".into())),
        }
    }
}

struct JsonParser<'a> {
    input: &'a [u8],
    pos: usize,
}

impl<'a> JsonParser<'a> {
    fn new(input: &'a str) -> Self {
        Self {
            input: input.as_bytes(),
            pos: 0,
        }
    }

    fn parse(mut self) -> Result<JsonValue, ExportError> {
        let value = self.parse_value()?;
        self.skip_ws();
        if self.pos != self.input.len() {
            return Err(ExportError::InvalidJson("trailing data".into()));
        }
        Ok(value)
    }

    fn parse_value(&mut self) -> Result<JsonValue, ExportError> {
        self.skip_ws();
        let ch = self.peek().ok_or_else(|| ExportError::InvalidJson("unexpected eof".into()))?;
        match ch {
            b'n' => {
                self.expect_bytes(b"null")?;
                Ok(JsonValue::Null)
            }
            b't' => {
                self.expect_bytes(b"true")?;
                Ok(JsonValue::Bool(true))
            }
            b'f' => {
                self.expect_bytes(b"false")?;
                Ok(JsonValue::Bool(false))
            }
            b'"' => Ok(JsonValue::String(self.parse_string()?)),
            b'[' => self.parse_array(),
            b'{' => self.parse_object(),
            b'-' | b'0'..=b'9' => self.parse_number(),
            _ => Err(ExportError::InvalidJson("invalid token".into())),
        }
    }

    fn parse_array(&mut self) -> Result<JsonValue, ExportError> {
        self.consume(b'[')?;
        let mut values = Vec::new();
        loop {
            self.skip_ws();
            if self.try_consume(b']') {
                break;
            }
            values.push(self.parse_value()?);
            self.skip_ws();
            if self.try_consume(b']') {
                break;
            }
            self.consume(b',')?;
        }
        Ok(JsonValue::Array(values))
    }

    fn parse_object(&mut self) -> Result<JsonValue, ExportError> {
        self.consume(b'{')?;
        let mut map = BTreeMap::new();
        loop {
            self.skip_ws();
            if self.try_consume(b'}') {
                break;
            }
            let key = self.parse_string()?;
            self.skip_ws();
            self.consume(b':')?;
            let value = self.parse_value()?;
            map.insert(key, value);
            self.skip_ws();
            if self.try_consume(b'}') {
                break;
            }
            self.consume(b',')?;
        }
        Ok(JsonValue::Object(map))
    }

    fn parse_string(&mut self) -> Result<String, ExportError> {
        self.consume(b'"')?;
        let mut value = String::new();
        while let Some(ch) = self.peek() {
            self.pos += 1;
            match ch {
                b'"' => return Ok(value),
                b'\\' => {
                    let escaped = self.peek().ok_or_else(|| ExportError::InvalidJson("bad escape".into()))?;
                    self.pos += 1;
                    match escaped {
                        b'"' => value.push('"'),
                        b'\\' => value.push('\\'),
                        b'n' => value.push('\n'),
                        b'r' => value.push('\r'),
                        b't' => value.push('\t'),
                        _ => return Err(ExportError::InvalidJson("unsupported escape".into())),
                    }
                }
                _ => value.push(ch as char),
            }
        }
        Err(ExportError::InvalidJson("unterminated string".into()))
    }

    fn parse_number(&mut self) -> Result<JsonValue, ExportError> {
        let start = self.pos;
        if self.peek() == Some(b'-') {
            self.pos += 1;
        }
        while matches!(self.peek(), Some(b'0'..=b'9')) {
            self.pos += 1;
        }
        let raw = std::str::from_utf8(&self.input[start..self.pos])
            .map_err(|_| ExportError::InvalidJson("bad number".into()))?;
        let value = raw
            .parse::<i64>()
            .map_err(|_| ExportError::InvalidJson("bad number".into()))?;
        Ok(JsonValue::Number(value))
    }

    fn expect_bytes(&mut self, bytes: &[u8]) -> Result<(), ExportError> {
        if self.input.get(self.pos..self.pos + bytes.len()) == Some(bytes) {
            self.pos += bytes.len();
            Ok(())
        } else {
            Err(ExportError::InvalidJson("unexpected token".into()))
        }
    }

    fn consume(&mut self, ch: u8) -> Result<(), ExportError> {
        if self.try_consume(ch) {
            Ok(())
        } else {
            Err(ExportError::InvalidJson("unexpected token".into()))
        }
    }

    fn try_consume(&mut self, ch: u8) -> bool {
        if self.peek() == Some(ch) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn skip_ws(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\n' | b'\r' | b'\t')) {
            self.pos += 1;
        }
    }

    fn peek(&self) -> Option<u8> {
        self.input.get(self.pos).copied()
    }
}

fn escape_json(input: &str) -> String {
    input
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

fn comm_to_string(comm: &[u8; 16]) -> String {
    let end = comm.iter().position(|byte| *byte == 0).unwrap_or(comm.len());
    String::from_utf8_lossy(&comm[..end]).to_string()
}

fn string_to_comm(value: &str) -> [u8; 16] {
    let mut comm = [0u8; 16];
    let bytes = value.as_bytes();
    let len = bytes.len().min(comm.len());
    comm[..len].copy_from_slice(&bytes[..len]);
    comm
}

fn attach_plan_json(plan: &AttachPlan) -> JsonValue {
    JsonValue::Object(BTreeMap::from([
        (
            "fragments".into(),
            JsonValue::Array(
                plan.fragments
                    .iter()
                    .map(|fragment| {
                        JsonValue::Object(BTreeMap::from([
                            ("id".into(), JsonValue::String(fragment.id.into())),
                            ("version".into(), JsonValue::Number(fragment.version as i64)),
                            (
                                "hookpoints".into(),
                                JsonValue::Array(
                                    fragment
                                        .hookpoints
                                        .iter()
                                        .map(|item| JsonValue::String(item.label()))
                                        .collect(),
                                ),
                            ),
                            (
                                "emits".into(),
                                JsonValue::Array(
                                    fragment
                                        .emits
                                        .iter()
                                        .map(|item| JsonValue::String(item.to_string()))
                                        .collect(),
                                ),
                            ),
                            (
                                "requires".into(),
                                JsonValue::Array(
                                    fragment
                                        .requires
                                        .iter()
                                        .map(|item| JsonValue::String(item.to_string()))
                                        .collect(),
                                ),
                            ),
                            (
                                "evidence_classes".into(),
                                JsonValue::Array(
                                    fragment
                                        .evidence_classes
                                        .iter()
                                        .map(|spec| {
                                            JsonValue::Object(BTreeMap::from([
                                                (
                                                    "fact_kind".into(),
                                                    JsonValue::String(spec.fact_kind.to_string()),
                                                ),
                                                (
                                                    "tier".into(),
                                                    JsonValue::String(match spec.tier {
                                                        EvidenceTier::CoreRequirement => {
                                                            "core_requirement"
                                                        }
                                                        EvidenceTier::OptionalEnhancement => {
                                                            "optional_enhancement"
                                                        }
                                                    }
                                                    .into()),
                                                ),
                                            ]))
                                        })
                                        .collect(),
                                ),
                            ),
                            (
                                "maps".into(),
                                JsonValue::Array(
                                    fragment
                                        .maps
                                        .iter()
                                        .map(|map| {
                                            JsonValue::Object(BTreeMap::from([
                                                ("name".into(), JsonValue::String(map.name.into())),
                                                (
                                                    "kind".into(),
                                                    JsonValue::String(match map.kind {
                                                        MapKind::RingBuf => "ringbuf",
                                                        MapKind::Hash => "hash",
                                                        MapKind::LruHash => "lru_hash",
                                                    }
                                                    .into()),
                                                ),
                                                (
                                                    "max_entries".into(),
                                                    JsonValue::Number(map.max_entries as i64),
                                                ),
                                            ]))
                                        })
                                        .collect(),
                                ),
                            ),
                            (
                                "capabilities".into(),
                                JsonValue::Array(
                                    fragment
                                        .capabilities
                                        .iter()
                                        .map(|cap| {
                                            JsonValue::String(match cap {
                                                CapabilityFlag::TcpState => "tcp_state",
                                                CapabilityFlag::PacketMeta => "packet_meta",
                                                CapabilityFlag::RouteMeta => "route_meta",
                                                CapabilityFlag::SockLineage => "sock_lineage",
                                            }
                                            .into())
                                        })
                                        .collect(),
                                ),
                            ),
                            (
                                "params".into(),
                                JsonValue::Array(
                                    fragment
                                        .params
                                        .iter()
                                        .map(|param| {
                                            JsonValue::Object(BTreeMap::from([
                                                ("key".into(), JsonValue::String(param.key.into())),
                                                (
                                                    "value_type".into(),
                                                    JsonValue::String(match param.value_type {
                                                        FragmentParamType::Bool => "bool",
                                                        FragmentParamType::U64 => "u64",
                                                        FragmentParamType::String => "string",
                                                    }
                                                    .into()),
                                                ),
                                            ]))
                                        })
                                        .collect(),
                                ),
                            ),
                        ]))
                    })
                    .collect(),
            ),
        ),
        (
            "hook_graph".into(),
            JsonValue::Array(
                plan.hook_graph
                    .iter()
                    .map(|binding| {
                        JsonValue::Object(BTreeMap::from([
                            ("fragment_id".into(), JsonValue::String(binding.fragment_id.into())),
                            (
                                "hookpoint".into(),
                                JsonValue::String(binding.hookpoint.label()),
                            ),
                        ]))
                    })
                    .collect(),
            ),
        ),
        (
            "fact_graph".into(),
            JsonValue::Array(
                plan.fact_graph
                    .iter()
                    .map(|binding| {
                        JsonValue::Object(BTreeMap::from([
                            ("fragment_id".into(), JsonValue::String(binding.fragment_id.into())),
                            (
                                "emits".into(),
                                JsonValue::Array(
                                    binding
                                        .emits
                                        .iter()
                                        .map(|item| JsonValue::String(item.to_string()))
                                        .collect(),
                                ),
                            ),
                            (
                                "requires".into(),
                                JsonValue::Array(
                                    binding
                                        .requires
                                        .iter()
                                        .map(|item| JsonValue::String(item.to_string()))
                                        .collect(),
                                ),
                            ),
                        ]))
                    })
                    .collect(),
            ),
        ),
        (
            "dependency_graph".into(),
            JsonValue::Array(
                plan.dependency_graph
                    .iter()
                    .map(|edge| {
                        JsonValue::Object(BTreeMap::from([
                            ("fragment_id".into(), JsonValue::String(edge.fragment_id.into())),
                            ("depends_on".into(), JsonValue::String(edge.depends_on.into())),
                            (
                                "fact_kind".into(),
                                JsonValue::String(edge.fact_kind.to_string()),
                            ),
                        ]))
                    })
                    .collect(),
            ),
        ),
        ("coverage".into(), coverage_json(&plan.coverage)),
    ]))
}

fn attach_report_json(report: &AttachReport) -> JsonValue {
    JsonValue::Object(BTreeMap::from([
        (
            "fragments_loaded".into(),
            JsonValue::Array(
                report
                    .fragments_loaded
                    .iter()
                    .map(|item| JsonValue::String(item.clone()))
                    .collect(),
            ),
        ),
        (
            "hookpoints_attached".into(),
            JsonValue::Array(
                report
                    .hookpoints_attached
                    .iter()
                    .map(|item| JsonValue::String(item.clone()))
                    .collect(),
            ),
        ),
        (
            "hookpoints_failed".into(),
            JsonValue::Array(
                report
                    .hookpoints_failed
                    .iter()
                    .map(|item| JsonValue::String(item.clone()))
                    .collect(),
            ),
        ),
        (
            "required_fact_kinds_coverage".into(),
            coverage_json(&report.required_fact_kinds_coverage),
        ),
        (
            "ringbuf_stats".into(),
            JsonValue::Object(BTreeMap::from([
                ("maps".into(), JsonValue::Number(report.ringbuf_stats.maps as i64)),
                (
                    "total_max_entries".into(),
                    JsonValue::Number(report.ringbuf_stats.total_max_entries as i64),
                ),
            ])),
        ),
    ]))
}

fn binding_diagnostics_json(diagnostics: &BindingDiagnostics) -> JsonValue {
    JsonValue::Object(BTreeMap::from([
        (
            "program_model".into(),
            diagnostics.program_model.as_ref().map_or(JsonValue::Null, model_diagnostics_json),
        ),
        (
            "reason_model".into(),
            diagnostics.reason_model.as_ref().map_or(JsonValue::Null, model_diagnostics_json),
        ),
    ]))
}

fn model_diagnostics_json(model: &ModelDiagnostics) -> JsonValue {
    JsonValue::Object(BTreeMap::from([
        ("model".into(), JsonValue::String(model.model.clone())),
        (
            "rules".into(),
            JsonValue::Array(model.rules.iter().map(rule_diagnostics_json).collect()),
        ),
    ]))
}

fn rule_diagnostics_json(rule: &RuleDiagnostics) -> JsonValue {
    JsonValue::Object(BTreeMap::from([
        ("rule_index".into(), JsonValue::Number(rule.rule_index as i64)),
        (
            "tier".into(),
            JsonValue::String(match rule.tier {
                RuleTier::CoreRequirement => "core_requirement",
                RuleTier::OptionalEnhancement => "optional_enhancement",
                RuleTier::Unsupported => "unsupported",
            }
            .into()),
        ),
        (
            "required_facts".into(),
            JsonValue::Array(
                rule.required_facts
                    .iter()
                    .map(|fact| JsonValue::String(fact.to_string()))
                    .collect(),
            ),
        ),
        (
            "supporting_fragments".into(),
            JsonValue::Array(
                rule.supporting_fragments
                    .iter()
                    .map(|fragment| JsonValue::String(fragment.clone()))
                    .collect(),
            ),
        ),
        (
            "missing_facts".into(),
            JsonValue::Array(
                rule.missing_facts
                    .iter()
                    .map(|fact| JsonValue::String(fact.to_string()))
                    .collect(),
            ),
        ),
        ("supported".into(), JsonValue::Bool(rule.supported)),
    ]))
}

fn attach_failure_summary_json(summary: &AttachFailureSummaryItem) -> JsonValue {
    JsonValue::Object(BTreeMap::from([
        (
            "hookpoint_kind".into(),
            JsonValue::String(summary.hookpoint_kind.clone()),
        ),
        ("count".into(), JsonValue::Number(summary.count as i64)),
    ]))
}

fn debug_summary_json(summary: &DebugSummary) -> JsonValue {
    JsonValue::Object(BTreeMap::from([
        (
            "fragments_loaded".into(),
            JsonValue::Number(summary.fragments_loaded as i64),
        ),
        (
            "hookpoints_failed".into(),
            JsonValue::Number(summary.hookpoints_failed as i64),
        ),
        (
            "accepted_facts".into(),
            JsonValue::Number(summary.accepted_facts as i64),
        ),
        (
            "rejected_facts".into(),
            JsonValue::Number(summary.rejected_facts as i64),
        ),
        ("flows".into(), JsonValue::Number(summary.flows as i64)),
        (
            "program_flows".into(),
            JsonValue::Number(summary.program_flows as i64),
        ),
        ("reasons".into(), JsonValue::Number(summary.reasons as i64)),
        ("degraded".into(), JsonValue::Bool(summary.degraded)),
    ]))
}

fn coverage_json(coverage: &CoverageReport) -> JsonValue {
    JsonValue::Object(BTreeMap::from([
        (
            "required".into(),
            JsonValue::Array(
                coverage
                    .required
                    .iter()
                    .map(|item| JsonValue::String(item.to_string()))
                    .collect(),
            ),
        ),
        (
            "covered".into(),
            JsonValue::Array(
                coverage
                    .covered
                    .iter()
                    .map(|item| JsonValue::String(item.to_string()))
                    .collect(),
            ),
        ),
        (
            "missing".into(),
            JsonValue::Array(
                coverage
                    .missing
                    .iter()
                    .map(|item| JsonValue::String(item.to_string()))
                    .collect(),
            ),
        ),
    ]))
}

fn fact_json(fact: &FactEnvelope) -> JsonValue {
    let kind = match &fact.kind {
        FactKind::TcpState(value) => JsonValue::Object(BTreeMap::from([
            ("tag".into(), JsonValue::String(FactKindTag::TcpState.to_string())),
            ("netns".into(), JsonValue::Number(value.netns as i64)),
            ("sk_cookie".into(), JsonValue::Number(value.sk_cookie as i64)),
            ("sport".into(), JsonValue::Number(value.sport as i64)),
            ("dport".into(), JsonValue::Number(value.dport as i64)),
            ("family".into(), JsonValue::Number(value.family as i64)),
            ("old".into(), JsonValue::Number(value.old as i64)),
            ("new".into(), JsonValue::Number(value.new as i64)),
        ])),
        FactKind::PacketMeta(value) => JsonValue::Object(BTreeMap::from([
            ("tag".into(), JsonValue::String(FactKindTag::PacketMeta.to_string())),
            ("netns".into(), JsonValue::Number(value.netns as i64)),
            (
                "sk_cookie".into(),
                value.sk_cookie.map_or(JsonValue::Null, |v| JsonValue::Number(v as i64)),
            ),
            ("dir".into(), JsonValue::String(value.dir.as_str().into())),
            ("l3_proto".into(), JsonValue::Number(value.l3_proto as i64)),
            ("l4_proto".into(), JsonValue::Number(value.l4_proto as i64)),
            ("tot_len".into(), JsonValue::Number(value.tot_len as i64)),
            ("tcp_flags".into(), JsonValue::Number(value.tcp_flags as i64)),
            (
                "seq".into(),
                value.seq.map_or(JsonValue::Null, |v| JsonValue::Number(v as i64)),
            ),
            (
                "ack".into(),
                value.ack.map_or(JsonValue::Null, |v| JsonValue::Number(v as i64)),
            ),
            (
                "window".into(),
                value.window.map_or(JsonValue::Null, |v| JsonValue::Number(v as i64)),
            ),
        ])),
        FactKind::RouteDecision(value) => JsonValue::Object(BTreeMap::from([
            ("tag".into(), JsonValue::String(FactKindTag::RouteDecision.to_string())),
            ("netns".into(), JsonValue::Number(value.netns as i64)),
            (
                "sk_cookie".into(),
                value.sk_cookie.map_or(JsonValue::Null, |v| JsonValue::Number(v as i64)),
            ),
            (
                "fib_table".into(),
                value.fib_table.map_or(JsonValue::Null, |v| JsonValue::Number(v as i64)),
            ),
            ("oif".into(), JsonValue::Number(value.oif as i64)),
        ])),
        FactKind::SockLineage(value) => JsonValue::Object(BTreeMap::from([
            ("tag".into(), JsonValue::String(FactKindTag::SockLineage.to_string())),
            ("netns".into(), JsonValue::Number(value.netns as i64)),
            ("sk_cookie".into(), JsonValue::Number(value.sk_cookie as i64)),
            ("pid".into(), JsonValue::Number(value.pid as i64)),
            ("tid".into(), JsonValue::Number(value.tid as i64)),
            ("cgroup_id".into(), JsonValue::Number(value.cgroup_id as i64)),
            ("comm".into(), JsonValue::String(comm_to_string(&value.comm))),
        ])),
        FactKind::DropAction(value) => JsonValue::Object(BTreeMap::from([
            ("tag".into(), JsonValue::String(FactKindTag::DropAction.to_string())),
            ("flow".into(), JsonValue::Number(value.flow as i64)),
            ("reason_id".into(), JsonValue::Number(value.reason_id as i64)),
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
            ("tag".into(), JsonValue::String(FactKindTag::AttachScope.to_string())),
            ("scope_hash".into(), JsonValue::Number(value.scope_hash as i64)),
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
            fact.ifindex.map_or(JsonValue::Null, |v| JsonValue::Number(v as i64)),
        ),
        ("session".into(), JsonValue::Number(fact.session.0 as i64)),
        (
            "fragment_id".into(),
            JsonValue::String(fact.fragment_id.clone()),
        ),
        ("kind".into(), kind),
    ]))
}

fn rejected_fact_json(rejected: &RejectedFact) -> JsonValue {
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

fn rejected_fact_summary_json(summary: &RejectedFactSummaryItem) -> JsonValue {
    JsonValue::Object(BTreeMap::from([
        (
            "fragment_id".into(),
            JsonValue::String(summary.fragment_id.clone()),
        ),
        ("reason".into(), JsonValue::String(summary.reason.clone())),
        ("count".into(), JsonValue::Number(summary.count as i64)),
    ]))
}

fn flow_json(flow: &FlowSnapshot) -> JsonValue {
    JsonValue::Object(BTreeMap::from([
        ("id".into(), JsonValue::Number(flow.id.0 as i64)),
        (
            "lifecycle".into(),
            JsonValue::Object(BTreeMap::from([
                (
                    "emerged_at".into(),
                    JsonValue::Number(flow.lifecycle.emerged_at.0 as i64),
                ),
                (
                    "last_seen_at".into(),
                    JsonValue::Number(flow.lifecycle.last_seen_at.0 as i64),
                ),
                (
                    "tcp_state_now".into(),
                    flow.lifecycle.tcp_state_now.map_or(JsonValue::Null, |v| JsonValue::Number(v as i64)),
                ),
                (
                    "terminated".into(),
                    JsonValue::Bool(flow.lifecycle.terminated),
                ),
                (
                    "termination_fact".into(),
                    flow.lifecycle
                        .termination_fact
                        .map_or(JsonValue::Null, |v| JsonValue::Number(v.0 as i64)),
                ),
            ])),
        ),
        (
            "path".into(),
            JsonValue::Object(BTreeMap::from([
                (
                    "current_oif".into(),
                    flow.path.current_oif.map_or(JsonValue::Null, |v| JsonValue::Number(v as i64)),
                ),
                (
                    "segments".into(),
                    JsonValue::Array(
                        flow.path
                            .segments
                            .iter()
                            .map(|segment| {
                                JsonValue::Object(BTreeMap::from([
                                    (
                                        "started_at".into(),
                                        JsonValue::Number(segment.started_at.0 as i64),
                                    ),
                                    (
                                        "oif".into(),
                                        segment.oif.map_or(JsonValue::Null, |v| JsonValue::Number(v as i64)),
                                    ),
                                ]))
                            })
                            .collect(),
                    ),
                ),
            ])),
        ),
        (
            "process".into(),
            flow.process.as_ref().map_or(JsonValue::Null, |process| {
                JsonValue::Object(BTreeMap::from([
                    ("pid".into(), JsonValue::Number(process.pid as i64)),
                    ("tid".into(), JsonValue::Number(process.tid as i64)),
                    ("cgroup_id".into(), JsonValue::Number(process.cgroup_id as i64)),
                    ("comm".into(), JsonValue::String(process.comm.clone())),
                ]))
            }),
        ),
        (
            "evidence".into(),
            JsonValue::Object(BTreeMap::from([
                (
                    "tcp_state_facts".into(),
                    fact_id_array(&flow.evidence.tcp_state_facts),
                ),
                ("packet_facts".into(), fact_id_array(&flow.evidence.packet_facts)),
                ("route_facts".into(), fact_id_array(&flow.evidence.route_facts)),
                ("lineage_facts".into(), fact_id_array(&flow.evidence.lineage_facts)),
            ])),
        ),
        (
            "confidence".into(),
            JsonValue::Number((flow.confidence * 1000.0) as i64),
        ),
        (
            "fragment_sources".into(),
            JsonValue::Array(
                flow.fragment_sources
                    .iter()
                    .map(|item| JsonValue::String(item.clone()))
                    .collect(),
            ),
        ),
    ]))
}

fn program_flow_json(flow: &ProgramFlow) -> JsonValue {
    JsonValue::Object(BTreeMap::from([
        ("id".into(), JsonValue::Number(flow.id.0 as i64)),
        (
            "process".into(),
            flow.process.as_ref().map_or(JsonValue::Null, |process| {
                JsonValue::Object(BTreeMap::from([
                    ("pid".into(), JsonValue::Number(process.pid as i64)),
                    ("tid".into(), JsonValue::Number(process.tid as i64)),
                    ("cgroup_id".into(), JsonValue::Number(process.cgroup_id as i64)),
                    ("comm".into(), JsonValue::String(process.comm.clone())),
                ]))
            }),
        ),
        (
            "operation".into(),
            JsonValue::String(program_operation_id(&flow.operation).into()),
        ),
        (
            "transport_flows".into(),
            JsonValue::Array(
                flow.transport_flows
                    .iter()
                    .map(|id| JsonValue::Number(id.0 as i64))
                    .collect(),
            ),
        ),
        (
            "stages".into(),
            JsonValue::Array(
                flow.stages
                    .iter()
                    .map(|stage| {
                        JsonValue::Object(BTreeMap::from([
                            ("at".into(), JsonValue::Number(stage.at.0 as i64)),
                            (
                                "kind".into(),
                                JsonValue::String(stage.kind.id().into()),
                            ),
                        ]))
                    })
                    .collect(),
            ),
        ),
        (
            "narrative".into(),
            JsonValue::Array(
                flow.narrative
                    .iter()
                    .map(|line| JsonValue::String(line.clone()))
                    .collect(),
            ),
        ),
    ]))
}

fn reason_json(reason: &ReasonChain) -> JsonValue {
    JsonValue::Object(BTreeMap::from([
        ("id".into(), JsonValue::Number(reason.id.0 as i64)),
        ("flow".into(), JsonValue::Number(reason.flow.0 as i64)),
        ("l0_facts".into(), fact_id_array(&reason.l0_facts)),
        (
            "l1".into(),
            JsonValue::Object(BTreeMap::from([
                (
                    "tcp_state_timeline".into(),
                    fact_id_array(&reason.l1.tcp_state_timeline),
                ),
                (
                    "path_segments".into(),
                    fact_id_array(&reason.l1.path_segments),
                ),
                (
                    "key_events".into(),
                    JsonValue::Array(
                        reason
                            .l1
                            .key_events
                            .iter()
                            .map(|event| {
                                let mut fields = BTreeMap::from([
                                    ("at".into(), JsonValue::Number(event.at.0 as i64)),
                                    (
                                        "kind".into(),
                                        JsonValue::String(match &event.kind {
                                            KeyEventKind::SynSeen => "syn_seen",
                                            KeyEventKind::UdpDatagramSeen => "udp_datagram_seen",
                                            KeyEventKind::ProcessIdentified => "process_identified",
                                            KeyEventKind::RetransSuspected => "retrans_suspected",
                                            KeyEventKind::RouteChanged => "route_changed",
                                            KeyEventKind::FinOrRst => "fin_or_rst",
                                            KeyEventKind::StateChange { .. } => "state_change",
                                        }
                                        .into()),
                                    ),
                                ]);
                                if let KeyEventKind::StateChange { old, new } = event.kind {
                                    fields.insert("old".into(), JsonValue::Number(old as i64));
                                    fields.insert("new".into(), JsonValue::Number(new as i64));
                                }
                                JsonValue::Object(fields)
                            })
                            .collect(),
                    ),
                ),
            ])),
        ),
        (
            "l3".into(),
            JsonValue::Object(BTreeMap::from([(
                "narrative".into(),
                JsonValue::Array(
                    reason
                        .l3
                        .narrative
                        .iter()
                        .map(|line| {
                            JsonValue::Object(BTreeMap::from([
                                ("at".into(), JsonValue::Number(line.at.0 as i64)),
                                ("text".into(), JsonValue::String(line.text.clone())),
                            ]))
                        })
                        .collect(),
                ),
            )])),
        ),
    ]))
}

fn fact_id_array(ids: &[FactId]) -> JsonValue {
    JsonValue::Array(ids.iter().map(|id| JsonValue::Number(id.0 as i64)).collect())
}

fn parse_fragment_inventory(value: &JsonValue) -> Result<FragmentInventoryItem, ExportError> {
    let object = value.as_object()?;
    Ok(FragmentInventoryItem {
        id: object
            .get("id")
            .ok_or_else(|| ExportError::InvalidShape("fragment_inventory.id".into()))?
            .as_str()?
            .to_string(),
        version: object
            .get("version")
            .ok_or_else(|| ExportError::InvalidShape("fragment_inventory.version".into()))?
            .as_i64()? as u32,
    })
}

fn parse_window_profile(value: &JsonValue) -> Result<WindowProfile, ExportError> {
    let object = value.as_object()?;
    Ok(WindowProfile {
        id: Box::leak(
            object
                .get("id")
                .ok_or_else(|| ExportError::InvalidShape("window_profile.id".into()))?
                .as_str()?
                .to_string()
                .into_boxed_str(),
        ),
        duration_ms: object
            .get("duration_ms")
            .ok_or_else(|| ExportError::InvalidShape("window_profile.duration_ms".into()))?
            .as_i64()? as u64,
        lateness_ms: object
            .get("lateness_ms")
            .ok_or_else(|| ExportError::InvalidShape("window_profile.lateness_ms".into()))?
            .as_i64()? as u64,
    })
}

fn reason_profile_json(profile: &ReasonProfile) -> JsonValue {
    match profile {
        ReasonProfile::HandshakeL1 | ReasonProfile::UdpDatagramL1 => {
            JsonValue::String(profile.id().into())
        }
        ReasonProfile::Declarative(model) => JsonValue::Object(BTreeMap::from([
            ("id".into(), JsonValue::String(model.id.into())),
            ("kind".into(), JsonValue::String("declarative".into())),
            (
                "rules".into(),
                JsonValue::Array(model.rules.iter().map(reason_rule_json).collect()),
            ),
        ])),
    }
}

fn reason_rule_json(rule: &ReasonRule) -> JsonValue {
    JsonValue::Object(BTreeMap::from([
        ("predicate".into(), reason_predicate_json(&rule.predicate)),
        (
            "key_event".into(),
            rule.signal
                .as_ref()
                .map_or(JsonValue::Null, |event| JsonValue::String(reason_key_event_id(event).into())),
        ),
        (
            "narrative".into(),
            reason_narrative_json(&rule.narrative),
        ),
        ("dedupe".into(), JsonValue::Bool(rule.dedupe)),
    ]))
}

fn reason_predicate_json(predicate: &ReasonPredicate) -> JsonValue {
    match predicate {
        ReasonPredicate::ProcessBound => JsonValue::String("process_bound".into()),
        ReasonPredicate::SocketStateObserved => JsonValue::String("socket_state_observed".into()),
        ReasonPredicate::RouteResolved => JsonValue::String("route_resolved".into()),
        ReasonPredicate::DatagramObserved { l4_proto } => JsonValue::Object(BTreeMap::from([
            ("kind".into(), JsonValue::String("datagram_observed".into())),
            ("l4_proto".into(), JsonValue::Number(*l4_proto as i64)),
        ])),
        ReasonPredicate::All(items) => JsonValue::Object(BTreeMap::from([
            ("kind".into(), JsonValue::String("all".into())),
            (
                "items".into(),
                JsonValue::Array(items.iter().map(reason_predicate_json).collect()),
            ),
        ])),
        ReasonPredicate::Any(items) => JsonValue::Object(BTreeMap::from([
            ("kind".into(), JsonValue::String("any".into())),
            (
                "items".into(),
                JsonValue::Array(items.iter().map(reason_predicate_json).collect()),
            ),
        ])),
    }
}

fn reason_key_event_id(event: &ReasonKeyEvent) -> &'static str {
    event.id()
}

fn reason_narrative_json(narrative: &ReasonNarrative) -> JsonValue {
    narrative_template_json(narrative)
}

fn narrative_template_json(narrative: &NarrativeTemplate) -> JsonValue {
    match narrative {
        NarrativeTemplate::None => JsonValue::String("none".into()),
        NarrativeTemplate::ProcessBound => JsonValue::String("process_bound".into()),
        NarrativeTemplate::TcpStateTransition => JsonValue::String("tcp_state_transition".into()),
        NarrativeTemplate::RouteChanged => JsonValue::String("route_changed".into()),
        NarrativeTemplate::UdpDatagramObserved => JsonValue::String("udp_datagram_observed".into()),
        NarrativeTemplate::Static(text) => JsonValue::Object(BTreeMap::from([
            ("kind".into(), JsonValue::String("static".into())),
            ("text".into(), JsonValue::String((*text).into())),
        ])),
    }
}

fn parse_reason_profile(value: &JsonValue) -> Result<ReasonProfile, ExportError> {
    match value {
        JsonValue::String(id) => ReasonProfile::from_id(id)
            .ok_or_else(|| ExportError::InvalidValue("unknown reason profile".into())),
        JsonValue::Object(object) => {
            let kind = object
                .get("kind")
                .ok_or_else(|| ExportError::InvalidShape("reason_profile.kind".into()))?
                .as_str()?;
            match kind {
                "declarative" => Ok(ReasonProfile::Declarative(ReasonModel {
                    id: Box::leak(
                        object
                            .get("id")
                            .ok_or_else(|| ExportError::InvalidShape("reason_profile.id".into()))?
                            .as_str()?
                            .to_string()
                            .into_boxed_str(),
                    ),
                    rules: object
                        .get("rules")
                        .ok_or_else(|| ExportError::InvalidShape("reason_profile.rules".into()))?
                        .as_array()?
                        .iter()
                        .map(parse_reason_rule)
                        .collect::<Result<Vec<_>, _>>()?,
                })),
                _ => Err(ExportError::InvalidValue("unknown reason profile kind".into())),
            }
        }
        _ => Err(ExportError::InvalidShape("reason_profile".into())),
    }
}

fn parse_reason_rule(value: &JsonValue) -> Result<ReasonRule, ExportError> {
    let object = value.as_object()?;
    Ok(ReasonRule {
        predicate: parse_reason_predicate(
            object
                .get("predicate")
                .ok_or_else(|| ExportError::InvalidShape("reason_rule.predicate".into()))?,
        )?,
        signal: match object.get("key_event").unwrap_or(&JsonValue::Null) {
            JsonValue::Null => None,
            value => Some(parse_reason_key_event(value.as_str()?)?),
        },
        narrative: parse_reason_narrative(
            object
                .get("narrative")
                .ok_or_else(|| ExportError::InvalidShape("reason_rule.narrative".into()))?,
        )?,
        dedupe: object
            .get("dedupe")
            .ok_or_else(|| ExportError::InvalidShape("reason_rule.dedupe".into()))?
            .as_bool()?,
    })
}

fn parse_reason_predicate(value: &JsonValue) -> Result<ReasonPredicate, ExportError> {
    match value {
        JsonValue::String(id) => match id.as_str() {
            "process_bound" => Ok(ReasonPredicate::ProcessBound),
            "socket_state_observed" => Ok(ReasonPredicate::SocketStateObserved),
            "route_resolved" => Ok(ReasonPredicate::RouteResolved),
            _ => Err(ExportError::InvalidValue("unknown reason predicate".into())),
        },
        JsonValue::Object(object) => match object
            .get("kind")
            .ok_or_else(|| ExportError::InvalidShape("reason_predicate.kind".into()))?
            .as_str()?
        {
            "datagram_observed" => Ok(ReasonPredicate::DatagramObserved {
                l4_proto: object
                    .get("l4_proto")
                    .ok_or_else(|| ExportError::InvalidShape("reason_predicate.l4_proto".into()))?
                    .as_i64()? as u8,
            }),
            "all" => Ok(ReasonPredicate::All(
                object
                    .get("items")
                    .ok_or_else(|| ExportError::InvalidShape("reason_predicate.items".into()))?
                    .as_array()?
                    .iter()
                    .map(parse_reason_predicate)
                    .collect::<Result<Vec<_>, _>>()?,
            )),
            "any" => Ok(ReasonPredicate::Any(
                object
                    .get("items")
                    .ok_or_else(|| ExportError::InvalidShape("reason_predicate.items".into()))?
                    .as_array()?
                    .iter()
                    .map(parse_reason_predicate)
                    .collect::<Result<Vec<_>, _>>()?,
            )),
            _ => Err(ExportError::InvalidValue("unknown reason predicate kind".into())),
        },
        _ => Err(ExportError::InvalidShape("reason_predicate".into())),
    }
}

fn parse_reason_key_event(value: &str) -> Result<ReasonKeyEvent, ExportError> {
    SignalKind::from_id(value).ok_or_else(|| ExportError::InvalidValue("unknown reason key event".into()))
}

fn parse_reason_narrative(value: &JsonValue) -> Result<ReasonNarrative, ExportError> {
    parse_narrative_template(value)
}

fn parse_narrative_template(value: &JsonValue) -> Result<NarrativeTemplate, ExportError> {
    match value {
        JsonValue::String(id) => match id.as_str() {
            "none" => Ok(NarrativeTemplate::None),
            "process_bound" => Ok(NarrativeTemplate::ProcessBound),
            "tcp_state_transition" => Ok(NarrativeTemplate::TcpStateTransition),
            "route_changed" => Ok(NarrativeTemplate::RouteChanged),
            "udp_datagram_observed" => Ok(NarrativeTemplate::UdpDatagramObserved),
            _ => Err(ExportError::InvalidValue("unknown reason narrative".into())),
        },
        JsonValue::Object(object) => match object
            .get("kind")
            .ok_or_else(|| ExportError::InvalidShape("reason_narrative.kind".into()))?
            .as_str()?
        {
            "static" => Ok(NarrativeTemplate::Static(Box::leak(
                object
                    .get("text")
                    .ok_or_else(|| ExportError::InvalidShape("reason_narrative.text".into()))?
                    .as_str()?
                    .to_string()
                    .into_boxed_str(),
            ))),
            _ => Err(ExportError::InvalidValue("unknown reason narrative kind".into())),
        },
        _ => Err(ExportError::InvalidShape("reason_narrative".into())),
    }
}

fn parse_attach_plan(value: &JsonValue) -> Result<AttachPlan, ExportError> {
    let object = value.as_object()?;
    Ok(AttachPlan {
        fragments: object
            .get("fragments")
            .ok_or_else(|| ExportError::InvalidShape("attach_plan.fragments".into()))?
            .as_array()?
            .iter()
            .map(parse_fragment_descriptor)
            .collect::<Result<Vec<_>, _>>()?,
        hook_graph: object
            .get("hook_graph")
            .ok_or_else(|| ExportError::InvalidShape("attach_plan.hook_graph".into()))?
            .as_array()?
            .iter()
            .map(parse_hook_binding)
            .collect::<Result<Vec<_>, _>>()?,
        fact_graph: object
            .get("fact_graph")
            .ok_or_else(|| ExportError::InvalidShape("attach_plan.fact_graph".into()))?
            .as_array()?
            .iter()
            .map(parse_fact_binding)
            .collect::<Result<Vec<_>, _>>()?,
        dependency_graph: object
            .get("dependency_graph")
            .ok_or_else(|| ExportError::InvalidShape("attach_plan.dependency_graph".into()))?
            .as_array()?
            .iter()
            .map(parse_dependency_edge)
            .collect::<Result<Vec<_>, _>>()?,
        coverage: parse_coverage(
            object
                .get("coverage")
                .ok_or_else(|| ExportError::InvalidShape("attach_plan.coverage".into()))?,
        )?,
    })
}

fn parse_attach_report(value: &JsonValue) -> Result<AttachReport, ExportError> {
    let object = value.as_object()?;
    Ok(AttachReport {
        fragments_loaded: object
            .get("fragments_loaded")
            .ok_or_else(|| ExportError::InvalidShape("attach_report.fragments_loaded".into()))?
            .as_array()?
            .iter()
            .map(|item| Ok(item.as_str()?.to_string()))
            .collect::<Result<Vec<_>, _>>()?,
        hookpoints_attached: object
            .get("hookpoints_attached")
            .ok_or_else(|| ExportError::InvalidShape("attach_report.hookpoints_attached".into()))?
            .as_array()?
            .iter()
            .map(|item| Ok(item.as_str()?.to_string()))
            .collect::<Result<Vec<_>, _>>()?,
        hookpoints_failed: object
            .get("hookpoints_failed")
            .ok_or_else(|| ExportError::InvalidShape("attach_report.hookpoints_failed".into()))?
            .as_array()?
            .iter()
            .map(|item| Ok(item.as_str()?.to_string()))
            .collect::<Result<Vec<_>, _>>()?,
        required_fact_kinds_coverage: parse_coverage(
            object.get("required_fact_kinds_coverage").ok_or_else(|| {
                ExportError::InvalidShape("attach_report.required_fact_kinds_coverage".into())
            })?,
        )?,
        ringbuf_stats: {
            let stats = object
                .get("ringbuf_stats")
                .ok_or_else(|| ExportError::InvalidShape("attach_report.ringbuf_stats".into()))?
                .as_object()?;
            RingBufStats {
                maps: stats
                    .get("maps")
                    .ok_or_else(|| ExportError::InvalidShape("attach_report.ringbuf_stats.maps".into()))?
                    .as_i64()? as usize,
                total_max_entries: stats
                    .get("total_max_entries")
                    .ok_or_else(|| {
                        ExportError::InvalidShape(
                            "attach_report.ringbuf_stats.total_max_entries".into(),
                        )
                    })?
                    .as_i64()? as u32,
            }
        },
    })
}

fn parse_binding_diagnostics(value: &JsonValue) -> Result<BindingDiagnostics, ExportError> {
    let object = value.as_object()?;
    Ok(BindingDiagnostics {
        program_model: match object.get("program_model").unwrap_or(&JsonValue::Null) {
            JsonValue::Null => None,
            value => Some(parse_model_diagnostics(value)?),
        },
        reason_model: match object.get("reason_model").unwrap_or(&JsonValue::Null) {
            JsonValue::Null => None,
            value => Some(parse_model_diagnostics(value)?),
        },
    })
}

fn parse_model_diagnostics(value: &JsonValue) -> Result<ModelDiagnostics, ExportError> {
    let object = value.as_object()?;
    Ok(ModelDiagnostics {
        model: object
            .get("model")
            .ok_or_else(|| ExportError::InvalidShape("model_diagnostics.model".into()))?
            .as_str()?
            .to_string(),
        rules: object
            .get("rules")
            .ok_or_else(|| ExportError::InvalidShape("model_diagnostics.rules".into()))?
            .as_array()?
            .iter()
            .map(parse_rule_diagnostics)
            .collect::<Result<Vec<_>, _>>()?,
    })
}

fn parse_rule_diagnostics(value: &JsonValue) -> Result<RuleDiagnostics, ExportError> {
    let object = value.as_object()?;
    Ok(RuleDiagnostics {
        rule_index: object
            .get("rule_index")
            .ok_or_else(|| ExportError::InvalidShape("rule_diagnostics.rule_index".into()))?
            .as_i64()? as usize,
        tier: match object
            .get("tier")
            .ok_or_else(|| ExportError::InvalidShape("rule_diagnostics.tier".into()))?
            .as_str()?
        {
            "core_requirement" => RuleTier::CoreRequirement,
            "optional_enhancement" => RuleTier::OptionalEnhancement,
            "unsupported" => RuleTier::Unsupported,
            _ => return Err(ExportError::InvalidValue("unknown rule diagnostics tier".into())),
        },
        required_facts: parse_fact_kind_list(
            object
                .get("required_facts")
                .ok_or_else(|| ExportError::InvalidShape("rule_diagnostics.required_facts".into()))?,
        )?,
        supporting_fragments: object
            .get("supporting_fragments")
            .ok_or_else(|| {
                ExportError::InvalidShape("rule_diagnostics.supporting_fragments".into())
            })?
            .as_array()?
            .iter()
            .map(|item| Ok(item.as_str()?.to_string()))
            .collect::<Result<Vec<_>, _>>()?,
        missing_facts: parse_fact_kind_list(
            object
                .get("missing_facts")
                .ok_or_else(|| ExportError::InvalidShape("rule_diagnostics.missing_facts".into()))?,
        )?,
        supported: object
            .get("supported")
            .ok_or_else(|| ExportError::InvalidShape("rule_diagnostics.supported".into()))?
            .as_bool()?,
    })
}

fn parse_coverage(value: &JsonValue) -> Result<CoverageReport, ExportError> {
    let object = value.as_object()?;
    Ok(CoverageReport {
        required: parse_fact_kind_list(
            object
                .get("required")
                .ok_or_else(|| ExportError::InvalidShape("coverage.required".into()))?,
        )?,
        covered: parse_fact_kind_list(
            object
                .get("covered")
                .ok_or_else(|| ExportError::InvalidShape("coverage.covered".into()))?,
        )?,
        missing: parse_fact_kind_list(
            object
                .get("missing")
                .ok_or_else(|| ExportError::InvalidShape("coverage.missing".into()))?,
        )?,
    })
}

fn parse_fact_kind_list(value: &JsonValue) -> Result<Vec<FactKindTag>, ExportError> {
    value
        .as_array()?
        .iter()
        .map(|item| {
            FactKindTag::from_str(item.as_str()?)
                .ok_or_else(|| ExportError::InvalidValue("unknown fact kind".into()))
        })
        .collect()
}

fn parse_fragment_descriptor(value: &JsonValue) -> Result<FragmentDescriptor, ExportError> {
    let object = value.as_object()?;
    Ok(FragmentDescriptor {
        id: Box::leak(
            object
                .get("id")
                .ok_or_else(|| ExportError::InvalidShape("fragment.id".into()))?
                .as_str()?
                .to_string()
                .into_boxed_str(),
        ),
        version: object
            .get("version")
            .ok_or_else(|| ExportError::InvalidShape("fragment.version".into()))?
            .as_i64()? as u32,
        hookpoints: object
            .get("hookpoints")
            .ok_or_else(|| ExportError::InvalidShape("fragment.hookpoints".into()))?
            .as_array()?
            .iter()
            .map(parse_hookpoint_value)
            .collect::<Result<Vec<_>, _>>()?,
        emits: parse_fact_kind_list(
            object
                .get("emits")
                .ok_or_else(|| ExportError::InvalidShape("fragment.emits".into()))?,
        )?,
        evidence_classes: object
            .get("evidence_classes")
            .unwrap_or(&JsonValue::Array(Vec::new()))
            .as_array()?
            .iter()
            .map(parse_evidence_class_spec)
            .collect::<Result<Vec<_>, _>>()?,
        requires: parse_fact_kind_list(
            object
                .get("requires")
                .ok_or_else(|| ExportError::InvalidShape("fragment.requires".into()))?,
        )?,
        maps: object
            .get("maps")
            .ok_or_else(|| ExportError::InvalidShape("fragment.maps".into()))?
            .as_array()?
            .iter()
            .map(parse_map_spec)
            .collect::<Result<Vec<_>, _>>()?,
        capabilities: object
            .get("capabilities")
            .ok_or_else(|| ExportError::InvalidShape("fragment.capabilities".into()))?
            .as_array()?
            .iter()
            .map(|item| match item.as_str()? {
                "tcp_state" => Ok(CapabilityFlag::TcpState),
                "packet_meta" => Ok(CapabilityFlag::PacketMeta),
                "route_meta" => Ok(CapabilityFlag::RouteMeta),
                "sock_lineage" => Ok(CapabilityFlag::SockLineage),
                _ => Err(ExportError::InvalidValue("unknown capability".into())),
            })
            .collect::<Result<Vec<_>, _>>()?,
        params: object
            .get("params")
            .unwrap_or(&JsonValue::Array(Vec::new()))
            .as_array()?
            .iter()
            .map(parse_fragment_param_spec)
            .collect::<Result<Vec<_>, _>>()?,
    })
}

fn parse_evidence_class_spec(value: &JsonValue) -> Result<EvidenceClassSpec, ExportError> {
    let object = value.as_object()?;
    Ok(EvidenceClassSpec {
        fact_kind: FactKindTag::from_str(
            object
                .get("fact_kind")
                .ok_or_else(|| ExportError::InvalidShape("fragment.evidence_class.fact_kind".into()))?
                .as_str()?,
        )
        .ok_or_else(|| ExportError::InvalidValue("unknown fact kind".into()))?,
        tier: match object
            .get("tier")
            .ok_or_else(|| ExportError::InvalidShape("fragment.evidence_class.tier".into()))?
            .as_str()?
        {
            "core_requirement" => EvidenceTier::CoreRequirement,
            "optional_enhancement" => EvidenceTier::OptionalEnhancement,
            _ => return Err(ExportError::InvalidValue("unknown evidence tier".into())),
        },
    })
}

fn parse_fragment_param_spec(value: &JsonValue) -> Result<FragmentParamSpec, ExportError> {
    let object = value.as_object()?;
    Ok(FragmentParamSpec {
        key: Box::leak(
            object
                .get("key")
                .ok_or_else(|| ExportError::InvalidShape("fragment.param.key".into()))?
                .as_str()?
                .to_string()
                .into_boxed_str(),
        ),
        value_type: match object
            .get("value_type")
            .ok_or_else(|| ExportError::InvalidShape("fragment.param.value_type".into()))?
            .as_str()?
        {
            "bool" => FragmentParamType::Bool,
            "u64" => FragmentParamType::U64,
            "string" => FragmentParamType::String,
            _ => return Err(ExportError::InvalidValue("unknown fragment param type".into())),
        },
    })
}

fn parse_hookpoint_value(value: &JsonValue) -> Result<HookPoint, ExportError> {
    parse_hookpoint(value.as_str()?)
}

fn parse_hookpoint(input: &str) -> Result<HookPoint, ExportError> {
    if let Some(value) = input.strip_prefix("tracepoint:") {
        return Ok(HookPoint::TracePoint(Box::leak(value.to_string().into_boxed_str())));
    }
    if let Some(value) = input.strip_prefix("kprobe:") {
        return Ok(HookPoint::KProbe(Box::leak(value.to_string().into_boxed_str())));
    }
    match input {
        "tc:ingress" => Ok(HookPoint::TCIngress),
        "tc:egress" => Ok(HookPoint::TCEgress),
        _ => Err(ExportError::InvalidValue("unknown hookpoint".into())),
    }
}

fn parse_map_spec(value: &JsonValue) -> Result<MapSpec, ExportError> {
    let object = value.as_object()?;
    Ok(MapSpec {
        name: Box::leak(
            object
                .get("name")
                .ok_or_else(|| ExportError::InvalidShape("map.name".into()))?
                .as_str()?
                .to_string()
                .into_boxed_str(),
        ),
        kind: match object
            .get("kind")
            .ok_or_else(|| ExportError::InvalidShape("map.kind".into()))?
            .as_str()?
        {
            "ringbuf" => MapKind::RingBuf,
            "hash" => MapKind::Hash,
            "lru_hash" => MapKind::LruHash,
            _ => return Err(ExportError::InvalidValue("unknown map kind".into())),
        },
        max_entries: object
            .get("max_entries")
            .ok_or_else(|| ExportError::InvalidShape("map.max_entries".into()))?
            .as_i64()? as u32,
    })
}

fn parse_hook_binding(value: &JsonValue) -> Result<HookBinding, ExportError> {
    let object = value.as_object()?;
    Ok(HookBinding {
        fragment_id: Box::leak(
            object
                .get("fragment_id")
                .ok_or_else(|| ExportError::InvalidShape("hook_binding.fragment_id".into()))?
                .as_str()?
                .to_string()
                .into_boxed_str(),
        ),
        hookpoint: parse_hookpoint(
            object
                .get("hookpoint")
                .ok_or_else(|| ExportError::InvalidShape("hook_binding.hookpoint".into()))?
                .as_str()?,
        )?,
    })
}

fn parse_fact_binding(value: &JsonValue) -> Result<FactBinding, ExportError> {
    let object = value.as_object()?;
    Ok(FactBinding {
        fragment_id: Box::leak(
            object
                .get("fragment_id")
                .ok_or_else(|| ExportError::InvalidShape("fact_binding.fragment_id".into()))?
                .as_str()?
                .to_string()
                .into_boxed_str(),
        ),
        emits: parse_fact_kind_list(
            object
                .get("emits")
                .ok_or_else(|| ExportError::InvalidShape("fact_binding.emits".into()))?,
        )?,
        requires: parse_fact_kind_list(
            object
                .get("requires")
                .ok_or_else(|| ExportError::InvalidShape("fact_binding.requires".into()))?,
        )?,
    })
}

fn parse_dependency_edge(value: &JsonValue) -> Result<DependencyEdge, ExportError> {
    let object = value.as_object()?;
    Ok(DependencyEdge {
        fragment_id: Box::leak(
            object
                .get("fragment_id")
                .ok_or_else(|| ExportError::InvalidShape("edge.fragment_id".into()))?
                .as_str()?
                .to_string()
                .into_boxed_str(),
        ),
        depends_on: Box::leak(
            object
                .get("depends_on")
                .ok_or_else(|| ExportError::InvalidShape("edge.depends_on".into()))?
                .as_str()?
                .to_string()
                .into_boxed_str(),
        ),
        fact_kind: FactKindTag::from_str(
            object
                .get("fact_kind")
                .ok_or_else(|| ExportError::InvalidShape("edge.fact_kind".into()))?
                .as_str()?,
        )
        .ok_or_else(|| ExportError::InvalidValue("unknown fact kind".into()))?,
    })
}

fn parse_fact(value: &JsonValue) -> Result<FactEnvelope, ExportError> {
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
                    .ok_or_else(|| ExportError::InvalidShape("fact.drop_action.packet_fact".into()))?
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

fn parse_flow(value: &JsonValue) -> Result<FlowSnapshot, ExportError> {
    let object = value.as_object()?;
    let lifecycle = object
        .get("lifecycle")
        .ok_or_else(|| ExportError::InvalidShape("flow.lifecycle".into()))?
        .as_object()?;
    let path = object
        .get("path")
        .ok_or_else(|| ExportError::InvalidShape("flow.path".into()))?
        .as_object()?;
    let process = object.get("process").unwrap_or(&JsonValue::Null);
    let evidence = object
        .get("evidence")
        .ok_or_else(|| ExportError::InvalidShape("flow.evidence".into()))?
        .as_object()?;

    Ok(FlowSnapshot {
        id: FlowId(
            object
                .get("id")
                .ok_or_else(|| ExportError::InvalidShape("flow.id".into()))?
                .as_i64()? as u64,
        ),
        lifecycle: FlowLifecycleView {
            emerged_at: FactId(
                lifecycle
                    .get("emerged_at")
                    .ok_or_else(|| ExportError::InvalidShape("flow.lifecycle.emerged_at".into()))?
                    .as_i64()? as u64,
            ),
            last_seen_at: FactId(
                lifecycle
                    .get("last_seen_at")
                    .ok_or_else(|| ExportError::InvalidShape("flow.lifecycle.last_seen_at".into()))?
                    .as_i64()? as u64,
            ),
            tcp_state_now: parse_optional_u8(lifecycle.get("tcp_state_now").unwrap_or(&JsonValue::Null))?,
            terminated: lifecycle
                .get("terminated")
                .ok_or_else(|| ExportError::InvalidShape("flow.lifecycle.terminated".into()))?
                .as_bool()?,
            termination_fact: parse_optional_fact_id(
                lifecycle.get("termination_fact").unwrap_or(&JsonValue::Null),
            )?,
        },
        path: PathView {
            current_oif: parse_optional_u32(path.get("current_oif").unwrap_or(&JsonValue::Null))?,
            current_gw: None,
            segments: path
                .get("segments")
                .ok_or_else(|| ExportError::InvalidShape("flow.path.segments".into()))?
                .as_array()?
                .iter()
                .map(|item| {
                    let object = item.as_object()?;
                    Ok(PathSegment {
                        started_at: FactId(
                            object
                                .get("started_at")
                                .ok_or_else(|| {
                                    ExportError::InvalidShape("flow.path.segment.started_at".into())
                                })?
                                .as_i64()? as u64,
                        ),
                        oif: parse_optional_u32(object.get("oif").unwrap_or(&JsonValue::Null))?,
                        gw: None,
                    })
                })
                .collect::<Result<Vec<_>, _>>()?,
        },
        process: parse_process_view(process)?,
        evidence: EvidenceIndex {
            tcp_state_facts: parse_fact_ids(
                evidence.get("tcp_state_facts").unwrap_or(&JsonValue::Array(vec![])),
            )?,
            packet_facts: parse_fact_ids(
                evidence.get("packet_facts").unwrap_or(&JsonValue::Array(vec![])),
            )?,
            route_facts: parse_fact_ids(
                evidence.get("route_facts").unwrap_or(&JsonValue::Array(vec![])),
            )?,
            lineage_facts: parse_fact_ids(
                evidence.get("lineage_facts").unwrap_or(&JsonValue::Array(vec![])),
            )?,
        },
        confidence: object
            .get("confidence")
            .ok_or_else(|| ExportError::InvalidShape("flow.confidence".into()))?
            .as_i64()? as f32
            / 1000.0,
        fragment_sources: object
            .get("fragment_sources")
            .ok_or_else(|| ExportError::InvalidShape("flow.fragment_sources".into()))?
            .as_array()?
            .iter()
            .map(|item| Ok(item.as_str()?.to_string()))
            .collect::<Result<Vec<_>, _>>()?,
    })
}

fn parse_reason(value: &JsonValue) -> Result<ReasonChain, ExportError> {
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
            tcp_state_timeline: parse_fact_ids(
                l1.get("tcp_state_timeline")
                    .ok_or_else(|| ExportError::InvalidShape("reason.l1.tcp_state_timeline".into()))?,
            )?,
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
                .map(|item| {
                    let object = item.as_object()?;
                    Ok(NarrLine {
                        at: FactId(
                            object
                                .get("at")
                                .ok_or_else(|| ExportError::InvalidShape("reason.narrative.at".into()))?
                                .as_i64()? as u64,
                        ),
                        text: object
                            .get("text")
                            .ok_or_else(|| ExportError::InvalidShape("reason.narrative.text".into()))?
                            .as_str()?
                            .to_string(),
                    })
                })
                .collect::<Result<Vec<_>, _>>()?,
        },
    })
}

fn parse_key_event(value: &JsonValue) -> Result<KeyEvent, ExportError> {
    let object = value.as_object()?;
    let kind = match object
        .get("kind")
        .ok_or_else(|| ExportError::InvalidShape("key_event.kind".into()))?
        .as_str()?
    {
        "syn_seen" => KeyEventKind::SynSeen,
        "udp_datagram_seen" => KeyEventKind::UdpDatagramSeen,
        "process_identified" => KeyEventKind::ProcessIdentified,
        "retrans_suspected" => KeyEventKind::RetransSuspected,
        "route_changed" => KeyEventKind::RouteChanged,
        "fin_or_rst" => KeyEventKind::FinOrRst,
        "state_change" => KeyEventKind::StateChange {
            old: object
                .get("old")
                .ok_or_else(|| ExportError::InvalidShape("key_event.old".into()))?
                .as_i64()? as u8,
            new: object
                .get("new")
                .ok_or_else(|| ExportError::InvalidShape("key_event.new".into()))?
                .as_i64()? as u8,
        },
        _ => return Err(ExportError::InvalidValue("unknown key event".into())),
    };
    Ok(KeyEvent {
        at: FactId(
            object
                .get("at")
                .ok_or_else(|| ExportError::InvalidShape("key_event.at".into()))?
                .as_i64()? as u64,
        ),
        kind,
    })
}

fn parse_rejected_fact(value: &JsonValue) -> Result<RejectedFact, ExportError> {
    let object = value.as_object()?;
    let reason = match object
        .get("reason")
        .ok_or_else(|| ExportError::InvalidShape("rejected_fact.reason".into()))?
        .as_str()?
    {
        "fragment_not_loaded" => RejectedFactReason::FragmentNotLoaded,
        "filtered_by_fragment_param" => RejectedFactReason::FilteredByFragmentParam,
        other => {
            return Err(ExportError::InvalidValue(format!(
                "unknown rejected fact reason: {other}"
            )))
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

fn parse_rejected_fact_summary(value: &JsonValue) -> Result<RejectedFactSummaryItem, ExportError> {
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

fn parse_attach_failure_summary(value: &JsonValue) -> Result<AttachFailureSummaryItem, ExportError> {
    let object = value.as_object()?;
    Ok(AttachFailureSummaryItem {
        hookpoint_kind: object
            .get("hookpoint_kind")
            .ok_or_else(|| ExportError::InvalidShape("attach_failure_summary.hookpoint_kind".into()))?
            .as_str()?
            .to_string(),
        count: object
            .get("count")
            .ok_or_else(|| ExportError::InvalidShape("attach_failure_summary.count".into()))?
            .as_i64()? as u64,
    })
}

fn parse_debug_summary(value: &JsonValue) -> Result<DebugSummary, ExportError> {
    let object = value.as_object()?;
    Ok(DebugSummary {
        fragments_loaded: object
            .get("fragments_loaded")
            .ok_or_else(|| ExportError::InvalidShape("debug_summary.fragments_loaded".into()))?
            .as_i64()? as u64,
        hookpoints_failed: object
            .get("hookpoints_failed")
            .ok_or_else(|| ExportError::InvalidShape("debug_summary.hookpoints_failed".into()))?
            .as_i64()? as u64,
        accepted_facts: object
            .get("accepted_facts")
            .ok_or_else(|| ExportError::InvalidShape("debug_summary.accepted_facts".into()))?
            .as_i64()? as u64,
        rejected_facts: object
            .get("rejected_facts")
            .ok_or_else(|| ExportError::InvalidShape("debug_summary.rejected_facts".into()))?
            .as_i64()? as u64,
        flows: object
            .get("flows")
            .ok_or_else(|| ExportError::InvalidShape("debug_summary.flows".into()))?
            .as_i64()? as u64,
        program_flows: object
            .get("program_flows")
            .ok_or_else(|| ExportError::InvalidShape("debug_summary.program_flows".into()))?
            .as_i64()? as u64,
        reasons: object
            .get("reasons")
            .ok_or_else(|| ExportError::InvalidShape("debug_summary.reasons".into()))?
            .as_i64()? as u64,
        degraded: object
            .get("degraded")
            .ok_or_else(|| ExportError::InvalidShape("debug_summary.degraded".into()))?
            .as_bool()?,
    })
}

fn parse_process_view(value: &JsonValue) -> Result<Option<crate::flow::ProcessView>, ExportError> {
    match value {
        JsonValue::Null => Ok(None),
        JsonValue::Object(object) => Ok(Some(crate::flow::ProcessView {
            pid: object
                .get("pid")
                .ok_or_else(|| ExportError::InvalidShape("flow.process.pid".into()))?
                .as_i64()? as u32,
            tid: object
                .get("tid")
                .ok_or_else(|| ExportError::InvalidShape("flow.process.tid".into()))?
                .as_i64()? as u32,
            cgroup_id: object
                .get("cgroup_id")
                .ok_or_else(|| ExportError::InvalidShape("flow.process.cgroup_id".into()))?
                .as_i64()? as u64,
            comm: object
                .get("comm")
                .ok_or_else(|| ExportError::InvalidShape("flow.process.comm".into()))?
                .as_str()?
                .to_string(),
        })),
        _ => Err(ExportError::InvalidShape("expected flow.process".into())),
    }
}

fn parse_program_flow(value: &JsonValue) -> Result<ProgramFlow, ExportError> {
    let object = value.as_object()?;
    Ok(ProgramFlow {
        id: ProgramFlowId(
            object
                .get("id")
                .ok_or_else(|| ExportError::InvalidShape("program_flow.id".into()))?
                .as_i64()? as u64,
        ),
        process: parse_process_view(object.get("process").unwrap_or(&JsonValue::Null))?,
        operation: match object
            .get("operation")
            .ok_or_else(|| ExportError::InvalidShape("program_flow.operation".into()))?
            .as_str()?
        {
            "connect_flow" => ProgramOperation::ConnectFlow,
            "datagram_exchange" => ProgramOperation::DatagramExchange,
            "unknown" => ProgramOperation::Unknown,
            other => ProgramOperation::Custom(other.into()),
        },
        transport_flows: object
            .get("transport_flows")
            .ok_or_else(|| ExportError::InvalidShape("program_flow.transport_flows".into()))?
            .as_array()?
            .iter()
            .map(|item| Ok(FlowId(item.as_i64()? as u64)))
            .collect::<Result<Vec<_>, _>>()?,
        stages: object
            .get("stages")
            .ok_or_else(|| ExportError::InvalidShape("program_flow.stages".into()))?
            .as_array()?
            .iter()
            .map(|item| {
                let object = item.as_object()?;
                Ok(ProgramStage {
                    at: FactId(
                        object
                            .get("at")
                            .ok_or_else(|| {
                                ExportError::InvalidShape("program_flow.stage.at".into())
                            })?
                            .as_i64()? as u64,
                    ),
                    kind: match object
                        .get("kind")
                        .ok_or_else(|| {
                            ExportError::InvalidShape("program_flow.stage.kind".into())
                        })?
                        .as_str()?
                    {
                        other => SignalKind::from_id(other).ok_or_else(|| {
                            ExportError::InvalidValue(format!(
                                "unknown program flow stage kind: {other}"
                            ))
                        })?,
                    },
                })
            })
            .collect::<Result<Vec<_>, _>>()?,
        narrative: object
            .get("narrative")
            .ok_or_else(|| ExportError::InvalidShape("program_flow.narrative".into()))?
            .as_array()?
            .iter()
            .map(|item| Ok(item.as_str()?.to_string()))
            .collect::<Result<Vec<_>, _>>()?,
    })
}

fn program_operation_id(operation: &ProgramOperation) -> &str {
    match operation {
        ProgramOperation::ConnectFlow => "connect_flow",
        ProgramOperation::DatagramExchange => "datagram_exchange",
        ProgramOperation::Custom(id) => id.as_str(),
        ProgramOperation::Unknown => "unknown",
    }
}

fn parse_fact_ids(value: &JsonValue) -> Result<Vec<FactId>, ExportError> {
    value
        .as_array()?
        .iter()
        .map(|item| Ok(FactId(item.as_i64()? as u64)))
        .collect()
}

fn parse_optional_u64(value: &JsonValue) -> Result<Option<u64>, ExportError> {
    match value {
        JsonValue::Null => Ok(None),
        JsonValue::Number(value) => Ok(Some(*value as u64)),
        _ => Err(ExportError::InvalidShape("expected optional u64".into())),
    }
}

fn parse_optional_u32(value: &JsonValue) -> Result<Option<u32>, ExportError> {
    parse_optional_u64(value).map(|value| value.map(|item| item as u32))
}

fn parse_optional_u16(value: &JsonValue) -> Result<Option<u16>, ExportError> {
    parse_optional_u64(value).map(|value| value.map(|item| item as u16))
}

fn parse_optional_u8(value: &JsonValue) -> Result<Option<u8>, ExportError> {
    parse_optional_u64(value).map(|value| value.map(|item| item as u8))
}

fn parse_optional_fact_id(value: &JsonValue) -> Result<Option<FactId>, ExportError> {
    parse_optional_u64(value).map(|value| value.map(FactId))
}
