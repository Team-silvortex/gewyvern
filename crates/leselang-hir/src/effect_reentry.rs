//! Accepted native reply restoration into a core-owned borrowed binding site.

use std::fmt;

use leselang_runtime_core::{
    AcceptedReply, CalculationFailure, CaptureScopeError, CaptureScopeHooks, CapturedBindings,
    Fuel, LocalExecutionScope, ScalarValue, ScopeFrame, with_captured_scope_state,
    with_owned_scope_state,
};

use crate::effect_evaluation::{
    ControlOutcome, EffectEvaluationEnvironment, EffectEvaluationFailure, EffectEvaluationFault,
    EffectEvaluationLimits, EffectEvaluationOutcome, evaluate, evaluate_resumable_effects_in_scope,
    native_failure, preflight,
};
use crate::ir::Computation;
use crate::local_scope::CallFence;
use crate::pure_evaluation::{
    PureEvaluationEnvironment, PureEvaluationFault, PureValue, preflight_value_scope,
};

/// Borrowed original bind site plus opaque native capture, never a wire checkpoint.
/// Only the shared walker constructs this object. Native capture cannot select a
/// different name/body during restoration. Extraction is data, not authority.
/// No Clone/serde/Debug/Send bounds are imposed on the borrowed IR or capture.
///
/// ```compile_fail
/// use leselang_hir::effect_reentry::EffectContinuation;
/// fn duplicate(frame: EffectContinuation<'_, (), ()>) { let copy = frame.clone(); }
/// ```
///
/// ```compile_fail
/// use leselang_hir::effect_reentry::EffectContinuation;
/// fn wire(frame: EffectContinuation<'_, (), ()>) {
///     let wire = serde_json::to_string(&frame).unwrap();
/// }
/// ```
#[must_use = "retain the original continuation until acceptance or cancellation"]
pub struct EffectContinuation<'expression, Node: ?Sized, Capture> {
    name: &'expression str,
    body: &'expression Node,
    capture: Capture,
}

impl<'expression, Node: ?Sized, Capture> EffectContinuation<'expression, Node, Capture> {
    pub(crate) fn new(name: &'expression str, body: &'expression Node, capture: Capture) -> Self {
        Self {
            name,
            body,
            capture,
        }
    }

    pub fn name(&self) -> &'expression str {
        self.name
    }
    pub fn body(&self) -> &'expression Node {
        self.body
    }

    pub fn into_parts(self) -> (&'expression str, &'expression Node, Capture) {
        (self.name, self.body, self.capture)
    }
}

impl<Node: ?Sized, Capture> fmt::Debug for EffectContinuation<'_, Node, Capture> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("EffectContinuation")
    }
}

#[must_use = "handle the value, prepared request or original binding continuation"]
pub enum ResumableEffectOutcome<'expression, Node: ?Sized, ResultTag, Request, Capture> {
    Value(PureValue<ResultTag>),
    Request(Request),
    Suspended {
        request: Request,
        continuation: EffectContinuation<'expression, Node, Capture>,
    },
}

impl<'expression, Node: ?Sized, Value, Request, Capture>
    From<ResumableEffectOutcome<'expression, Node, Value, Request, Capture>>
    for ControlOutcome<Value, Request, EffectContinuation<'expression, Node, Capture>>
{
    fn from(outcome: ResumableEffectOutcome<'expression, Node, Value, Request, Capture>) -> Self {
        match outcome {
            ResumableEffectOutcome::Value(value) => Self::Value(value),
            ResumableEffectOutcome::Request(request) => Self::Request(request),
            ResumableEffectOutcome::Suspended {
                request,
                continuation,
            } => Self::Suspended {
                request,
                continuation,
            },
        }
    }
}

impl<Node: ?Sized, ResultTag, Request, Capture>
    ResumableEffectOutcome<'_, Node, ResultTag, Request, Capture>
{
    pub fn into_legacy(self) -> EffectEvaluationOutcome<ResultTag, Request, Capture> {
        match self {
            Self::Value(value) => EffectEvaluationOutcome::Value(value),
            Self::Request(request) => EffectEvaluationOutcome::Request(request),
            Self::Suspended {
                request,
                continuation,
            } => {
                let (_, _, capture) = continuation.into_parts();
                EffectEvaluationOutcome::Suspended { request, capture }
            }
        }
    }
}

impl<Node: ?Sized, ResultTag, Request, Capture> fmt::Debug
    for ResumableEffectOutcome<'_, Node, ResultTag, Request, Capture>
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Value(_) => "Value",
            Self::Request(_) => "Request",
            Self::Suspended { .. } => "Suspended",
        })
    }
}

pub type ResumableEffectResult<'expression, Node, ResultTag, Request, Capture, Native> = Result<
    ResumableEffectOutcome<'expression, Node, ResultTag, Request, Capture>,
    EffectEvaluationFailure<Native>,
>;
pub type RestoredEffectBindings<'expression, ResultTag> =
    Vec<(&'expression str, PureValue<ResultTag>)>;

/// Native saved-value restoration and reply projection, never body selection.
/// Each hook receives the same original meter; no accounting policy is implicit.
///
/// A restoration adapter cannot replace the shared body evaluator:
///
/// ```compile_fail
/// use leselang_hir::effect_reentry::CapturedEffectHooks;
/// let replacement = CapturedEffectHooks {
///     scalar: (), result: (), group: (), pending: (), body: (),
/// };
/// ```
pub struct CapturedEffectHooks<Scalar, ResultHook, Group, Pending> {
    pub scalar: Scalar,
    pub result: ResultHook,
    pub group: Group,
    pub pending: Pending,
}

pub type CapturedEffectResult<'expression, Node, Value, Request, Capture, Native> = Result<
    ResumableEffectOutcome<'expression, Node, Value, Request, Capture>,
    CaptureScopeError<EffectEvaluationFailure<Native>>,
>;
pub type CapturedEffectControlResult<
    'expression,
    Field,
    Operation,
    HostEffect,
    IrResult,
    Environment,
> = CapturedEffectResult<
    'expression,
    Computation<Field, Operation, HostEffect, IrResult>,
    <Environment as PureEvaluationEnvironment<Field, Operation>>::Result,
    <Environment as EffectEvaluationEnvironment<
        'expression,
        Field,
        Operation,
        HostEffect,
        IrResult,
    >>::Request,
    <Environment as EffectEvaluationEnvironment<
        'expression,
        Field,
        Operation,
        HostEffect,
        IrResult,
    >>::Capture,
    <Environment as PureEvaluationEnvironment<Field, Operation>>::Error,
>;

/// Re-enter a borrowed saved body through the shared effect walker.
///
/// Restore scalar/result/group rows and the reply in original wire order, then
/// evaluate the exact saved body with the assembled scope and original Fuel.
/// Native restoration precedes each duplicate check, retaining legacy debit and
/// refusal precedence. Body physical/scope and all cold effect preflight occur
/// after restoration and before body execution; there is no extra early pass or
/// native hook retry. Hooks cannot supply a replacement body or custom evaluator.
/// Scalar calculation failures retain their typed channel; native failures stay
/// external even when their diagnostics resemble recoverable language faults.
///
/// Scope suffix and remaining hook captures finish cleanup before output/error
/// handoff. Saved data and the existing prefix remain caller-owned. Names/body
/// are borrowed; native saved rows, requests and captures need no Clone/Debug/
/// serde/Default/Send/Sync bounds. Result views retain the evaluator's existing
/// Clone contract. Drop/callback unwind propagates; a second cleanup panic may
/// abort. Charges and native mutations are not rolled back or refunded.
///
/// This is execution, not saved-frame admission or reply acceptance. Before entry
/// the host must bound codecs/aggregate storage and validate the original program,
/// complete capture structure/schema, reply provenance, current policy and durable
/// state. It must supply the authentic existing meter and bound native hook work.
/// This function creates no fuel grant, executable certificate, receipt, journal,
/// dispatch or replay fence. Repeating a call is not globally deduplicated.
pub fn resume_captured_effects_in_scope<
    'expression,
    Field,
    Operation,
    HostEffect,
    IrResult,
    SavedResult,
    SavedGroup,
    Environment,
    Scalar,
    ResultHook,
    Group,
    Pending,
>(
    capture: &'expression CapturedBindings<
        Computation<Field, Operation, HostEffect, IrResult>,
        SavedResult,
        SavedGroup,
    >,
    parent: &mut ScopeFrame<'_, 'expression, PureValue<Environment::Result>>,
    environment: &Environment,
    fuel: &mut Fuel,
    limits: EffectEvaluationLimits,
    hooks: CapturedEffectHooks<Scalar, ResultHook, Group, Pending>,
) -> CapturedEffectControlResult<'expression, Field, Operation, HostEffect, IrResult, Environment>
where
    Environment: EffectEvaluationEnvironment<'expression, Field, Operation, HostEffect, IrResult>,
    Scalar: FnMut(
        &'expression ScalarValue,
        &mut Fuel,
    )
        -> Result<PureValue<Environment::Result>, CalculationFailure<Environment::Error>>,
    ResultHook:
        FnMut(
            &'expression SavedResult,
            &mut Fuel,
        )
            -> Result<PureValue<Environment::Result>, CalculationFailure<Environment::Error>>,
    Group: FnMut(
        &'expression SavedGroup,
        &mut Fuel,
    )
        -> Result<PureValue<Environment::Result>, CalculationFailure<Environment::Error>>,
    Pending:
        FnOnce(
            &mut Fuel,
        )
            -> Result<PureValue<Environment::Result>, CalculationFailure<Environment::Error>>,
{
    let CapturedEffectHooks {
        mut scalar,
        mut result,
        mut group,
        pending,
    } = hooks;
    with_captured_scope_state(
        capture,
        parent,
        fuel,
        CaptureScopeHooks {
            scalar: move |value, fuel: &mut Fuel| scalar(value, fuel).map_err(native_failure),
            result: move |value, fuel: &mut Fuel| result(value, fuel).map_err(native_failure),
            group: move |value, fuel: &mut Fuel| group(value, fuel).map_err(native_failure),
            pending: move |fuel: &mut Fuel| pending(fuel).map_err(native_failure),
            body: |body,
                   scope: &mut ScopeFrame<'_, 'expression, PureValue<Environment::Result>>,
                   fuel: &mut Fuel| {
                evaluate_resumable_effects_in_scope(body, scope, environment, fuel, limits)
            },
        },
    )
}

pub type EffectControlResult<'expression, Field, Operation, HostEffect, IrResult, Environment> =
    ResumableEffectResult<
        'expression,
        Computation<Field, Operation, HostEffect, IrResult>,
        <Environment as PureEvaluationEnvironment<Field, Operation>>::Result,
        <Environment as EffectEvaluationEnvironment<
            'expression,
            Field,
            Operation,
            HostEffect,
            IrResult,
        >>::Request,
        <Environment as EffectEvaluationEnvironment<
            'expression,
            Field,
            Operation,
            HostEffect,
            IrResult,
        >>::Capture,
        <Environment as PureEvaluationEnvironment<Field, Operation>>::Error,
    >;
pub type AcceptedEffectContinuation<
    'schema,
    'expression,
    Identity,
    Field,
    Operation,
    HostEffect,
    IrResult,
    Capture,
    Declaration,
    Reply,
> = AcceptedReply<
    'schema,
    Identity,
    EffectContinuation<'expression, Computation<Field, Operation, HostEffect, IrResult>, Capture>,
    Declaration,
    Reply,
>;

/// Trusted native restoration/projection, never dispatch, authority or IR selection.
/// The host owns capture integrity, live schema/result-view compatibility, bounded
/// ingress/native work and costs inside these hooks. `restore_capture` must check
/// opaque size/type policy before allocation or native work and charge its costs.
/// Returning bindings moves values, not a serialized observation or grant.
/// Result views may be Clone for local reads; incoming payload/capture need not be.
pub trait EffectReentryEnvironment<
    'expression,
    Field,
    Operation,
    HostEffect,
    IrResult,
    Identity,
    Declaration: ?Sized,
    Reply,
>: EffectEvaluationEnvironment<'expression, Field, Operation, HostEffect, IrResult>
{
    fn restore_capture(
        &self,
        identity: &Identity,
        declaration: &Declaration,
        capture: Self::Capture,
        fuel: &mut Fuel,
        limits: EffectEvaluationLimits,
    ) -> Result<RestoredEffectBindings<'expression, Self::Result>, CalculationFailure<Self::Error>>;

    fn bind_reply(
        &self,
        identity: &Identity,
        declaration: &Declaration,
        reply: Reply,
        fuel: &mut Fuel,
    ) -> Result<PureValue<Self::Result>, CalculationFailure<Self::Error>>;
}

/// Consume one accepted frame and re-enter its exact original body.
/// Cold physical and all current schema preflight precede native restoration or
/// projection. Bounded restored prefixes cost one fuel unit per binding before
/// validation; native restoration/copy/projection costs remain explicit hooks.
/// Prefix duplicates, bad names/scalars, capacity and reply-name shadowing stop
/// before projection. Mapped scalar bounds are checked before body preparation;
/// native result-view type compatibility remains explicit adapter policy.
/// Owned restored values and the reply binding are dropped on every exit/unwind.
/// Outputs/errors stay local until framework-owned prefix, unused input and
/// identity cleanup finish; callback-internal cleanup remains native responsibility.
/// Consumption never rearms the accepted handle or refunds/retries native work.
/// Acceptance is an in-memory observation: hosts still own provenance, mutation,
/// live dispatch, group/opaque typing and durable cancellation/replay semantics.
pub fn resume_accepted_effects<
    'expression,
    'schema,
    Field,
    Operation,
    HostEffect,
    IrResult,
    Identity,
    Declaration: ?Sized,
    Reply,
    Environment,
>(
    accepted: AcceptedEffectContinuation<
        'schema,
        'expression,
        Identity,
        Field,
        Operation,
        HostEffect,
        IrResult,
        Environment::Capture,
        Declaration,
        Reply,
    >,
    environment: &Environment,
    fuel: &mut Fuel,
    limits: EffectEvaluationLimits,
) -> EffectControlResult<'expression, Field, Operation, HostEffect, IrResult, Environment>
where
    Environment: EffectReentryEnvironment<
            'expression,
            Field,
            Operation,
            HostEffect,
            IrResult,
            Identity,
            Declaration,
            Reply,
        >,
{
    let (identity, continuation, declaration, reply) = accepted.into_parts();
    let (name, body, capture) = continuation.into_parts();
    let mut capture = Some(capture);
    let mut reply = Some(reply);
    // Borrow pending native ownership so every error/output remains local during
    // unused capture/reply and identity cleanup, including cold refusal.
    let output = (|| {
        let mut empty: RestoredEffectBindings<'expression, Environment::Result> = Vec::new();
        let effects = preflight(body, &ScopeFrame::new(&mut empty), limits)?;
        if limits.pure.max_bindings == 0 {
            return Err(EffectEvaluationFault::Pure(PureEvaluationFault::BindingLimit).into());
        }
        for effect in effects {
            environment
                .preflight_effect(effect)
                .map_err(EffectEvaluationFault::Native)?;
        }
        let saved = capture.take().ok_or(EffectEvaluationFault::Pure(
            PureEvaluationFault::InvalidContract,
        ))?;
        let bindings = restore_effect_scope(
            name,
            |fuel| environment.restore_capture(&identity, declaration, saved, fuel, limits),
            fuel,
            limits,
        )?;
        with_owned_scope_state(bindings, fuel, |scope, fuel| {
            let incoming = reply.take().ok_or(EffectEvaluationFault::Pure(
                PureEvaluationFault::InvalidContract,
            ))?;
            let value = environment
                .bind_reply(&identity, declaration, incoming, fuel)
                .map_err(native_failure)?;
            bind_effect_value(name, value, scope, limits)?;
            evaluate(body, scope, environment, fuel, limits.pure.max_bindings)
        })
    })();
    drop((capture, reply, identity));
    output
}

// The caller preflights every original pending body before native restoration.
pub(crate) fn restore_effect_scope<'expression, Value, Native>(
    name: &'expression str,
    restore: impl FnOnce(
        &mut Fuel,
    ) -> Result<
        RestoredEffectBindings<'expression, Value>,
        CalculationFailure<Native>,
    >,
    fuel: &mut Fuel,
    limits: EffectEvaluationLimits,
) -> Result<RestoredEffectBindings<'expression, Value>, EffectEvaluationFailure<Native>> {
    restore_effect_scope_with_liveness(name, restore, fuel, limits, None)
}

pub(crate) fn restore_effect_scope_with_liveness<'expression, Value, Native>(
    name: &'expression str,
    restore: impl FnOnce(
        &mut Fuel,
    ) -> Result<
        RestoredEffectBindings<'expression, Value>,
        CalculationFailure<Native>,
    >,
    fuel: &mut Fuel,
    limits: EffectEvaluationLimits,
    execution_scope: Option<&LocalExecutionScope>,
) -> Result<RestoredEffectBindings<'expression, Value>, EffectEvaluationFailure<Native>> {
    if limits.pure.max_bindings == 0 {
        return Err(EffectEvaluationFault::Pure(PureEvaluationFault::BindingLimit).into());
    }
    let mut bindings = restore(fuel).map_err(native_failure)?;
    let checked = CallFence::run(execution_scope, || {
        let scope = ScopeFrame::new(&mut bindings);
        if scope.len() >= limits.pure.max_bindings {
            return Err(EffectEvaluationFault::Pure(PureEvaluationFault::BindingLimit).into());
        }
        fuel.charge(scope.len() as u64)
            .map_err(|_| EffectEvaluationFault::Pure(PureEvaluationFault::FuelExhausted))?;
        preflight_value_scope(&scope, limits.pure).map_err(EffectEvaluationFault::Pure)?;
        if scope.get(name).is_some() {
            return Err(EffectEvaluationFault::Pure(PureEvaluationFault::InvalidContract).into());
        }
        Ok(())
    });
    match checked {
        Ok(()) => Ok(bindings),
        Err(error) => {
            let output = Err(error);
            drop(bindings);
            output
        }
    }
}

pub(crate) fn bind_effect_value<'expression, Value, Native>(
    name: &'expression str,
    value: PureValue<Value>,
    scope: &mut ScopeFrame<'_, 'expression, PureValue<Value>>,
    limits: EffectEvaluationLimits,
) -> Result<(), EffectEvaluationFailure<Native>> {
    scope
        .push(name, value)
        .map_err(|_| EffectEvaluationFault::Pure(PureEvaluationFault::InvalidContract))?;
    preflight_value_scope(scope, limits.pure).map_err(EffectEvaluationFault::Pure)?;
    Ok(())
}
