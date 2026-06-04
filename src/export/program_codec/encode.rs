use super::{
    FlowSnapshot, JsonValue, ModuleFinding, ModuleSeverity, ProcessView, ProgramFinding,
    ProgramFindingCause, ProgramFlow, ProgramStage, program_operation_id,
};
use crate::export::reason_codec::fact_id_array;
use std::collections::BTreeMap;

pub(crate) fn flow_json(flow: &FlowSnapshot) -> JsonValue {
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
                    flow.lifecycle
                        .tcp_state_now
                        .map_or(JsonValue::Null, |v| JsonValue::Number(v as i64)),
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
                    flow.path
                        .current_oif
                        .map_or(JsonValue::Null, |v| JsonValue::Number(v as i64)),
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
                                        segment.oif.map_or(JsonValue::Null, |v| {
                                            JsonValue::Number(v as i64)
                                        }),
                                    ),
                                ]))
                            })
                            .collect(),
                    ),
                ),
            ])),
        ),
        ("process".into(), process_view_json(flow.process.as_ref())),
        (
            "evidence".into(),
            JsonValue::Object(BTreeMap::from([
                (
                    "tcp_state_facts".into(),
                    fact_id_array(&flow.evidence.tcp_state_facts),
                ),
                (
                    "packet_facts".into(),
                    fact_id_array(&flow.evidence.packet_facts),
                ),
                (
                    "quic_facts".into(),
                    fact_id_array(&flow.evidence.quic_facts),
                ),
                (
                    "route_facts".into(),
                    fact_id_array(&flow.evidence.route_facts),
                ),
                (
                    "lineage_facts".into(),
                    fact_id_array(&flow.evidence.lineage_facts),
                ),
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

pub(crate) fn process_view_json(process: Option<&ProcessView>) -> JsonValue {
    process.map_or(JsonValue::Null, |process| {
        JsonValue::Object(BTreeMap::from([
            ("pid".into(), JsonValue::Number(process.pid as i64)),
            ("tid".into(), JsonValue::Number(process.tid as i64)),
            (
                "cgroup_id".into(),
                JsonValue::Number(process.cgroup_id as i64),
            ),
            ("comm".into(), JsonValue::String(process.comm.clone())),
        ]))
    })
}

pub(crate) fn program_flow_json(flow: &ProgramFlow) -> JsonValue {
    JsonValue::Object(BTreeMap::from([
        ("id".into(), JsonValue::Number(flow.id.0 as i64)),
        ("process".into(), process_view_json(flow.process.as_ref())),
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
            JsonValue::Array(flow.stages.iter().map(program_stage_json).collect()),
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

fn program_stage_json(stage: &ProgramStage) -> JsonValue {
    let mut object = BTreeMap::from([
        ("at".into(), JsonValue::Number(stage.at.0 as i64)),
        ("kind".into(), JsonValue::String(stage.kind.id().into())),
    ]);
    object.insert(
        "phase".into(),
        stage
            .phase
            .as_ref()
            .map_or(JsonValue::Null, |phase| JsonValue::String(phase.clone())),
    );
    object.insert(
        "phase_kind".into(),
        stage
            .phase_kind
            .as_ref()
            .map_or(JsonValue::Null, |phase_kind| {
                JsonValue::String(phase_kind.clone())
            }),
    );
    JsonValue::Object(object)
}

pub(crate) fn program_finding_json(finding: &ProgramFinding) -> JsonValue {
    JsonValue::Object(BTreeMap::from([
        (
            "program_flow".into(),
            JsonValue::Number(finding.program_flow.0 as i64),
        ),
        (
            "process".into(),
            process_view_json(finding.process.as_ref()),
        ),
        (
            "operation".into(),
            JsonValue::String(program_operation_id(&finding.operation).into()),
        ),
        (
            "module_label".into(),
            JsonValue::String(finding.module_label.clone()),
        ),
        (
            "network_module_kind".into(),
            JsonValue::String(finding.network_module_kind.clone()),
        ),
        (
            "phase".into(),
            finding
                .phase
                .as_ref()
                .map_or(JsonValue::Null, |phase| JsonValue::String(phase.clone())),
        ),
        (
            "phase_kind".into(),
            finding
                .phase_kind
                .as_ref()
                .map_or(JsonValue::Null, |phase_kind| {
                    JsonValue::String(phase_kind.clone())
                }),
        ),
        (
            "phase_transition".into(),
            finding
                .phase_transition
                .as_ref()
                .map_or(JsonValue::Null, |transition| {
                    JsonValue::String(transition.clone())
                }),
        ),
        (
            "phase_transition_kind".into(),
            finding
                .phase_transition_kind
                .as_ref()
                .map_or(JsonValue::Null, |kind| JsonValue::String(kind.clone())),
        ),
        (
            "suspect_area".into(),
            JsonValue::String(finding.suspect_area.clone()),
        ),
        (
            "cause".into(),
            JsonValue::String(match finding.cause {
                ProgramFindingCause::AttachFailure => "attach_failure".into(),
                ProgramFindingCause::RejectedEvidence => "rejected_evidence".into(),
                ProgramFindingCause::MissingCoreStage => "missing_core_stage".into(),
            }),
        ),
        ("summary".into(), JsonValue::String(finding.summary.clone())),
        (
            "supporting_fragments".into(),
            JsonValue::Array(
                finding
                    .supporting_fragments
                    .iter()
                    .map(|fragment| JsonValue::String(fragment.clone()))
                    .collect(),
            ),
        ),
        (
            "evidence_trace".into(),
            JsonValue::Array(
                finding
                    .evidence_trace
                    .iter()
                    .map(|item| JsonValue::String(item.clone()))
                    .collect(),
            ),
        ),
    ]))
}

pub(crate) fn module_finding_json(finding: &ModuleFinding) -> JsonValue {
    JsonValue::Object(BTreeMap::from([
        (
            "module_label".into(),
            JsonValue::String(finding.module_label.clone()),
        ),
        (
            "process".into(),
            process_view_json(finding.process.as_ref()),
        ),
        (
            "operation".into(),
            JsonValue::String(program_operation_id(&finding.operation).into()),
        ),
        (
            "severity".into(),
            JsonValue::String(match finding.severity {
                ModuleSeverity::High => "high".into(),
                ModuleSeverity::Medium => "medium".into(),
                ModuleSeverity::Low => "low".into(),
            }),
        ),
        (
            "network_module_kinds".into(),
            JsonValue::Array(
                finding
                    .network_module_kinds
                    .iter()
                    .map(|kind| JsonValue::String(kind.clone()))
                    .collect(),
            ),
        ),
        (
            "phases".into(),
            JsonValue::Array(
                finding
                    .phases
                    .iter()
                    .map(|phase| JsonValue::String(phase.clone()))
                    .collect(),
            ),
        ),
        (
            "phase_kinds".into(),
            JsonValue::Array(
                finding
                    .phase_kinds
                    .iter()
                    .map(|kind| JsonValue::String(kind.clone()))
                    .collect(),
            ),
        ),
        (
            "phase_transitions".into(),
            JsonValue::Array(
                finding
                    .phase_transitions
                    .iter()
                    .map(|transition| JsonValue::String(transition.clone()))
                    .collect(),
            ),
        ),
        (
            "phase_transition_kinds".into(),
            JsonValue::Array(
                finding
                    .phase_transition_kinds
                    .iter()
                    .map(|kind| JsonValue::String(kind.clone()))
                    .collect(),
            ),
        ),
        (
            "suspect_areas".into(),
            JsonValue::Array(
                finding
                    .suspect_areas
                    .iter()
                    .map(|area| JsonValue::String(area.clone()))
                    .collect(),
            ),
        ),
        (
            "causes".into(),
            JsonValue::Array(
                finding
                    .causes
                    .iter()
                    .map(|cause| {
                        JsonValue::String(match cause {
                            ProgramFindingCause::AttachFailure => "attach_failure".into(),
                            ProgramFindingCause::RejectedEvidence => "rejected_evidence".into(),
                            ProgramFindingCause::MissingCoreStage => "missing_core_stage".into(),
                        })
                    })
                    .collect(),
            ),
        ),
        (
            "supporting_fragments".into(),
            JsonValue::Array(
                finding
                    .supporting_fragments
                    .iter()
                    .map(|fragment| JsonValue::String(fragment.clone()))
                    .collect(),
            ),
        ),
        (
            "program_flows".into(),
            JsonValue::Array(
                finding
                    .program_flows
                    .iter()
                    .map(|id| JsonValue::Number(id.0 as i64))
                    .collect(),
            ),
        ),
        (
            "summaries".into(),
            JsonValue::Array(
                finding
                    .summaries
                    .iter()
                    .map(|summary| JsonValue::String(summary.clone()))
                    .collect(),
            ),
        ),
        (
            "evidence_trace".into(),
            JsonValue::Array(
                finding
                    .evidence_trace
                    .iter()
                    .map(|item| JsonValue::String(item.clone()))
                    .collect(),
            ),
        ),
    ]))
}
