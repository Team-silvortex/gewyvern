//! Optional reference-VM mutation bridge. Inspection alone never authorizes execution.

use leselang_command::{
    CommandPlan, ContinuationImage, DebuggerAuditContext, DebuggerCancelResult, encode_continuation,
};
use leselang_ui::{DebuggerProjection, DebuggerState};
use leselang_vm::Vm;
use leserpent_domain::PlannedOperation;

use crate::{
    DebuggerMutationResult, DebuggerMutationStatus, ObserveError, inspect_debugger_cancel_plan,
};

pub fn execute_debugger_cancel(
    plan: &CommandPlan,
    projection: &DebuggerProjection,
    image: &ContinuationImage,
    vm: &mut Vm,
    now_ms: u64,
) -> Result<DebuggerMutationResult, ObserveError> {
    let inspection = inspect_debugger_cancel_plan(plan, projection)?;
    encode_continuation(image).map_err(ObserveError::InvalidDebuggerContinuation)?;
    let effect_id = image
        .token
        .as_str()
        .strip_prefix("continuation-")
        .map(|sequence| format!("effect-{sequence}"));
    if image.program_counter != projection.program_counter
        || image.fuel_remaining != projection.fuel_remaining
        || projection
            .pending_effect
            .as_ref()
            .map(|effect| effect.effect_id.as_str())
            != effect_id.as_deref()
    {
        return Err(ObserveError::DebuggerSessionMismatch);
    }
    if inspection.dry_run {
        return Ok(DebuggerMutationResult {
            inspection,
            status: DebuggerMutationStatus::Planned,
            state: projection.state,
            audited_at_ms: None,
        });
    }

    let PlannedOperation::Command(command) = &plan.operation else {
        return Err(ObserveError::DebuggerSessionMismatch);
    };
    let audit = DebuggerAuditContext {
        command_id: command.command_id.clone(),
        idempotency_key: command.idempotency_key.clone(),
        principal: command.principal.clone(),
        origin: command.origin,
        session_id: inspection.session_id.clone(),
        expected_revision: inspection.expected_revision,
    };
    let record = vm
        .cancel_effect_audited(image, now_ms, &audit)
        .map_err(ObserveError::InvalidDebuggerContinuation)?;
    Ok(DebuggerMutationResult {
        inspection,
        status: DebuggerMutationStatus::Applied,
        state: DebuggerState::Cancelled,
        audited_at_ms: Some(record.observed_at_ms),
    })
}

pub fn execute_debugger_cancel_effect(
    plan: &CommandPlan,
    projection: &DebuggerProjection,
    image: &ContinuationImage,
    vm: &mut Vm,
    now_ms: u64,
) -> Result<DebuggerCancelResult, ObserveError> {
    let mutation = execute_debugger_cancel(plan, projection, image, vm, now_ms)?;
    if mutation.status != DebuggerMutationStatus::Applied {
        return Err(ObserveError::UnexpectedDebuggerStep);
    }
    let PlannedOperation::Command(command) = &plan.operation else {
        return Err(ObserveError::DebuggerSessionMismatch);
    };
    Ok(DebuggerCancelResult {
        command_id: command.command_id.clone(),
        session_id: mutation.inspection.session_id,
        observed_at_ms: mutation
            .audited_at_ms
            .ok_or(ObserveError::UnexpectedDebuggerStep)?,
    })
}
