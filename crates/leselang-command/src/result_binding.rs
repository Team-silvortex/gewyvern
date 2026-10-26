//! Product capture validation, separate from execution and journal ownership.

use leselang_hir_core::projection_access::ProjectionAccessLimits;
use leselang_runtime_core::{
    CaptureValidationLimits, GroupCaptureLimits, validate_captured_bindings,
};
use leserpent_hir::{
    Effect, Type,
    computation::{Computation, GroupLocalType},
};

use crate::{
    InvalidProjectedResult, ProjectedGroup, ProjectedGroupExt, ProjectedResult,
    validate_captured_projection_accesses,
};

/// Exact product specialization of the core's mutable captured-data container.
/// Construction/serde do not validate it or grant restoration authority.
pub type ResultBinding =
    leselang_runtime_core::CapturedBindings<Computation, ProjectedResult, ProjectedGroup>;

/// Caller-selected limits, with no VM defaults or silent repairs.
/// HIR structure/type checking retains its own language safety ceilings.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResultBindingLimits {
    pub captures: CaptureValidationLimits,
    pub groups: GroupCaptureLimits,
    /// Used only when saved results/groups require closed-field observation.
    pub accesses: ProjectionAccessLimits,
}

/// Validate count/name/scalar bounds, ordered captures, then result-flow shape.
///
/// This is not lexical/type validation. Callers bound decoded input, aggregate
/// payloads and trusted name-callback work. Callbacks run in saved group/member
/// order; early count/scalar/result refusal skips later hooks. Native side effects
/// are not rolled back on refusal or unwind, and unwinding propagates.
pub fn validate_result_binding_structure(
    binding: &ResultBinding,
    captures: CaptureValidationLimits,
    groups: GroupCaptureLimits,
    mut validate_group_name: impl FnMut(&str) -> bool,
) -> Result<(), InvalidProjectedResult> {
    validate_captured_bindings(
        binding,
        captures,
        ProjectedResult::validate,
        |group| group.validate(groups, &mut validate_group_name),
        |body| {
            body.validate_structure()
                .map_err(|_| InvalidProjectedResult)?;
            if !body.is_result_flow() {
                return Err(InvalidProjectedResult);
            }
            Ok(())
        },
    )
    .map_err(|_| InvalidProjectedResult)
}

/// Validate a product capture and compute its residual result type.
///
/// Preserves structure, closed-field observation, then canonical/type checking.
/// The canonical pass reconstructs owned lexical IR only after the earlier gates;
/// it does not mutate input, execute the body, dispatch work or charge/refill fuel.
/// The scalar-only path retains literal wrapping and omits projection observation.
///
/// Callers must bound the pending effect and total ingress before entry, validate
/// original schema/program/receipt identity and authority, and own budgets and
/// durable recovery. Type success is not an authentic frame or permission to
/// resume. Name callbacks remain trusted native code as documented above.
pub fn validate_result_binding(
    binding: &ResultBinding,
    pending: &Effect,
    limits: ResultBindingLimits,
    validate_group_name: impl FnMut(&str) -> bool,
) -> Result<Type, InvalidProjectedResult> {
    validate_result_binding_structure(
        binding,
        limits.captures,
        limits.groups,
        validate_group_name,
    )?;
    if !binding.results.is_empty() || !binding.groups.is_empty() {
        validate_captured_projection_accesses(binding, pending, limits.accesses)?;
    }
    validate_canonical_binding_type(binding, pending)
}

fn validate_canonical_binding_type(
    binding: &ResultBinding,
    pending: &Effect,
) -> Result<Type, InvalidProjectedResult> {
    // Reconstruct the lexical scope for the normal type/canonical validator.
    // The normal type checker also fences groups and effectful operands.
    let mut expression = Computation::Bind {
        name: binding.name.clone(),
        value: Box::new(Computation::Host {
            effect: Box::new(pending.clone()),
        }),
        body: Box::new(binding.body.clone()),
    };
    if !binding.results.is_empty() || !binding.groups.is_empty() {
        let scope = binding
            .locals
            .iter()
            .map(|local| (local.name.clone(), Type::Scalar(local.value.scalar_type())))
            .chain(
                binding
                    .results
                    .iter()
                    .map(|result| (result.name.clone(), result.result.operation.result_type())),
            )
            .collect::<Vec<_>>();
        let groups = binding
            .groups
            .iter()
            .map(|saved| GroupLocalType {
                name: saved.name.clone(),
                members: saved
                    .group
                    .members
                    .iter()
                    .map(|member| (member.name.clone(), member.result.operation))
                    .collect(),
            })
            .collect::<Vec<_>>();
        return expression
            .validate_in_group_scope(&scope, &groups)
            .map_err(|_| InvalidProjectedResult);
    }
    for local in binding.locals.iter().rev() {
        expression = Computation::Bind {
            name: local.name.clone(),
            value: Box::new(Computation::Literal {
                value: local.value.clone(),
            }),
            body: Box::new(expression),
        };
    }
    expression
        .validate_in_scope(&[])
        .map_err(|_| InvalidProjectedResult)
}
