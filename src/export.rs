use crate::flow::{FlowSnapshot, ModuleFinding, ProgramFinding, ProgramFlow};
use crate::fragment::{AttachPlan, AttachReport, BindingDiagnostics, EvidenceTier};
use crate::ledger::{FactEnvelope, FactKindTag};
use crate::reason::{ReasonChain, ReasonProfile};
use crate::runtime::{
    RejectedFact, RuntimeError, RuntimeSession, SessionConfig, summarize_rejected_facts,
};
use crate::template::{
    FragmentParamValue, Template, WindowProfile, default_program_model_for_reason_profile,
};
use std::collections::BTreeMap;

mod attach_codec;
mod fact_codec;
mod json;
mod program_codec;
mod protocol_ir;
mod reason_codec;

use self::attach_codec::{
    attach_failure_summary_json, attach_plan_json, attach_report_json, binding_diagnostics_json,
    debug_summary_json, parse_attach_plan, parse_attach_report, parse_binding_diagnostics,
    parse_debug_summary,
};
use self::fact_codec::{
    fact_json, parse_fact, parse_rejected_fact, parse_rejected_fact_summary, rejected_fact_json,
    rejected_fact_summary_json,
};
use self::json::{JsonParser, JsonValue};
use self::program_codec::{
    flow_json, module_finding_json, parse_flow, parse_module_finding, parse_program_finding,
    parse_program_flow, program_finding_json, program_flow_json,
};
pub use self::protocol_ir::ProtocolIr;
pub(crate) use self::protocol_ir::infer_protocol_ir;
use self::protocol_ir::{parse_protocol_ir, protocol_ir_json};
use self::reason_codec::{parse_reason, parse_reason_profile, reason_json, reason_profile_json};

#[derive(Clone, Debug, PartialEq)]
pub struct FragmentInventoryItem {
    pub id: String,
    pub version: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ExportBundle {
    pub template_id: String,
    pub ingest_trust_mode: String,
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
    pub protocol_ir: Vec<ProtocolIr>,
    pub program_findings: Vec<ProgramFinding>,
    pub module_findings: Vec<ModuleFinding>,
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
    pub program_findings: u64,
    pub module_findings: u64,
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
            id: self.template_id.clone(),
            fragment_set: self
                .fragment_inventory
                .iter()
                .map(|item| item.id.clone())
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
        let mut replay = session.into_export_bundle();
        replay.ingest_trust_mode = self.ingest_trust_mode.clone();
        replay.binding_diagnostics = self.binding_diagnostics.clone();
        replay.attach_failure_summary = self.attach_failure_summary.clone();
        replay.debug_summary = self.debug_summary.clone();
        replay.rejected_fact_summary = summarize_rejected_facts(&replay.rejected_facts);
        replay.program_flows = self.program_flows.clone();
        replay.protocol_ir = self.protocol_ir.clone();
        replay.program_findings = self.program_findings.clone();
        replay.module_findings = self.module_findings.clone();
        Ok(replay)
    }

    pub fn to_json(&self) -> String {
        let root = JsonValue::Object(BTreeMap::from([
            (
                "template_id".into(),
                JsonValue::String(self.template_id.clone()),
            ),
            (
                "ingest_trust_mode".into(),
                JsonValue::String(self.ingest_trust_mode.clone()),
            ),
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
            (
                "attach_report".into(),
                attach_report_json(&self.attach_report),
            ),
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
            (
                "debug_summary".into(),
                debug_summary_json(&self.debug_summary),
            ),
            (
                "window_profile".into(),
                JsonValue::Object(BTreeMap::from([
                    (
                        "id".into(),
                        JsonValue::String(self.window_profile.id.clone()),
                    ),
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
                "protocol_ir".into(),
                JsonValue::Array(self.protocol_ir.iter().map(protocol_ir_json).collect()),
            ),
            (
                "program_findings".into(),
                JsonValue::Array(
                    self.program_findings
                        .iter()
                        .map(program_finding_json)
                        .collect(),
                ),
            ),
            (
                "module_findings".into(),
                JsonValue::Array(
                    self.module_findings
                        .iter()
                        .map(module_finding_json)
                        .collect(),
                ),
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
            ingest_trust_mode: root
                .get("ingest_trust_mode")
                .map(|value| value.as_str())
                .transpose()?
                .unwrap_or("unspecified")
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
                root.get("binding_diagnostics").ok_or_else(|| {
                    ExportError::InvalidShape("missing binding_diagnostics".into())
                })?,
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
            protocol_ir: root
                .get("protocol_ir")
                .unwrap_or(&JsonValue::Array(vec![]))
                .as_array()?
                .iter()
                .map(parse_protocol_ir)
                .collect::<Result<Vec<_>, _>>()?,
            program_findings: root
                .get("program_findings")
                .ok_or_else(|| ExportError::InvalidShape("missing program_findings".into()))?
                .as_array()?
                .iter()
                .map(parse_program_finding)
                .collect::<Result<Vec<_>, _>>()?,
            module_findings: root
                .get("module_findings")
                .ok_or_else(|| ExportError::InvalidShape("missing module_findings".into()))?
                .as_array()?
                .iter()
                .map(parse_module_finding)
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
    JsonValue::Object(BTreeMap::from_iter(fragment_params.iter().map(
        |(fragment_id, params)| {
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
        },
    )))
}

fn evidence_overrides_json(evidence_overrides: &BTreeMap<FactKindTag, EvidenceTier>) -> JsonValue {
    JsonValue::Object(BTreeMap::from_iter(evidence_overrides.iter().map(
        |(fact_kind, tier)| {
            (
                fact_kind.to_string(),
                JsonValue::String(match tier {
                    EvidenceTier::CoreRequirement => "core_requirement".into(),
                    EvidenceTier::OptionalEnhancement => "optional_enhancement".into(),
                }),
            )
        },
    )))
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
                            )));
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
                    )));
                }
            };
            Ok((fact_kind, tier))
        })
        .collect()
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
        id: object
            .get("id")
            .ok_or_else(|| ExportError::InvalidShape("window_profile.id".into()))?
            .as_str()?
            .to_string(),
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

fn parse_attach_failure_summary(
    value: &JsonValue,
) -> Result<AttachFailureSummaryItem, ExportError> {
    let object = value.as_object()?;
    Ok(AttachFailureSummaryItem {
        hookpoint_kind: object
            .get("hookpoint_kind")
            .ok_or_else(|| {
                ExportError::InvalidShape("attach_failure_summary.hookpoint_kind".into())
            })?
            .as_str()?
            .to_string(),
        count: object
            .get("count")
            .ok_or_else(|| ExportError::InvalidShape("attach_failure_summary.count".into()))?
            .as_i64()? as u64,
    })
}
