//! Accepted native reply restoration into a core-owned borrowed binding site.

use std::fmt;

use leselang_runtime_core::{AcceptedReply, CalculationFailure, Fuel, ScopeFrame};

use crate::effect_evaluation::{
    EffectEvaluationEnvironment, EffectEvaluationFailure, EffectEvaluationFault,
    EffectEvaluationLimits, EffectEvaluationOutcome, evaluate, native_failure, preflight,
};
use crate::ir::Computation;
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
    let mut bindings = environment
        .restore_capture(&identity, declaration, capture, fuel, limits)
        .map_err(native_failure)?;
    let mut scope = ScopeFrame::new(&mut bindings);
    if scope.len() >= limits.pure.max_bindings {
        return Err(EffectEvaluationFault::Pure(PureEvaluationFault::BindingLimit).into());
    }
    fuel.charge(scope.len() as u64)
        .map_err(|_| EffectEvaluationFault::Pure(PureEvaluationFault::FuelExhausted))?;
    preflight_value_scope(&scope, limits.pure).map_err(EffectEvaluationFault::Pure)?;
    if scope.get(name).is_some() {
        return Err(EffectEvaluationFault::Pure(PureEvaluationFault::InvalidContract).into());
    }
    let value = environment
        .bind_reply(&identity, declaration, reply, fuel)
        .map_err(native_failure)?;
    scope
        .push(name, value)
        .map_err(|_| EffectEvaluationFault::Pure(PureEvaluationFault::InvalidContract))?;
    preflight_value_scope(&scope, limits.pure).map_err(EffectEvaluationFault::Pure)?;
    evaluate(
        body,
        &mut scope,
        environment,
        fuel,
        limits.pure.max_bindings,
    )
}
