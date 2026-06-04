use super::{ExportError, JsonValue};
use crate::flow::{
    EvidenceIndex, FlowId, FlowLifecycleView, FlowSnapshot, ModuleFinding, ModuleSeverity,
    PathSegment, PathView, ProcessView, ProgramFinding, ProgramFindingCause, ProgramFlow,
    ProgramFlowId, ProgramOperation, ProgramStage,
};
use crate::ir::SignalKind;
use crate::ledger::FactId;

mod decode;
mod encode;

pub(super) use self::decode::{
    parse_flow, parse_module_finding, parse_program_finding, parse_program_flow,
};
pub(super) use self::encode::{
    flow_json, module_finding_json, program_finding_json, program_flow_json,
};

fn program_operation_id(operation: &ProgramOperation) -> &str {
    match operation {
        ProgramOperation::ConnectFlow => "connect_flow",
        ProgramOperation::DatagramExchange => "datagram_exchange",
        ProgramOperation::Custom(id) => id.as_str(),
        ProgramOperation::Unknown => "unknown",
    }
}

fn parse_program_operation(
    value: &JsonValue,
    field: &str,
) -> Result<ProgramOperation, ExportError> {
    match value.as_str()? {
        "connect_flow" => Ok(ProgramOperation::ConnectFlow),
        "datagram_exchange" => Ok(ProgramOperation::DatagramExchange),
        "unknown" => Ok(ProgramOperation::Unknown),
        other => {
            if other.is_empty() {
                Err(ExportError::InvalidShape(field.into()))
            } else {
                Ok(ProgramOperation::Custom(other.into()))
            }
        }
    }
}

fn parse_program_finding_cause(
    value: &JsonValue,
    field: &str,
) -> Result<ProgramFindingCause, ExportError> {
    match value.as_str()? {
        "attach_failure" => Ok(ProgramFindingCause::AttachFailure),
        "rejected_evidence" => Ok(ProgramFindingCause::RejectedEvidence),
        "missing_core_stage" => Ok(ProgramFindingCause::MissingCoreStage),
        other => Err(ExportError::InvalidValue(format!(
            "unknown {field}: {other}"
        ))),
    }
}

fn parse_module_severity(value: &JsonValue) -> Result<ModuleSeverity, ExportError> {
    match value.as_str()? {
        "high" => Ok(ModuleSeverity::High),
        "medium" => Ok(ModuleSeverity::Medium),
        "low" => Ok(ModuleSeverity::Low),
        other => Err(ExportError::InvalidValue(format!(
            "unknown module severity: {other}"
        ))),
    }
}

fn parse_stage_kind(value: &JsonValue) -> Result<SignalKind, ExportError> {
    let id = value.as_str()?;
    SignalKind::from_id(id)
        .ok_or_else(|| ExportError::InvalidValue(format!("unknown program flow stage kind: {id}")))
}
