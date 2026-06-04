use super::super::fact_codec::{
    parse_fact_ids, parse_optional_fact_id, parse_optional_u8, parse_optional_u32,
};
use super::{
    EvidenceIndex, ExportError, FactId, FlowId, FlowLifecycleView, FlowSnapshot, JsonValue,
    ModuleFinding, PathSegment, PathView, ProcessView, ProgramFinding, ProgramFlow, ProgramFlowId,
    ProgramStage, parse_module_severity, parse_program_finding_cause, parse_program_operation,
    parse_stage_kind,
};

pub(crate) fn parse_flow(value: &JsonValue) -> Result<FlowSnapshot, ExportError> {
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
            tcp_state_now: parse_optional_u8(
                lifecycle.get("tcp_state_now").unwrap_or(&JsonValue::Null),
            )?,
            terminated: lifecycle
                .get("terminated")
                .ok_or_else(|| ExportError::InvalidShape("flow.lifecycle.terminated".into()))?
                .as_bool()?,
            termination_fact: parse_optional_fact_id(
                lifecycle
                    .get("termination_fact")
                    .unwrap_or(&JsonValue::Null),
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
                evidence
                    .get("tcp_state_facts")
                    .unwrap_or(&JsonValue::Array(vec![])),
            )?,
            packet_facts: parse_fact_ids(
                evidence
                    .get("packet_facts")
                    .unwrap_or(&JsonValue::Array(vec![])),
            )?,
            quic_facts: parse_fact_ids(
                evidence
                    .get("quic_facts")
                    .unwrap_or(&JsonValue::Array(vec![])),
            )?,
            route_facts: parse_fact_ids(
                evidence
                    .get("route_facts")
                    .unwrap_or(&JsonValue::Array(vec![])),
            )?,
            lineage_facts: parse_fact_ids(
                evidence
                    .get("lineage_facts")
                    .unwrap_or(&JsonValue::Array(vec![])),
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

pub(crate) fn parse_process_view(value: &JsonValue) -> Result<Option<ProcessView>, ExportError> {
    match value {
        JsonValue::Null => Ok(None),
        JsonValue::Object(object) => Ok(Some(ProcessView {
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

pub(crate) fn parse_program_flow(value: &JsonValue) -> Result<ProgramFlow, ExportError> {
    let object = value.as_object()?;
    Ok(ProgramFlow {
        id: ProgramFlowId(
            object
                .get("id")
                .ok_or_else(|| ExportError::InvalidShape("program_flow.id".into()))?
                .as_i64()? as u64,
        ),
        process: parse_process_view(object.get("process").unwrap_or(&JsonValue::Null))?,
        operation: parse_program_operation(
            object
                .get("operation")
                .ok_or_else(|| ExportError::InvalidShape("program_flow.operation".into()))?,
            "program_flow.operation",
        )?,
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
            .map(parse_program_stage)
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

fn parse_program_stage(value: &JsonValue) -> Result<ProgramStage, ExportError> {
    let object = value.as_object()?;
    Ok(ProgramStage {
        at: FactId(
            object
                .get("at")
                .ok_or_else(|| ExportError::InvalidShape("program_flow.stage.at".into()))?
                .as_i64()? as u64,
        ),
        kind: parse_stage_kind(
            object
                .get("kind")
                .ok_or_else(|| ExportError::InvalidShape("program_flow.stage.kind".into()))?,
        )?,
        phase: match object.get("phase").unwrap_or(&JsonValue::Null) {
            JsonValue::Null => None,
            value => Some(value.as_str()?.to_string()),
        },
        phase_kind: match object.get("phase_kind").unwrap_or(&JsonValue::Null) {
            JsonValue::Null => None,
            value => Some(value.as_str()?.to_string()),
        },
    })
}

pub(crate) fn parse_program_finding(value: &JsonValue) -> Result<ProgramFinding, ExportError> {
    let object = value.as_object()?;
    Ok(ProgramFinding {
        program_flow: ProgramFlowId(
            object
                .get("program_flow")
                .ok_or_else(|| ExportError::InvalidShape("program_finding.program_flow".into()))?
                .as_i64()? as u64,
        ),
        process: parse_process_view(object.get("process").unwrap_or(&JsonValue::Null))?,
        operation: parse_program_operation(
            object
                .get("operation")
                .ok_or_else(|| ExportError::InvalidShape("program_finding.operation".into()))?,
            "program_finding.operation",
        )?,
        module_label: object
            .get("module_label")
            .ok_or_else(|| ExportError::InvalidShape("program_finding.module_label".into()))?
            .as_str()?
            .to_string(),
        network_module_kind: object
            .get("network_module_kind")
            .map(|value| value.as_str().map(|v| v.to_string()))
            .transpose()?
            .unwrap_or_else(|| "network_module".to_string()),
        phase: match object.get("phase").unwrap_or(&JsonValue::Null) {
            JsonValue::Null => None,
            value => Some(value.as_str()?.to_string()),
        },
        phase_kind: match object.get("phase_kind").unwrap_or(&JsonValue::Null) {
            JsonValue::Null => None,
            value => Some(value.as_str()?.to_string()),
        },
        phase_transition: match object.get("phase_transition").unwrap_or(&JsonValue::Null) {
            JsonValue::Null => None,
            value => Some(value.as_str()?.to_string()),
        },
        phase_transition_kind: match object
            .get("phase_transition_kind")
            .unwrap_or(&JsonValue::Null)
        {
            JsonValue::Null => None,
            value => Some(value.as_str()?.to_string()),
        },
        suspect_area: object
            .get("suspect_area")
            .ok_or_else(|| ExportError::InvalidShape("program_finding.suspect_area".into()))?
            .as_str()?
            .to_string(),
        cause: parse_program_finding_cause(
            object
                .get("cause")
                .ok_or_else(|| ExportError::InvalidShape("program_finding.cause".into()))?,
            "program finding cause",
        )?,
        summary: object
            .get("summary")
            .ok_or_else(|| ExportError::InvalidShape("program_finding.summary".into()))?
            .as_str()?
            .to_string(),
        supporting_fragments: object
            .get("supporting_fragments")
            .ok_or_else(|| {
                ExportError::InvalidShape("program_finding.supporting_fragments".into())
            })?
            .as_array()?
            .iter()
            .map(|item| Ok(item.as_str()?.to_string()))
            .collect::<Result<Vec<_>, _>>()?,
        evidence_trace: object
            .get("evidence_trace")
            .ok_or_else(|| ExportError::InvalidShape("program_finding.evidence_trace".into()))?
            .as_array()?
            .iter()
            .map(|item| Ok(item.as_str()?.to_string()))
            .collect::<Result<Vec<_>, _>>()?,
    })
}

pub(crate) fn parse_module_finding(value: &JsonValue) -> Result<ModuleFinding, ExportError> {
    let object = value.as_object()?;
    Ok(ModuleFinding {
        module_label: object
            .get("module_label")
            .ok_or_else(|| ExportError::InvalidShape("module_finding.module_label".into()))?
            .as_str()?
            .to_string(),
        process: parse_process_view(object.get("process").unwrap_or(&JsonValue::Null))?,
        operation: parse_program_operation(
            object
                .get("operation")
                .ok_or_else(|| ExportError::InvalidShape("module_finding.operation".into()))?,
            "module_finding.operation",
        )?,
        severity: parse_module_severity(
            object
                .get("severity")
                .ok_or_else(|| ExportError::InvalidShape("module_finding.severity".into()))?,
        )?,
        network_module_kinds: parse_string_array(
            object
                .get("network_module_kinds")
                .unwrap_or(&JsonValue::Array(Vec::new())),
        )?,
        phases: parse_string_array(
            object
                .get("phases")
                .unwrap_or(&JsonValue::Array(Vec::new())),
        )?,
        phase_kinds: parse_string_array(
            object
                .get("phase_kinds")
                .unwrap_or(&JsonValue::Array(Vec::new())),
        )?,
        phase_transitions: parse_string_array(
            object
                .get("phase_transitions")
                .unwrap_or(&JsonValue::Array(Vec::new())),
        )?,
        phase_transition_kinds: parse_string_array(
            object
                .get("phase_transition_kinds")
                .unwrap_or(&JsonValue::Array(Vec::new())),
        )?,
        suspect_areas: parse_string_array(
            object
                .get("suspect_areas")
                .ok_or_else(|| ExportError::InvalidShape("module_finding.suspect_areas".into()))?,
        )?,
        causes: object
            .get("causes")
            .ok_or_else(|| ExportError::InvalidShape("module_finding.causes".into()))?
            .as_array()?
            .iter()
            .map(|item| parse_program_finding_cause(item, "module finding cause"))
            .collect::<Result<Vec<_>, _>>()?,
        supporting_fragments: parse_string_array(object.get("supporting_fragments").ok_or_else(
            || ExportError::InvalidShape("module_finding.supporting_fragments".into()),
        )?)?,
        program_flows: object
            .get("program_flows")
            .ok_or_else(|| ExportError::InvalidShape("module_finding.program_flows".into()))?
            .as_array()?
            .iter()
            .map(|item| Ok(ProgramFlowId(item.as_i64()? as u64)))
            .collect::<Result<Vec<_>, _>>()?,
        summaries: parse_string_array(
            object
                .get("summaries")
                .ok_or_else(|| ExportError::InvalidShape("module_finding.summaries".into()))?,
        )?,
        evidence_trace: parse_string_array(
            object
                .get("evidence_trace")
                .ok_or_else(|| ExportError::InvalidShape("module_finding.evidence_trace".into()))?,
        )?,
    })
}

fn parse_string_array(value: &JsonValue) -> Result<Vec<String>, ExportError> {
    value
        .as_array()?
        .iter()
        .map(|item| Ok(item.as_str()?.to_string()))
        .collect()
}
