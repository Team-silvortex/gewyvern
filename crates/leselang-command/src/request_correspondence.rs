//! Product request/frame correspondence after caller-owned frame admission.

use std::fmt;

use leselang_host_contract::Revision;
use leselang_runtime_core::{ContinuationFrame, ResourceBudget};
use leserpent_hir::{Effect, Type};

use crate::{EffectOperation, OperationRequestContext, OperationRequestError, ResultBinding};

/// Borrow original request data without imposing traits on the native token.
/// Construction is unchecked and does not admit the frame or authenticate a request.
pub struct EffectRequestView<'a, Token> {
    pub effect_id: &'a str,
    pub required_capability: &'a str,
    pub operation: &'a EffectOperation,
    pub continuation: &'a ContinuationFrame<Token, Revision, Type, Effect, ResultBinding>,
    pub budget: &'a ResourceBudget,
}

impl<Token> EffectRequestView<'_, Token> {
    /// Compare declared authority, revision and inherited deadline/output policy.
    /// Fuel progression, token identity, request/frame admission and live grants
    /// remain separate checks. Equal forged metadata is not an execution grant.
    pub fn has_same_inherited_context<Other>(&self, saved: &EffectRequestView<'_, Other>) -> bool {
        leselang_runtime_core::validate_inherited_context(
            &self.continuation.resource_budget(),
            &saved.continuation.resource_budget(),
            || {
                Ok::<_, std::convert::Infallible>(
                    self.operation.authority() == saved.operation.authority(),
                )
            },
            || Ok(self.continuation.expected_revision == saved.continuation.expected_revision),
        )
        .is_ok()
    }
}

/// Closed, payload-free correspondence refusals, not transport or recovery errors.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RequestCorrespondenceError {
    SuccessorAuthority,
    ContinuationIdentity,
    EffectIdentity,
    Operation(OperationRequestError),
    Budget,
}

impl fmt::Display for RequestCorrespondenceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::SuccessorAuthority => {
                "invalid result-driven successor or missing original effect authority"
            }
            Self::ContinuationIdentity => "effect request has a non-canonical continuation token",
            Self::EffectIdentity => "effect identifier does not match continuation token",
            Self::Operation(error) => return fmt::Display::fmt(error, formatter),
            Self::Budget => "effect request does not match its continuation",
        })
    }
}

impl std::error::Error for RequestCorrespondenceError {}

/// Check cold successor authority, identity, operation, then exact budget equality.
///
/// Use [`crate::validate_effect_request`] when the frame has not yet been admitted.
///
/// Callers must bound ingress and fully admit the original frame, including the
/// pending effect and captured body, before entry. The trusted sequence hook must
/// recognize canonical, nonzero, bounded identities in the caller's token domain;
/// it borrows the original token exactly once after cold capability checks pass.
/// No normalization or sequence allocation occurs here. Product effect/command/
/// idempotency names are derived from the returned sequence as correspondence
/// expectations only. Hook side effects are not rolled back, unwind propagates
/// without retry, and an uncalled hook is dropped on early refusal.
///
/// This does not validate frame schema/structure, grant current authority, debit
/// fuel, dispatch work or admit durable recovery. A forged mutually consistent
/// pair can pass: provenance, original budgets and replay fences remain external.
pub fn validate_preflighted_effect_request<Token>(
    request: EffectRequestView<'_, Token>,
    canonical_sequence: impl FnOnce(&Token) -> Option<u64>,
) -> Result<(), RequestCorrespondenceError> {
    if let Some(binding) = &request.continuation.result_binding {
        let (_, capabilities) = request.operation.authority();
        if binding
            .body
            .required_capabilities()
            .iter()
            .any(|capability| !capabilities.contains(capability))
        {
            return Err(RequestCorrespondenceError::SuccessorAuthority);
        }
    }
    let sequence = canonical_sequence(&request.continuation.token)
        .ok_or(RequestCorrespondenceError::ContinuationIdentity)?;
    let expected_effect_id = format!("effect-{sequence}");
    let expected_command_id = format!("leselang-command-{sequence}");
    let expected_idempotency_key = format!("leselang-effect-{sequence}");
    if request.effect_id != expected_effect_id {
        return Err(RequestCorrespondenceError::EffectIdentity);
    }
    request
        .operation
        .validate_request(OperationRequestContext {
            effect: &request.continuation.pending_effect,
            required_capability: request.required_capability,
            expected_command_id: &expected_command_id,
            expected_idempotency_key: &expected_idempotency_key,
            expected_revision: request.continuation.expected_revision,
        })
        .map_err(RequestCorrespondenceError::Operation)?;
    if *request.budget != request.continuation.resource_budget() {
        return Err(RequestCorrespondenceError::Budget);
    }
    Ok(())
}
