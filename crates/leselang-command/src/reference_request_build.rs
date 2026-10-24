//! Product request construction; live reservation and dispatch remain host-owned.

use leselang_hir::{Effect, Type};
use leselang_host_contract::{CapabilitySet, Principal, Revision};
use leselang_runtime_core::{Fault, Fuel};
use leserpent_domain::{CommandId, IdempotencyKey};

use crate::continuation_schema::CONTINUATION_SCHEMA_VERSION;
use crate::reference_request::{DEFAULT_MAX_OUTPUT_ITEMS, validate_effect_request};
use crate::{ContinuationImage, ContinuationToken, EffectRequest};

/// Inputs for a previously reserved effect, not proof of a reservation or dispatch grant.
/// The effect stays borrowed; inherited authority is copied once as before.
pub struct ReservedEffectInput<'a> {
    pub effect: &'a Effect,
    pub result_type: Type,
    pub sequence: u64,
    pub principal: Principal,
    pub capabilities: CapabilitySet,
    pub expected_revision: Option<Revision>,
    pub deadline_ms: u64,
    pub deadline_at_ms: Option<u64>,
    pub fuel_remaining: u64,
}

/// Build a static product request with the original single host-operation charge.
/// The caller must bound/admit the effect before this function clones it and must
/// reserve the sequence in its own live journal. Public fields and a successful
/// return prove neither reservation nor current permission, and do not install,
/// dispatch or settle a continuation. Failed construction does not refund a
/// reservation. Authority, deadlines and remaining fuel are inherited unchanged
/// apart from the one debit, never replenished here.
pub fn build_effect_request(input: ReservedEffectInput<'_>) -> Result<EffectRequest, Fault> {
    let ReservedEffectInput {
        effect,
        result_type,
        sequence,
        principal,
        capabilities,
        expected_revision,
        deadline_ms,
        deadline_at_ms,
        fuel_remaining,
    } = input;
    let mut fuel = Fuel::new(fuel_remaining);
    fuel.charge(1).map_err(|_| Fault {
        code: "LSV1001".into(),
        message: "execution fuel exhausted before host operation".into(),
    })?;
    let image = ContinuationImage {
        schema_version: CONTINUATION_SCHEMA_VERSION,
        token: ContinuationToken::from_wire(format!("continuation-{sequence}")),
        program_counter: 1,
        expected_revision,
        result_type,
        pending_effect: effect.clone(),
        fuel_remaining: fuel.remaining(),
        deadline_ms,
        deadline_at_ms,
        max_output_items: DEFAULT_MAX_OUTPUT_ITEMS,
        result_binding: None,
        group_result: None,
    };
    let (required_capability, operation) =
        crate::lower_effect_operation(effect, principal, capabilities, expected_revision, || {
            Some((
                CommandId::new(format!("leselang-command-{sequence}")).ok()?,
                IdempotencyKey::new(format!("leselang-effect-{sequence}")).ok()?,
            ))
        })
        .map_err(|error| match error {
            crate::OperationLoweringError::Identity => generated_effect_identity_fault(),
            crate::OperationLoweringError::Lowering(error) => Fault {
                code: "LSV1002".to_string(),
                message: format!("structured branch lowering failed: {error:?}"),
            },
        })?;
    let budget = image.resource_budget();
    let request = EffectRequest {
        effect_id: format!("effect-{sequence}"),
        required_capability,
        operation,
        continuation: image,
        budget,
    };
    validate_effect_request(&request)?;
    Ok(request)
}

fn generated_effect_identity_fault() -> Fault {
    Fault {
        code: "LSV1002".to_string(),
        message: "generated effect identity is invalid".to_string(),
    }
}
