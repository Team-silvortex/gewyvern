use super::{AttachFailureSummaryItem, DebugSummary, ExportError, JsonValue};
use crate::fragment::{
    AttachPlan, AttachReport, BindingDiagnostics, CapabilityFlag, CoverageReport, DependencyEdge,
    EvidenceClassSpec, EvidenceTier, FactBinding, FragmentDescriptor, FragmentParamSpec,
    FragmentParamType, HookBinding, HookPoint, MapKind, MapSpec, ModelDiagnostics, RingBufStats,
    RuleDiagnostics, RuleTier,
};
use crate::ledger::FactKindTag;

mod decode;
mod encode;

pub(super) use self::decode::{
    parse_attach_plan, parse_attach_report, parse_binding_diagnostics, parse_debug_summary,
};
pub(super) use self::encode::{
    attach_failure_summary_json, attach_plan_json, attach_report_json, binding_diagnostics_json,
    debug_summary_json,
};

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

fn parse_hookpoint(input: &str) -> Result<HookPoint, ExportError> {
    if let Some(value) = input.strip_prefix("tracepoint:") {
        return Ok(HookPoint::TracePoint(Box::leak(
            value.to_string().into_boxed_str(),
        )));
    }
    if let Some(value) = input.strip_prefix("kprobe:") {
        return Ok(HookPoint::KProbe(Box::leak(
            value.to_string().into_boxed_str(),
        )));
    }
    match input {
        "tc:ingress" => Ok(HookPoint::TCIngress),
        "tc:egress" => Ok(HookPoint::TCEgress),
        _ => Err(ExportError::InvalidValue("unknown hookpoint".into())),
    }
}
