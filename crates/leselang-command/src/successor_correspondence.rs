//! Product successor correspondence, not request admission or durable settlement.

use crate::{
    CaptureReplayError, EffectRequestView, ResultBindingLimits, Value,
    continuation_schema::successor_schema_matches, validate_result_binding,
    validate_result_binding_extension,
};

/// Closed categories preserve the caller's existing fault mapping without payloads.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SuccessorCorrespondenceError {
    Mismatch,
    InvalidBinding,
}

impl std::fmt::Display for SuccessorCorrespondenceError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Mismatch => "invalid successor correspondence",
            Self::InvalidBinding => "invalid successor binding or capture projection",
        })
    }
}

impl std::error::Error for SuccessorCorrespondenceError {}

/// Check schema/body, inherited context, decreasing fuel, increasing identity,
/// next-then-prior final types and finally preserved named captures, in that order.
///
/// Callers must bound ingress, fully admit both original requests/frames and admit
/// the parent's non-scalar completed value before entry. Borrowed views and native
/// token projections must observe one immutable admitted snapshot. Token text must
/// identify the original group owners, and the sequence hook must recognize canonical,
/// bounded, nonzero identities in the same native domain. Hooks are trusted: early
/// refusal skips later hooks, and side effects/unwind are not retried or rolled back.
/// Name validation and capture/type limits remain explicit caller policy.
///
/// Equal forged context is not authority. This does not authenticate a completion,
/// revalidate raw value bounds, allocate an identity, debit fuel, run saved code or
/// authorize replay. Live grants, budgets, leases and atomic journal publication
/// remain native responsibilities. Inputs are borrowed, never cloned or replaced.
pub fn validate_preflighted_successor_pair<Token>(
    parent: EffectRequestView<'_, Token>,
    next: EffectRequestView<'_, Token>,
    value: &Value,
    limits: ResultBindingLimits,
    token_text: impl Fn(&Token) -> &str,
    canonical_sequence: impl Fn(&Token) -> Option<u64>,
    mut validate_group_name: impl FnMut(&str) -> bool,
) -> Result<(), SuccessorCorrespondenceError> {
    use SuccessorCorrespondenceError::{InvalidBinding, Mismatch};

    let first = parent.continuation;
    let second = next.continuation;
    let schema_valid = successor_schema_matches(
        first.schema_version,
        first.group_result.as_ref().map(&token_text),
        second.schema_version,
        second.group_result.as_ref().map(&token_text),
    );
    if !schema_valid
        || first
            .result_binding
            .as_ref()
            .is_none_or(|binding| binding.body.is_pure())
        || !parent.has_same_inherited_context(&next)
        || first.fuel_remaining <= second.fuel_remaining
        || canonical_sequence(&first.token) >= canonical_sequence(&second.token)
    {
        return Err(Mismatch);
    }
    let prior = first.result_binding.as_ref().ok_or(Mismatch)?;
    let next_type = match &second.result_binding {
        Some(binding) => validate_result_binding(
            binding,
            &second.pending_effect,
            limits,
            &mut validate_group_name,
        )
        .map_err(|_| InvalidBinding)?,
        None => second.result_type,
    };
    if validate_result_binding(
        prior,
        &first.pending_effect,
        limits,
        &mut validate_group_name,
    )
    .map_err(|_| InvalidBinding)?
        != next_type
    {
        return Err(Mismatch);
    }
    if let Some(binding) = &second.result_binding {
        validate_result_binding_extension(
            prior,
            binding,
            &first.pending_effect,
            value,
            limits.captures.max_saved_bindings,
        )
        .map_err(|error| match error {
            CaptureReplayError::InvalidProjection => InvalidBinding,
            CaptureReplayError::Mismatch => Mismatch,
        })?;
    }
    Ok(())
}
