//! Product operation construction without continuation or dispatch ownership.

use std::fmt;

use leselang_hir::{CAPABILITY_UI_PRESENTATION, Effect};
use leselang_host_contract::{CapabilitySet, CommandOrigin, Confirmation, Principal, Revision};
use leserpent_domain::{CommandId, DOMAIN_SCHEMA_VERSION, IdempotencyKey};

use crate::{
    EffectOperation, LoweringContext, LoweringError, PlannedOperation, PresentationEnvelope,
    PresentationOperation, lower_effect,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OperationLoweringError {
    Identity,
    Lowering(LoweringError),
}

impl fmt::Display for OperationLoweringError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Identity => "host operation identity generation failed",
            Self::Lowering(_) => "host operation lowering failed",
        })
    }
}
impl std::error::Error for OperationLoweringError {}

/// Construct the released product operation from an already admitted effect.
///
/// GUI requests move the original authority into their envelope and never invoke
/// the identity factory. Other effects invoke it exactly once before control-plane
/// lowering, including queries and unsupported structured effects. The factory
/// supplies identities from the caller's original execution, not received input;
/// None refuses construction without lowering. No identity prefix or allocator
/// lives here. Unused/consumed factory captures release before result handoff.
///
/// Control-plane lowering retains its existing validation and copies; it stamps
/// Leselang origin, non-dry-run execution and the operation's confirmation policy.
/// Confirmation is protocol metadata, not proof of user consent. GUI construction
/// alone does not validate source, principal or capabilities. Callers must still
/// validate the complete request, original/live authority and resource budgets.
/// No fuel, timer, execution, journal, replay protection or receipt is created.
/// Native factory work/Drop are trusted; unwind propagates without rollback.
pub fn lower_effect_operation(
    effect: &Effect,
    principal: Principal,
    capabilities: CapabilitySet,
    expected_revision: Option<Revision>,
    command_identity: impl FnOnce() -> Option<(CommandId, IdempotencyKey)>,
) -> Result<(String, EffectOperation), OperationLoweringError> {
    if let Some(operation) = PresentationOperation::from_effect(effect) {
        return Ok((
            CAPABILITY_UI_PRESENTATION.to_string(),
            EffectOperation::Presentation(PresentationEnvelope {
                schema_version: DOMAIN_SCHEMA_VERSION,
                principal,
                capabilities,
                operation,
            }),
        ));
    }
    let (command_id, idempotency_key) =
        command_identity().ok_or(OperationLoweringError::Identity)?;
    let plan = lower_effect(
        effect,
        &LoweringContext {
            principal,
            capabilities,
            expected_revision,
            command_id,
            idempotency_key,
            origin: CommandOrigin::Leselang,
            confirmation: if matches!(
                effect,
                Effect::RuntimeDeploy { .. } | Effect::DebuggerCancel { .. }
            ) {
                Confirmation::Confirmed
            } else {
                Confirmation::NotRequired
            },
            dry_run: false,
        },
    )
    .map_err(OperationLoweringError::Lowering)?;
    let operation = match plan.operation {
        PlannedOperation::Query(query) => EffectOperation::Query(query),
        PlannedOperation::Command(command) => EffectOperation::Command(command),
    };
    Ok((plan.required_capability, operation))
}
