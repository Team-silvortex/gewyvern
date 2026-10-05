//! Shared synchronous control execution with explicit, non-dispatching host hooks.

use std::fmt;

use leselang_runtime_core::{CalculationFailure, Fuel, ScalarValue, ScopeFrame, StructureBudget};

use crate::call_typing::{CallTypeError, MAX_CALL_ARGUMENTS, preflight_arguments_with_budget};
use crate::flow_typing::MAX_TYPED_GROUP_BRANCHES;
use crate::ir::{Computation, GroupKind};
use crate::pure_evaluation::{
    PureEvaluationEnvironment, PureEvaluationFailure, PureEvaluationFault, PureEvaluationLimits,
    PureValue, evaluate_preflighted_in_scope, preflight_value_scope,
};
use crate::pure_typing::{
    MAX_LOCAL_NAME_BYTES, PureTypeError, preflight_with_budget, valid_local_name,
};

type Node<Field, Operation, HostEffect, IrResult> =
    Computation<Field, Operation, HostEffect, IrResult>;
type EffectSites<'expression, Field, Operation, HostEffect, IrResult> =
    Vec<&'expression Node<Field, Operation, HostEffect, IrResult>>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EffectEvaluationLimits {
    pub pure: PureEvaluationLimits,
    pub max_arguments: usize,
    pub max_branches: usize,
}

/// Developer-owned preparation and capture, never actual effect dispatch.
///
/// All physical nodes/prefixes are bounded before any cold preflight hook. Every
/// cold Host/Call/Group is then checked before fuel, native value cloning or pure
/// projection. Hooks must retain the original schema, check versions/grants/named
/// shape and opaque payload policy, and must not invoke effects. Static field/type
/// inference and live dispatch authority remain separate host gates.
///
/// Selected preparation returns move-only request data. It owns argument-domain
/// checks and group identity/type policy, but not the already charged root unit.
/// Capture owns projection/snapshot limits and fuel and may retain borrowed IR.
/// Neither hook creates a receipt or durable replay authority. Native work may
/// mutate, allocate or unwind; it is not preempted, rolled back or retried.
pub trait EffectEvaluationEnvironment<'expression, Field, Operation, HostEffect, IrResult>:
    PureEvaluationEnvironment<Field, Operation>
{
    type Request;
    type Capture;

    fn preflight_effect(
        &self,
        expression: &'expression Node<Field, Operation, HostEffect, IrResult>,
    ) -> Result<(), Self::Error>;

    fn prepare_effect(
        &self,
        expression: &'expression Node<Field, Operation, HostEffect, IrResult>,
        scope: &mut ScopeFrame<'_, 'expression, PureValue<Self::Result>>,
        fuel: &mut Fuel,
    ) -> Result<Self::Request, CalculationFailure<Self::Error>>;

    fn capture(
        &self,
        name: &'expression str,
        body: &'expression Node<Field, Operation, HostEffect, IrResult>,
        scope: &ScopeFrame<'_, 'expression, PureValue<Self::Result>>,
        fuel: &mut Fuel,
    ) -> Result<Self::Capture, Self::Error>;
}

/// Ephemeral host-owned data, not a serialized continuation or execution grant.
/// No Clone/serde/Debug/Send bounds are imposed on requests, captures or results.
/// Consuming a suspension alone does not correlate or accept a returned reply.
///
/// ```compile_fail
/// use leselang_hir::effect_evaluation::EffectEvaluationOutcome;
/// use leselang_hir::pure_evaluation::PureValue;
/// use leselang_runtime_core::ScalarValue;
/// let outcome: EffectEvaluationOutcome<(), (), ()> =
///     EffectEvaluationOutcome::Value(PureValue::Scalar(ScalarValue::None));
/// let duplicate = outcome.clone();
/// ```
#[must_use = "the owning host must handle the value, prepared request or suspension"]
pub enum EffectEvaluationOutcome<ResultTag, Request, Capture> {
    Value(PureValue<ResultTag>),
    Request(Request),
    Suspended { request: Request, capture: Capture },
}

impl<ResultTag, Request, Capture> fmt::Debug
    for EffectEvaluationOutcome<ResultTag, Request, Capture>
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Value(_) => "Value",
            Self::Request(_) => "Request",
            Self::Suspended { .. } => "Suspended",
        })
    }
}

pub enum EffectEvaluationFault<Native> {
    Pure(PureEvaluationFault<Native>),
    CallShape(CallTypeError),
    GroupShape,
    NestedSuspension,
    Native(Native),
}

impl<Native> fmt::Display for EffectEvaluationFault<Native> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Pure(_) => "effect control value or structure rejected",
            Self::CallShape(_) => "effect control call shape rejected",
            Self::GroupShape => "effect control group shape rejected",
            Self::NestedSuspension => "effect control cannot capture an unfinished binding",
            Self::Native(_) => "effect control native adapter failed",
        })
    }
}

impl<Native> fmt::Debug for EffectEvaluationFault<Native> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, formatter)
    }
}

impl<Native> std::error::Error for EffectEvaluationFault<Native> {}

pub type EffectEvaluationFailure<Native> = CalculationFailure<EffectEvaluationFault<Native>>;
pub type EffectEvaluationResult<ResultTag, Request, Capture, Native> =
    Result<EffectEvaluationOutcome<ResultTag, Request, Capture>, EffectEvaluationFailure<Native>>;

fn pure_failure<Native>(failure: PureEvaluationFailure<Native>) -> EffectEvaluationFailure<Native> {
    match failure {
        CalculationFailure::Scalar(error) => CalculationFailure::Scalar(error),
        CalculationFailure::External(error) => EffectEvaluationFault::Pure(error).into(),
    }
}

fn native_failure<Native>(failure: CalculationFailure<Native>) -> EffectEvaluationFailure<Native> {
    match failure {
        CalculationFailure::Scalar(error) => CalculationFailure::Scalar(error),
        CalculationFailure::External(error) => EffectEvaluationFault::Native(error).into(),
    }
}

fn invalid<Native>() -> EffectEvaluationFailure<Native> {
    EffectEvaluationFault::Pure(PureEvaluationFault::InvalidContract).into()
}

fn preflight<'expression, Field, Operation, HostEffect, IrResult, NativeResult, NativeError>(
    expression: &'expression Node<Field, Operation, HostEffect, IrResult>,
    scope: &ScopeFrame<'_, 'expression, PureValue<NativeResult>>,
    limits: EffectEvaluationLimits,
) -> Result<
    EffectSites<'expression, Field, Operation, HostEffect, IrResult>,
    EffectEvaluationFailure<NativeError>,
> {
    if limits.max_arguments > MAX_CALL_ARGUMENTS || limits.max_branches > MAX_TYPED_GROUP_BRANCHES {
        return Err(EffectEvaluationFault::Pure(PureEvaluationFault::InvalidLimits).into());
    }
    preflight_value_scope(scope, limits.pure).map_err(EffectEvaluationFault::Pure)?;
    let mut budget = StructureBudget::new(limits.pure.max_nodes, limits.pure.max_depth);
    let mut pending = vec![(expression, 0)];
    let mut effects = Vec::new();
    while let Some((node, depth)) = pending.pop() {
        match node {
            Computation::Bind { .. }
            | Computation::Choose { .. }
            | Computation::Host { .. }
            | Computation::Call { .. }
            | Computation::Group { .. } => {
                budget.visit(depth, 0, 0).map_err(|error| {
                    EffectEvaluationFault::Pure(PureEvaluationFault::Preflight(
                        PureTypeError::Structure(error),
                    ))
                })?;
                if let Computation::Bind { name, .. } = node
                    && !valid_local_name(name)
                {
                    return Err(EffectEvaluationFault::Pure(PureEvaluationFault::Preflight(
                        PureTypeError::InvalidName,
                    ))
                    .into());
                }
            }
            _ => {
                preflight_with_budget(node, depth, &mut budget).map_err(|error| {
                    EffectEvaluationFault::Pure(PureEvaluationFault::Preflight(error))
                })?;
                continue;
            }
        }
        match node {
            Computation::Call { arguments, .. } => {
                preflight_arguments_with_budget(
                    arguments,
                    depth + 1,
                    &mut budget,
                    limits.max_arguments,
                )
                .map_err(EffectEvaluationFault::CallShape)?;
                effects.push(node);
                continue;
            }
            Computation::Host { .. } => {
                effects.push(node);
                continue;
            }
            Computation::Group {
                group_kind,
                branches,
            } => {
                let minimum = if *group_kind == GroupKind::Sequence {
                    1
                } else {
                    2
                };
                if branches.len() < minimum || branches.len() > limits.max_branches {
                    return Err(EffectEvaluationFault::GroupShape.into());
                }
                for (index, branch) in branches.iter().enumerate() {
                    if branch.name.is_empty()
                        || branch.name.len() > MAX_LOCAL_NAME_BYTES
                        || !branch
                            .name
                            .bytes()
                            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
                        || branches[..index]
                            .iter()
                            .any(|previous| previous.name == branch.name)
                    {
                        return Err(EffectEvaluationFault::GroupShape.into());
                    }
                }
                effects.push(node);
            }
            Computation::Choose { when, .. } => {
                preflight_with_budget(when, depth + 1, &mut budget).map_err(|error| {
                    EffectEvaluationFault::Pure(PureEvaluationFault::Preflight(error))
                })?;
            }
            _ => {}
        }
        for child in node.children().rev() {
            if let Computation::Choose { when, .. } = node
                && std::ptr::eq(child, when.as_ref())
            {
                continue;
            }
            budget.check_pending(pending.len(), 1).map_err(|error| {
                EffectEvaluationFault::Pure(PureEvaluationFault::Preflight(
                    PureTypeError::Structure(error),
                ))
            })?;
            pending.push((child, depth + 1));
        }
    }
    Ok(effects)
}

/// Execute the selected control path and return prepared data to its owning host.
///
/// Entire cold physical/prefix preflight precedes every adapter hook, then all
/// cold effect schemas precede fuel/value execution. This is not static typing:
/// hosts separately check all cold types and live dispatch policy. Bind/Choose
/// reuse one guarded lexical scope; scalar recovery and pure forms reuse the
/// pure executor. A nested suspension as a binding value is rejected, not flattened.
/// Capture failure drops the prepared request without dispatch or implicit retry.
/// The caller validates/correlates received replies and restores its captured
/// lexical values before entering again; no wire, journal or scheduler is supplied.
pub fn evaluate_effects_in_scope<'expression, Field, Operation, HostEffect, IrResult, Environment>(
    expression: &'expression Node<Field, Operation, HostEffect, IrResult>,
    scope: &mut ScopeFrame<'_, 'expression, PureValue<Environment::Result>>,
    environment: &Environment,
    fuel: &mut Fuel,
    limits: EffectEvaluationLimits,
) -> EffectEvaluationResult<
    Environment::Result,
    Environment::Request,
    Environment::Capture,
    Environment::Error,
>
where
    Environment: EffectEvaluationEnvironment<'expression, Field, Operation, HostEffect, IrResult>,
{
    let effects = preflight(expression, scope, limits)?;
    for effect in effects {
        environment
            .preflight_effect(effect)
            .map_err(EffectEvaluationFault::Native)?;
    }
    evaluate(
        expression,
        scope,
        environment,
        fuel,
        limits.pure.max_bindings,
    )
}

fn evaluate<'expression, Field, Operation, HostEffect, IrResult, Environment>(
    expression: &'expression Node<Field, Operation, HostEffect, IrResult>,
    scope: &mut ScopeFrame<'_, 'expression, PureValue<Environment::Result>>,
    environment: &Environment,
    fuel: &mut Fuel,
    max_bindings: usize,
) -> EffectEvaluationResult<
    Environment::Result,
    Environment::Request,
    Environment::Capture,
    Environment::Error,
>
where
    Environment: EffectEvaluationEnvironment<'expression, Field, Operation, HostEffect, IrResult>,
{
    match expression {
        Computation::Bind { .. }
        | Computation::Choose { .. }
        | Computation::Host { .. }
        | Computation::Call { .. }
        | Computation::Group { .. } => {}
        _ => {
            return evaluate_preflighted_in_scope(
                expression,
                scope,
                environment,
                fuel,
                max_bindings,
            )
            .map(EffectEvaluationOutcome::Value)
            .map_err(pure_failure);
        }
    }
    fuel.charge(1)
        .map_err(|_| EffectEvaluationFault::Pure(PureEvaluationFault::FuelExhausted))?;
    match expression {
        Computation::Bind { name, value, body } => {
            let value = evaluate(value, scope, environment, fuel, max_bindings)?;
            if scope.len() >= max_bindings {
                return Err(EffectEvaluationFault::Pure(PureEvaluationFault::BindingLimit).into());
            }
            if scope.get(name).is_some() {
                return Err(invalid());
            }
            match value {
                EffectEvaluationOutcome::Value(value) => {
                    let mut local = scope.nested();
                    local.push(name, value).map_err(|_| invalid())?;
                    evaluate(body, &mut local, environment, fuel, max_bindings)
                }
                EffectEvaluationOutcome::Request(request) => {
                    let capture = environment
                        .capture(name, body, scope, fuel)
                        .map_err(EffectEvaluationFault::Native)?;
                    Ok(EffectEvaluationOutcome::Suspended { request, capture })
                }
                EffectEvaluationOutcome::Suspended { .. } => {
                    Err(EffectEvaluationFault::NestedSuspension.into())
                }
            }
        }
        Computation::Choose {
            when,
            then,
            otherwise,
        } => {
            let PureValue::Scalar(ScalarValue::Boolean(selected)) =
                evaluate_preflighted_in_scope(when, scope, environment, fuel, max_bindings)
                    .map_err(pure_failure)?
            else {
                return Err(invalid());
            };
            evaluate(
                if selected { then } else { otherwise },
                scope,
                environment,
                fuel,
                max_bindings,
            )
        }
        Computation::Host { .. } | Computation::Call { .. } | Computation::Group { .. } => {
            environment
                .prepare_effect(expression, scope, fuel)
                .map(EffectEvaluationOutcome::Request)
                .map_err(native_failure)
        }
        _ => Err(invalid()),
    }
}
