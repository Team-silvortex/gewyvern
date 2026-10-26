//! Shared synchronous control execution with explicit, non-dispatching host hooks.

use std::fmt;

use leselang_runtime_core::{CalculationFailure, Fuel, ScalarValue, ScopeFrame, StructureBudget};

use crate::call_typing::{CallTypeError, MAX_CALL_ARGUMENTS, preflight_arguments_with_budget};
use crate::effect_reentry::{EffectContinuation, ResumableEffectOutcome};
use crate::flow_typing::MAX_TYPED_GROUP_BRANCHES;
use crate::ir::Computation;
use crate::native_group::preflight_native_group_members;
use crate::pure_evaluation::{
    PureEvaluationEnvironment, PureEvaluationFailure, PureEvaluationFault, PureEvaluationLimits,
    PureValue, evaluate_preflighted_in_scope, preflight_value_scope,
};
use crate::pure_typing::{PureTypeError, preflight_with_budget, valid_local_name};

type Node<Field, Operation, HostEffect, IrResult> =
    Computation<Field, Operation, HostEffect, IrResult>;
pub(crate) type EffectSites<'expression, Field, Operation, HostEffect, IrResult> =
    Vec<&'expression Node<Field, Operation, HostEffect, IrResult>>;

pub(crate) enum ControlOutcome<Value, Request, Frame> {
    Value(PureValue<Value>),
    Request(Request),
    Suspended {
        request: Request,
        continuation: Frame,
    },
}
type ControlResult<Value, Request, Frame, Native> =
    Result<ControlOutcome<Value, Request, Frame>, EffectEvaluationFailure<Native>>;

pub(crate) trait SuspensionProfile<'expression, Node: ?Sized, Capture> {
    type Frame;
    fn admits(frame: Option<&Self::Frame>, max_frames: usize) -> bool;
    fn append(
        frame: Option<Self::Frame>,
        next: EffectContinuation<'expression, Node, Capture>,
    ) -> Result<Self::Frame, ()>;
}
pub(crate) struct SingleFrame;
impl<'expression, Node: ?Sized + 'expression, Capture> SuspensionProfile<'expression, Node, Capture>
    for SingleFrame
{
    type Frame = EffectContinuation<'expression, Node, Capture>;
    fn admits(frame: Option<&Self::Frame>, _: usize) -> bool {
        frame.is_none()
    }
    fn append(frame: Option<Self::Frame>, next: Self::Frame) -> Result<Self::Frame, ()> {
        if frame.is_none() { Ok(next) } else { Err(()) }
    }
}

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
/// use leselang_hir_core::effect_evaluation::EffectEvaluationOutcome;
/// use leselang_hir_core::pure_evaluation::PureValue;
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

pub(crate) fn native_failure<Native>(
    failure: CalculationFailure<Native>,
) -> EffectEvaluationFailure<Native> {
    match failure {
        CalculationFailure::Scalar(error) => CalculationFailure::Scalar(error),
        CalculationFailure::External(error) => EffectEvaluationFault::Native(error).into(),
    }
}

fn invalid<Native>() -> EffectEvaluationFailure<Native> {
    EffectEvaluationFault::Pure(PureEvaluationFault::InvalidContract).into()
}

pub(crate) fn preflight<
    'expression,
    Field,
    Operation,
    HostEffect,
    IrResult,
    NativeResult,
    NativeError,
>(
    expression: &'expression Node<Field, Operation, HostEffect, IrResult>,
    scope: &ScopeFrame<'_, 'expression, PureValue<NativeResult>>,
    limits: EffectEvaluationLimits,
) -> Result<
    EffectSites<'expression, Field, Operation, HostEffect, IrResult>,
    EffectEvaluationFailure<NativeError>,
> {
    preflight_scope(scope, limits)?;
    let mut budget = StructureBudget::new(limits.pure.max_nodes, limits.pure.max_depth);
    let mut effects = Vec::new();
    preflight_into(expression, limits, &mut budget, &mut effects)?;
    Ok(effects)
}

pub(crate) fn preflight_scope<Value, Native>(
    scope: &ScopeFrame<'_, '_, PureValue<Value>>,
    limits: EffectEvaluationLimits,
) -> Result<(), EffectEvaluationFailure<Native>> {
    if limits.max_arguments > MAX_CALL_ARGUMENTS || limits.max_branches > MAX_TYPED_GROUP_BRANCHES {
        return Err(EffectEvaluationFault::Pure(PureEvaluationFault::InvalidLimits).into());
    }
    preflight_value_scope(scope, limits.pure).map_err(EffectEvaluationFault::Pure)?;
    Ok(())
}

pub(crate) fn preflight_into<'expression, Field, Operation, HostEffect, IrResult, Native>(
    expression: &'expression Node<Field, Operation, HostEffect, IrResult>,
    limits: EffectEvaluationLimits,
    budget: &mut StructureBudget,
    effects: &mut EffectSites<'expression, Field, Operation, HostEffect, IrResult>,
) -> Result<(), EffectEvaluationFailure<Native>> {
    let mut pending = vec![(expression, 0)];
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
                preflight_with_budget(node, depth, budget).map_err(|error| {
                    EffectEvaluationFault::Pure(PureEvaluationFault::Preflight(error))
                })?;
                continue;
            }
        }
        match node {
            Computation::Call { arguments, .. } => {
                preflight_arguments_with_budget(arguments, depth + 1, budget, limits.max_arguments)
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
                if preflight_native_group_members(
                    *group_kind,
                    branches,
                    limits.max_branches,
                    |branch| Ok::<_, std::convert::Infallible>(branch.name.as_str()),
                )
                .is_err()
                {
                    return Err(EffectEvaluationFault::GroupShape.into());
                }
                effects.push(node);
            }
            Computation::Choose { when, .. } => {
                preflight_with_budget(when, depth + 1, budget).map_err(|error| {
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
    Ok(())
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
    evaluate_resumable_effects_in_scope(expression, scope, environment, fuel, limits)
        .map(ResumableEffectOutcome::into_legacy)
}

/// Additive entry retaining the original binding site alongside native capture.
/// The move-only continuation can be placed in a `PendingReply` and consumed by
/// `resume_accepted_effects`. All preflight, evaluation and capture costs match
/// the legacy entry. This does not dispatch or create durable replay authority.
pub fn evaluate_resumable_effects_in_scope<
    'expression,
    Field,
    Operation,
    HostEffect,
    IrResult,
    Environment,
>(
    expression: &'expression Node<Field, Operation, HostEffect, IrResult>,
    scope: &mut ScopeFrame<'_, 'expression, PureValue<Environment::Result>>,
    environment: &Environment,
    fuel: &mut Fuel,
    limits: EffectEvaluationLimits,
) -> crate::effect_reentry::EffectControlResult<
    'expression,
    Field,
    Operation,
    HostEffect,
    IrResult,
    Environment,
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

pub(crate) fn evaluate<'expression, Field, Operation, HostEffect, IrResult, Environment>(
    expression: &'expression Node<Field, Operation, HostEffect, IrResult>,
    scope: &mut ScopeFrame<'_, 'expression, PureValue<Environment::Result>>,
    environment: &Environment,
    fuel: &mut Fuel,
    max_bindings: usize,
) -> crate::effect_reentry::EffectControlResult<
    'expression,
    Field,
    Operation,
    HostEffect,
    IrResult,
    Environment,
>
where
    Environment: EffectEvaluationEnvironment<'expression, Field, Operation, HostEffect, IrResult>,
{
    evaluate_with_profile::<_, _, _, _, _, SingleFrame>(
        expression,
        scope,
        environment,
        fuel,
        max_bindings,
        1,
    )
    .map(|outcome| match outcome {
        ControlOutcome::Value(value) => ResumableEffectOutcome::Value(value),
        ControlOutcome::Request(request) => ResumableEffectOutcome::Request(request),
        ControlOutcome::Suspended {
            request,
            continuation,
        } => ResumableEffectOutcome::Suspended {
            request,
            continuation,
        },
    })
}

pub(crate) fn evaluate_with_profile<
    'expression,
    Field,
    Operation,
    HostEffect,
    IrResult,
    Environment,
    Profile,
>(
    expression: &'expression Node<Field, Operation, HostEffect, IrResult>,
    scope: &mut ScopeFrame<'_, 'expression, PureValue<Environment::Result>>,
    environment: &Environment,
    fuel: &mut Fuel,
    max_bindings: usize,
    max_frames: usize,
) -> ControlResult<Environment::Result, Environment::Request, Profile::Frame, Environment::Error>
where
    Environment: EffectEvaluationEnvironment<'expression, Field, Operation, HostEffect, IrResult>,
    Profile: SuspensionProfile<
            'expression,
            Node<Field, Operation, HostEffect, IrResult>,
            Environment::Capture,
        >,
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
            .map(ControlOutcome::Value)
            .map_err(pure_failure);
        }
    }
    fuel.charge(1)
        .map_err(|_| EffectEvaluationFault::Pure(PureEvaluationFault::FuelExhausted))?;
    match expression {
        Computation::Bind { name, value, body } => {
            let value = evaluate_with_profile::<_, _, _, _, _, Profile>(
                value,
                scope,
                environment,
                fuel,
                max_bindings,
                max_frames,
            )?;
            if scope.len() >= max_bindings {
                return Err(EffectEvaluationFault::Pure(PureEvaluationFault::BindingLimit).into());
            }
            if scope.get(name).is_some() {
                return Err(invalid());
            }
            match value {
                ControlOutcome::Value(value) => {
                    let mut local = scope.nested();
                    local.push(name, value).map_err(|_| invalid())?;
                    evaluate_with_profile::<_, _, _, _, _, Profile>(
                        body,
                        &mut local,
                        environment,
                        fuel,
                        max_bindings,
                        max_frames,
                    )
                }
                ControlOutcome::Request(request) => {
                    if !Profile::admits(None, max_frames) {
                        return Err(EffectEvaluationFault::NestedSuspension.into());
                    }
                    let capture = environment
                        .capture(name, body, scope, fuel)
                        .map_err(EffectEvaluationFault::Native)?;
                    Ok(ControlOutcome::Suspended {
                        request,
                        continuation: Profile::append(
                            None,
                            EffectContinuation::new(name, body, capture),
                        )
                        .map_err(|_| EffectEvaluationFault::NestedSuspension)?,
                    })
                }
                ControlOutcome::Suspended {
                    request,
                    continuation,
                } => {
                    if !Profile::admits(Some(&continuation), max_frames) {
                        return Err(EffectEvaluationFault::NestedSuspension.into());
                    }
                    let capture = environment
                        .capture(name, body, scope, fuel)
                        .map_err(EffectEvaluationFault::Native)?;
                    Ok(ControlOutcome::Suspended {
                        request,
                        continuation: Profile::append(
                            Some(continuation),
                            EffectContinuation::new(name, body, capture),
                        )
                        .map_err(|_| EffectEvaluationFault::NestedSuspension)?,
                    })
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
            evaluate_with_profile::<_, _, _, _, _, Profile>(
                if selected { then } else { otherwise },
                scope,
                environment,
                fuel,
                max_bindings,
                max_frames,
            )
        }
        Computation::Host { .. } | Computation::Call { .. } | Computation::Group { .. } => {
            environment
                .prepare_effect(expression, scope, fuel)
                .map(ControlOutcome::Request)
                .map_err(native_failure)
        }
        _ => Err(invalid()),
    }
}
