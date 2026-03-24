use crate::flow::{EvidenceIndex, FlowId, FlowLifecycleView, FlowSnapshot, PathSegment, PathView};
use crate::fragment::{
    AttachPlan, AttachReport, CapabilityFlag, CoverageReport, DependencyEdge, FactBinding,
    FragmentDescriptor, HookBinding, HookPoint, MapKind, MapSpec, RingBufStats,
};
use crate::ledger::{
    millis_to_system_time, system_time_to_millis, AttachScopeFact, CpuId, DropActionFact,
    DropVerdict, FactEnvelope, FactId, FactKind, FactKindTag, PacketDir, PacketMetaFact,
    RouteDecisionFact, SessionId, SockLineageFact, TcpStateFact,
};
use crate::reason::{
    KeyEvent, KeyEventKind, NarrLine, ReasonChain, ReasonId, ReasonL1, ReasonL3, ReasonProfile,
};
use crate::runtime::{RuntimeError, RuntimeSession, SessionConfig};
use crate::template::{Template, WindowProfile};
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
    pub window_profile: WindowProfile,
    pub reason_profile_id: String,
    pub facts: Vec<FactEnvelope>,
    pub flows: Vec<FlowSnapshot>,
    pub reasons: Vec<ReasonChain>,
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
        let template = Template {
            id: Box::leak(self.template_id.clone().into_boxed_str()),
            fragment_set: self
                .fragment_inventory
                .iter()
                .map(|item| Box::leak(item.id.clone().into_boxed_str()) as &'static str)
                .collect(),
            window_profile: Some(self.window_profile.clone()),
            reason_profile: Some(
                ReasonProfile::from_id(&self.reason_profile_id)
                    .ok_or_else(|| ExportError::InvalidValue("unknown reason profile".into()))?,
            ),
        };

        let config = SessionConfig::for_template(template).map_err(ExportError::Runtime)?;
        let mut session = RuntimeSession::start(config).map_err(ExportError::Runtime)?;
        for fact in &self.facts {
            session.ingest(fact.clone());
        }
        Ok(session.export_bundle())
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
                "facts".into(),
                JsonValue::Array(self.facts.iter().map(fact_json).collect()),
            ),
            (
                "flows".into(),
                JsonValue::Array(self.flows.iter().map(flow_json).collect()),
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
            window_profile: parse_window_profile(
                root.get("window_profile")
                    .ok_or_else(|| ExportError::InvalidShape("missing window_profile".into()))?,
            )?,
            reason_profile_id: root
                .get("reason_profile_id")
                .ok_or_else(|| ExportError::InvalidShape("missing reason_profile_id".into()))?
                .as_str()?
                .to_string(),
            facts: root
                .get("facts")
                .ok_or_else(|| ExportError::InvalidShape("missing facts".into()))?
                .as_array()?
                .iter()
                .map(parse_fact)
                .collect::<Result<Vec<_>, _>>()?,
            flows: root
                .get("flows")
                .ok_or_else(|| ExportError::InvalidShape("missing flows".into()))?
                .as_array()?
                .iter()
                .map(parse_flow)
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
                                            }
                                            .into())
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
                _ => Err(ExportError::InvalidValue("unknown capability".into())),
            })
            .collect::<Result<Vec<_>, _>>()?,
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
            comm: [0; 16],
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
