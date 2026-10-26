//! Correspondence of an admitted product group plan and its saved request rows.

use leserpent_hir::{Type, computation::Computation, host_call::HostOperation};

use crate::{
    EffectRequestView, GroupResultBinding, MergePlan, ProjectedGroupExt, ResultBindingLimits,
    validate_result_binding,
};

/// Original plan admission and complete result typing precede this borrowed view.
/// Limits are explicit native policy, not implicit VM defaults or resource grants.
pub struct GroupRequestContext<'a, Token> {
    pub owner: &'a Token,
    pub plan: &'a MergePlan,
    pub result_type: Type,
    pub max_successor_names: usize,
    pub binding_limits: ResultBindingLimits,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InvalidGroupRequests;

impl std::fmt::Display for InvalidGroupRequests {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("invalid group request correspondence")
    }
}

impl std::error::Error for InvalidGroupRequests {}

/// Check count/fuel, original members, appended successors, then prefix identities.
///
/// Callers must bound ingress, admit the original plan/binding and its result type,
/// and validate every request/frame before entry. Request views borrow existing rows
/// without an intermediate collection. Token equality compares owners; token text
/// preserves exact reserved spelling, and the canonical hook must recognize bounded
/// nonzero prefix identities. Hooks and native equality are trusted and may be called
/// repeatedly in traversal order; no rollback, retry or panic conversion is supplied.
/// Views and token projections must observe one immutable admitted snapshot; native
/// instrumentation may have side effects but must not change the compared data.
///
/// This checks inherited declared authority, not current grants, raw receipt contents,
/// original budget authenticity, settlement or journal provenance. Input, reservations
/// and meters are unchanged; success alone cannot authorize dispatch or recovery.
pub fn validate_preflighted_group_requests<Token: PartialEq, Request>(
    group: &GroupResultBinding,
    context: GroupRequestContext<'_, Token>,
    requests: &[(&str, Request)],
    view: impl for<'a> Fn(&'a Request) -> EffectRequestView<'a, Token>,
    token_text: impl Fn(&Token) -> &str,
    canonical_sequence: impl Fn(&Token) -> Option<u64>,
    mut validate_group_name: impl FnMut(&str) -> bool,
) -> Result<(), InvalidGroupRequests> {
    let branches = group.plan_branches().map_err(|_| InvalidGroupRequests)?;
    let prefix_cost = branches.len().checked_sub(1).ok_or(InvalidGroupRequests)? as u64;
    let first = view(&requests.first().ok_or(InvalidGroupRequests)?.1);
    if requests.len() != context.plan.branches.len()
        || first.continuation.fuel_remaining.checked_sub(prefix_cost) != Some(group.fuel_remaining)
    {
        return Err(InvalidGroupRequests);
    }
    for (index, (branch, (name, request))) in branches.iter().zip(requests).enumerate() {
        let request = view(request);
        let image = request.continuation;
        let expected_fuel =
            first
                .continuation
                .fuel_remaining
                .checked_sub(if context.plan.order.is_sequential() {
                    index as u64
                } else {
                    0
                });
        if *name != branch.name
            || image.pending_effect != branch.effect
            || image.result_type != branch.result_type
            || image.group_result.as_ref() != Some(context.owner)
            || image.result_binding.is_some()
            || image.schema_version != group.continuation_schema()
            || Some(image.fuel_remaining) != expected_fuel
            || !first.has_same_inherited_context(&request)
        {
            return Err(InvalidGroupRequests);
        }
    }
    let mut previous_fuel = group.fuel_remaining;
    for (index, (name, request)) in requests.iter().skip(branches.len()).enumerate() {
        let request = view(request);
        let image = request.continuation;
        let sequence = group
            .reserved_sequences()
            .nth(index)
            .ok_or(InvalidGroupRequests)?;
        if *name
            != group
                .plan_successor_name_at(index, context.max_successor_names)
                .map_err(|_| InvalidGroupRequests)?
            || token_text(&image.token) != format!("continuation-{sequence}")
            || image.schema_version != group.continuation_schema()
            || image.group_result.as_ref() != Some(context.owner)
            || HostOperation::for_effect(&image.pending_effect).is_none()
            || if group.captures_successor() {
                image.result_binding.as_ref().is_none_or(|binding| {
                    !(binding.body.is_pure()
                        || (group.is_dataflow() && binding.body.is_result_chain())
                        || (group.is_conditional() && binding.body.is_result_flow()))
                        || binding.groups.is_empty()
                        || validate_result_binding(
                            binding,
                            &image.pending_effect,
                            context.binding_limits,
                            &mut validate_group_name,
                        )
                        .ok()
                            != Some(context.result_type)
                        || !group.preflighted_capture_accepts_type_at(index, image.result_type)
                        || !binding
                            .groups
                            .iter()
                            .any(|saved| saved.name == group.binding.name)
                        || binding
                            .groups
                            .iter()
                            .any(|saved| !saved.group.matches_signature(branches))
                })
            } else {
                image.result_binding.is_some() || image.result_type != context.result_type
            }
            || image.fuel_remaining >= previous_fuel
            || !first.has_same_inherited_context(&request)
        {
            return Err(InvalidGroupRequests);
        }
        previous_fuel = image.fuel_remaining;
    }
    if let Some(sequence) = group.successor_sequence
        && requests.iter().take(branches.len()).any(|(_, request)| {
            canonical_sequence(&view(request).continuation.token)
                .is_none_or(|prefix| prefix >= sequence)
        })
    {
        return Err(InvalidGroupRequests);
    }
    Ok(())
}

impl GroupResultBinding {
    /// Look up a captured successor's possible type in already bounded result IR.
    /// Cold alternatives are both inspected. This does not validate the binding,
    /// admit a request/value, authenticate a receipt or authorize restoration.
    pub fn preflighted_capture_accepts_type_at(&self, position: usize, ty: Type) -> bool {
        capture_accepts_type_at(&self.binding.body, position, ty)
    }
}

fn capture_accepts_type_at(expression: &Computation, position: usize, ty: Type) -> bool {
    match expression {
        Computation::Bind { value, body, .. } if value.is_pure() => {
            capture_accepts_type_at(body, position, ty)
        }
        Computation::Bind { body, .. } if position > 0 => {
            capture_accepts_type_at(body, position - 1, ty)
        }
        Computation::Bind { value, .. } => value
            .prepared_atomic_operation()
            .is_some_and(|operation| operation.result_type() == ty),
        Computation::Choose {
            then, otherwise, ..
        } => {
            capture_accepts_type_at(then, position, ty)
                || capture_accepts_type_at(otherwise, position, ty)
        }
        _ => false,
    }
}
