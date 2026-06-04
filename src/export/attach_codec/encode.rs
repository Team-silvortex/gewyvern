use super::{
    AttachFailureSummaryItem, AttachPlan, AttachReport, BindingDiagnostics, CapabilityFlag,
    CoverageReport, DebugSummary, EvidenceTier, JsonValue, MapKind, ModelDiagnostics,
    RuleDiagnostics, RuleTier,
};
use std::collections::BTreeMap;

pub(crate) fn attach_plan_json(plan: &AttachPlan) -> JsonValue {
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
                                                    JsonValue::String(
                                                        match spec.tier {
                                                            EvidenceTier::CoreRequirement => {
                                                                "core_requirement"
                                                            }
                                                            EvidenceTier::OptionalEnhancement => {
                                                                "optional_enhancement"
                                                            }
                                                        }
                                                        .into(),
                                                    ),
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
                                                    JsonValue::String(
                                                        match map.kind {
                                                            MapKind::RingBuf => "ringbuf",
                                                            MapKind::Hash => "hash",
                                                            MapKind::LruHash => "lru_hash",
                                                        }
                                                        .into(),
                                                    ),
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
                                            JsonValue::String(
                                                match cap {
                                                    CapabilityFlag::TcpState => "tcp_state",
                                                    CapabilityFlag::PacketMeta => "packet_meta",
                                                    CapabilityFlag::RouteMeta => "route_meta",
                                                    CapabilityFlag::SockLineage => "sock_lineage",
                                                }
                                                .into(),
                                            )
                                        })
                                        .collect(),
                                ),
                            ),
                            (
                                "sampled_payload_offsets".into(),
                                JsonValue::Array(
                                    fragment
                                        .sampled_payload_offsets
                                        .iter()
                                        .map(|offset| JsonValue::Number(*offset as i64))
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
                                                    JsonValue::String(
                                                        match param.value_type {
                                                            super::FragmentParamType::Bool => {
                                                                "bool"
                                                            }
                                                            super::FragmentParamType::U64 => "u64",
                                                            super::FragmentParamType::String => {
                                                                "string"
                                                            }
                                                        }
                                                        .into(),
                                                    ),
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
                            (
                                "fragment_id".into(),
                                JsonValue::String(binding.fragment_id.into()),
                            ),
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
                            (
                                "fragment_id".into(),
                                JsonValue::String(binding.fragment_id.into()),
                            ),
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
                            (
                                "fragment_id".into(),
                                JsonValue::String(edge.fragment_id.into()),
                            ),
                            (
                                "depends_on".into(),
                                JsonValue::String(edge.depends_on.into()),
                            ),
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

pub(crate) fn attach_report_json(report: &AttachReport) -> JsonValue {
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
                (
                    "maps".into(),
                    JsonValue::Number(report.ringbuf_stats.maps as i64),
                ),
                (
                    "total_max_entries".into(),
                    JsonValue::Number(report.ringbuf_stats.total_max_entries as i64),
                ),
            ])),
        ),
    ]))
}

pub(crate) fn binding_diagnostics_json(diagnostics: &BindingDiagnostics) -> JsonValue {
    JsonValue::Object(BTreeMap::from([
        (
            "program_model".into(),
            diagnostics
                .program_model
                .as_ref()
                .map_or(JsonValue::Null, model_diagnostics_json),
        ),
        (
            "reason_model".into(),
            diagnostics
                .reason_model
                .as_ref()
                .map_or(JsonValue::Null, model_diagnostics_json),
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
        (
            "rule_index".into(),
            JsonValue::Number(rule.rule_index as i64),
        ),
        (
            "tier".into(),
            JsonValue::String(
                match rule.tier {
                    RuleTier::CoreRequirement => "core_requirement",
                    RuleTier::OptionalEnhancement => "optional_enhancement",
                    RuleTier::Unsupported => "unsupported",
                }
                .into(),
            ),
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
        (
            "unsupported_payload_offsets".into(),
            JsonValue::Array(
                rule.unsupported_payload_offsets
                    .iter()
                    .map(|offset| JsonValue::Number(*offset as i64))
                    .collect(),
            ),
        ),
        ("supported".into(), JsonValue::Bool(rule.supported)),
    ]))
}

pub(crate) fn attach_failure_summary_json(summary: &AttachFailureSummaryItem) -> JsonValue {
    JsonValue::Object(BTreeMap::from([
        (
            "hookpoint_kind".into(),
            JsonValue::String(summary.hookpoint_kind.clone()),
        ),
        ("count".into(), JsonValue::Number(summary.count as i64)),
    ]))
}

pub(crate) fn debug_summary_json(summary: &DebugSummary) -> JsonValue {
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
        (
            "program_findings".into(),
            JsonValue::Number(summary.program_findings as i64),
        ),
        (
            "module_findings".into(),
            JsonValue::Number(summary.module_findings as i64),
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
