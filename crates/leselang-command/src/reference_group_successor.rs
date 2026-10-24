//! Product group-successor assembly around the caller's request builder.

use leselang_hir::{Effect, host_call::HostOperation};
use leselang_runtime_core::Fault;

use crate::reference_continuation::validate_continuation_size;
use crate::reference_merge::invalid_group_binding as invalid;
use crate::reference_request::validate_effect_request;
use crate::{ContinuationToken, EffectRequest, GroupResultBinding, ResultBinding};

/// Original admitted group context. These public fields are observations, not
/// proof of reserved identities, current authority or a complete group journal.
pub struct GroupSuccessorContext<'a> {
    pub owner: &'a ContinuationToken,
    pub first: &'a EffectRequest,
    pub fuel_remaining: u64,
    pub position: usize,
}

pub use crate::reference_request_build::ReservedEffectInput;

/// Assemble a preflighted group's successor without evaluating, allocating IDs,
/// persisting or dispatching it. The caller bounds/adopts the original group and
/// requests, proves its reservations and retains live authority and pair checks.
///
/// Atomic operation selection precedes reserved-position lookup. Only then is
/// the original builder called once with the remaining fuel, inherited authority,
/// revision and deadline. That trusted builder owns the existing charge, identity
/// formatting, lowering and static admission; the caller owns reservations. The
/// builder must not dispatch. The
/// adapter does not reconstruct, refund or supplement that budget, authenticate
/// a substitute builder, retry failures or roll back native callback side effects.
pub fn build_group_successor(
    group: &GroupResultBinding,
    effect: &Effect,
    binding: Option<Box<ResultBinding>>,
    context: GroupSuccessorContext<'_>,
    build: impl FnOnce(ReservedEffectInput<'_>) -> Result<EffectRequest, Fault>,
) -> Result<EffectRequest, Fault> {
    let operation = HostOperation::for_effect(effect).ok_or_else(invalid)?;
    let (principal, capabilities) = context.first.operation.authority();
    let image = &context.first.continuation;
    let mut request = build(ReservedEffectInput {
        effect,
        result_type: operation.result_type(),
        sequence: group
            .reserved_sequences()
            .nth(context.position)
            .ok_or_else(invalid)?,
        principal: principal.clone(),
        capabilities: capabilities.clone(),
        expected_revision: image.expected_revision,
        deadline_ms: image.deadline_ms,
        deadline_at_ms: image.deadline_at_ms,
        fuel_remaining: context.fuel_remaining,
    })?;
    request.continuation.schema_version = group.continuation_schema();
    request.continuation.group_result = Some(context.owner.clone());
    request.continuation.result_binding = binding;
    request.continuation.max_output_items = image.max_output_items;
    request.budget.max_output_items = image.max_output_items;
    validate_effect_request(&request)?;
    validate_continuation_size(&request.continuation)?;
    Ok(request)
}
