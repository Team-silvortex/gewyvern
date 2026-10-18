//! Ordered product group-binding admission with caller-owned encoding policy.

use std::fmt;

use leselang_hir::Type;

use crate::{
    GroupPlanLimits, GroupResultBinding, MergePlan, ResultBindingLimits, validate_result_binding,
    validate_result_binding_structure,
};

/// Explicit saved-data limits, without VM defaults or execution grants.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GroupBindingLimits {
    pub plan: GroupPlanLimits,
    pub binding: ResultBindingLimits,
}

/// Native encoding failures move to the caller without formatting their payload.
#[derive(Eq, PartialEq)]
pub enum GroupBindingError<Encoding> {
    Binding,
    Plan,
    Encoding(Encoding),
}

impl<Encoding> fmt::Display for GroupBindingError<Encoding> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Binding => "invalid group result binding",
            Self::Plan => "invalid group plan correspondence",
            Self::Encoding(_) => "group plan encoding rejected",
        })
    }
}

impl<Encoding> fmt::Debug for GroupBindingError<Encoding> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, formatter)
    }
}

impl<Encoding> std::error::Error for GroupBindingError<Encoding> {}

/// Compose structure, correspondence, native encoding, then full type/scalar checks.
///
/// The encoding hook runs exactly once only after both early gates pass, borrowing
/// the original plan. Its error is returned unchanged inside the typed wrapper.
/// Name hooks retain the underlying binding validators' ordered behavior; native
/// callbacks are trusted, their work is not rolled back, and unwind propagates.
/// No input is changed, no effect executes, and no fuel or identity is allocated.
///
/// Callers must bound ingress and pending effects, validate outer plan shape, and
/// select the original binding from that plan before entry. Encoding must be bounded
/// by native policy. Type success does not authenticate frames, requests, receipts
/// or durable state; current authority and recovery fences remain external.
pub fn validate_group_binding<Encoding>(
    group: &GroupResultBinding,
    plan: &MergePlan,
    limits: GroupBindingLimits,
    mut validate_group_name: impl FnMut(&str) -> bool,
    validate_encoding: impl FnOnce(&MergePlan) -> Result<(), Encoding>,
) -> Result<Type, GroupBindingError<Encoding>> {
    validate_result_binding_structure(
        &group.binding,
        limits.binding.captures,
        limits.binding.groups,
        &mut validate_group_name,
    )
    .map_err(|_| GroupBindingError::Binding)?;
    group
        .validate_preflighted_plan(plan, limits.plan)
        .map_err(|_| GroupBindingError::Plan)?;
    validate_encoding(plan).map_err(GroupBindingError::Encoding)?;
    let ty = validate_result_binding(
        &group.binding,
        &group.pending,
        limits.binding,
        validate_group_name,
    )
    .map_err(|_| GroupBindingError::Binding)?;
    if (group.successor_sequence.is_none() || group.captures_successor())
        && !matches!(ty, Type::Scalar(_))
    {
        return Err(GroupBindingError::Plan);
    }
    Ok(ty)
}
