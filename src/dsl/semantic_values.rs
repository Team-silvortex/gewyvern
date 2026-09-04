use crate::flow::{ProgramOperation, ProgramStageKind};
use crate::template::{WindowProfile, default_5s_window};

use super::DslError;

pub(super) fn parse_window_profile(value: &str) -> Result<WindowProfile, DslError> {
    match value {
        "default_5s" => Ok(default_5s_window()),
        other => Err(DslError::InvalidValue(format!(
            "unknown window profile '{other}'"
        ))),
    }
}

pub(super) fn parse_operation(value: &str) -> ProgramOperation {
    match value {
        "connect_flow" => ProgramOperation::ConnectFlow,
        "datagram_exchange" => ProgramOperation::DatagramExchange,
        "unknown" => ProgramOperation::Unknown,
        other => ProgramOperation::Custom(other.into()),
    }
}

pub(crate) fn parse_stage(value: &str) -> Result<Option<ProgramStageKind>, DslError> {
    Ok(match value {
        "none" => None,
        other => Some(
            crate::ir::SignalKind::from_id(other)
                .ok_or_else(|| DslError::InvalidValue(format!("unknown stage '{other}'")))?,
        ),
    })
}
