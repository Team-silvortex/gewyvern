//! Opt-in bounded in-memory return frames over the shared synchronous walker.

use std::{convert::Infallible, fmt};

use leselang_runtime_core::{
    AcceptedReply, CalculationFailure, Fuel, LocalExecutionScope, ScopeFrame, StructureBudget,
    with_owned_scope_state,
};

use crate::effect_evaluation::{
    ControlOutcome, EffectEvaluationEnvironment, EffectEvaluationFailure, EffectEvaluationFault,
    EffectEvaluationLimits, SuspensionProfile, evaluate_with_profile, preflight, preflight_into,
    preflight_scope,
};
use crate::effect_reentry::{
    EffectContinuation, EffectReentryEnvironment, bind_effect_value,
    restore_effect_scope_with_liveness,
};
use crate::ir::Computation;
use crate::local_scope::CallFence;
use crate::pure_evaluation::{PureEvaluationEnvironment, PureEvaluationFault, PureValue};
use crate::pure_typing::{MAX_TYPE_INFERENCE_DEPTH, valid_local_name};

pub const MAX_EFFECT_RETURN_FRAMES: usize = MAX_TYPE_INFERENCE_DEPTH;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EffectStackLimits {
    pub effects: EffectEvaluationLimits,
    /// Pending binding return sites, separately bounded from active lexical locals.
    pub max_frames: usize,
}

/// Core-owned binding sites in innermost-to-outermost return order, captured live
/// or restored from explicitly host-admitted saved data. Captures remain native,
/// move-only and nonportable. Inspection/extraction supplies neither reply
/// authority nor durable provenance.
/// The host must bound snapshot work/bytes across all frames, not just each capture.
///
/// ```compile_fail
/// use leselang_hir::effect_stack::EffectReturnStack;
/// fn copy(stack: EffectReturnStack<'_, (), ()>) { let copied = stack.clone(); }
/// ```
///
/// ```compile_fail
/// use leselang_hir::effect_stack::EffectReturnStack;
/// fn wire(stack: EffectReturnStack<'_, (), ()>) {
///     let encoded = serde_json::to_string(&stack);
/// }
/// ```
#[must_use = "retain every original return frame until acceptance or cancellation"]
pub struct EffectReturnStack<'expression, Node: ?Sized, Capture> {
    frames: Vec<EffectContinuation<'expression, Node, Capture>>,
}
impl<'expression, Node: ?Sized, Capture> EffectReturnStack<'expression, Node, Capture> {
    pub fn frame_count(&self) -> usize {
        self.frames.len()
    }
    pub fn frames(&self) -> &[EffectContinuation<'expression, Node, Capture>] {
        &self.frames
    }
    pub fn into_frames(self) -> Vec<EffectContinuation<'expression, Node, Capture>> {
        self.frames
    }
}
impl<Node: ?Sized, Capture> fmt::Debug for EffectReturnStack<'_, Node, Capture> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EffectReturnStack")
            .field("frames", &self.frame_count())
            .finish()
    }
}

/// Unchecked host-decoded return data, not a live frame, receipt or authority.
/// Bodies/names borrow the admitted snapshot; captures move without cloning.
/// Input order is innermost to outermost. The host must authenticate that order,
/// each original site and capture, and bound decoding/aggregate native storage.
pub struct SavedEffectReturnFrame<'expression, Node: ?Sized, Capture> {
    pub name: &'expression str,
    pub body: &'expression Node,
    pub capture: Capture,
}
impl<Node: ?Sized, Capture> fmt::Debug for SavedEffectReturnFrame<'_, Node, Capture> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SavedEffectReturnFrame")
    }
}

pub type EffectStackRestoreResult<'expression, Node, Capture> =
    Result<EffectReturnStack<'expression, Node, Capture>, EffectStackFailure<Infallible>>;

/// Rebuild bounded return sites from host-admitted saved data without executing.
/// Frame count, names and the complete body forest share the original structural
/// limits. Each lexical frame is independent; equal names across frames are valid.
/// No native callback, capture copy, effect preparation, fuel grant or reply is
/// created. Rebuilt stacks still require PendingReply acceptance and the existing
/// resume_accepted_stacked_effects entry, which rechecks all cold effects before
/// restoring any prefix or projecting the accepted reply once.
///
/// This is structural admission only. The host must first validate the original
/// stored graph/site correspondence, schema, captures and saved resource budget,
/// then enforce live authority, authentic replies, settlement and cancellation.
/// A caller can submit different well-formed data; success does not authenticate
/// it. This supplies no codec, journal, migration, replay fence or automatic retry.
/// Owned inputs are consumed on refusal; native Drop/unwind remains host-owned.
pub fn restore_effect_return_stack<'expression, Field, Operation, HostEffect, IrResult, Capture>(
    frames: Vec<
        SavedEffectReturnFrame<
            'expression,
            Computation<Field, Operation, HostEffect, IrResult>,
            Capture,
        >,
    >,
    policy: EffectStackLimits,
) -> EffectStackRestoreResult<
    'expression,
    Computation<Field, Operation, HostEffect, IrResult>,
    Capture,
> {
    limits(policy)?;
    if frames.is_empty() || frames.len() > policy.max_frames {
        return Err(EffectStackFault::ReturnLimit.into());
    }
    let mut empty = Vec::<(&str, PureValue<()>)>::new();
    preflight_scope(&ScopeFrame::new(&mut empty), policy.effects).map_err(stack_failure)?;
    let mut budget =
        StructureBudget::new(policy.effects.pure.max_nodes, policy.effects.pure.max_depth);
    let mut effects = Vec::new();
    for frame in &frames {
        if !valid_local_name(frame.name) {
            return Err(EffectStackFault::Control(EffectEvaluationFault::Pure(
                PureEvaluationFault::InvalidScope,
            ))
            .into());
        }
        preflight_into(frame.body, policy.effects, &mut budget, &mut effects)
            .map_err(stack_failure)?;
    }
    if policy.effects.pure.max_bindings == 0 {
        return Err(EffectStackFault::Control(EffectEvaluationFault::Pure(
            PureEvaluationFault::BindingLimit,
        ))
        .into());
    }
    Ok(EffectReturnStack {
        frames: frames
            .into_iter()
            .map(|frame| EffectContinuation::new(frame.name, frame.body, frame.capture))
            .collect(),
    })
}

#[must_use = "route the value, prepared request or complete return-frame stack"]
pub enum StackedEffectOutcome<'expression, Node: ?Sized, Value, Request, Capture> {
    Value(PureValue<Value>),
    Request(Request),
    Suspended {
        request: Request,
        continuation: EffectReturnStack<'expression, Node, Capture>,
    },
}
impl<Node: ?Sized, Value, Request, Capture> fmt::Debug
    for StackedEffectOutcome<'_, Node, Value, Request, Capture>
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Value(_) => "Value",
            Self::Request(_) => "Request",
            Self::Suspended { .. } => "Suspended",
        })
    }
}

pub enum EffectStackFault<Native> {
    InvalidLimits,
    ReturnLimit,
    Control(EffectEvaluationFault<Native>),
}
impl<Native> fmt::Display for EffectStackFault<Native> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidLimits => f.write_str("effect return-frame limits exceed safety ceilings"),
            Self::ReturnLimit => f.write_str("effect pending return-frame limit exceeded"),
            Self::Control(error) => fmt::Display::fmt(error, f),
        }
    }
}
impl<Native> fmt::Debug for EffectStackFault<Native> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}
impl<Native> std::error::Error for EffectStackFault<Native> {}
pub type EffectStackFailure<Native> = CalculationFailure<EffectStackFault<Native>>;
pub type AcceptedEffectStackContinuation<
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
    EffectReturnStack<'expression, Computation<Field, Operation, HostEffect, IrResult>, Capture>,
    Declaration,
    Reply,
>;
type Node<Field, Operation, HostEffect, IrResult> =
    Computation<Field, Operation, HostEffect, IrResult>;
pub type StackedControlResult<'expression, Field, Operation, HostEffect, IrResult, Environment> =
    Result<
        StackedEffectOutcome<
            'expression,
            Node<Field, Operation, HostEffect, IrResult>,
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
        >,
        EffectStackFailure<<Environment as PureEvaluationEnvironment<Field, Operation>>::Error>,
    >;
type Outcome<'e, Node, Value, Request, Capture> =
    ControlOutcome<Value, Request, EffectReturnStack<'e, Node, Capture>>;

struct ReturnFrames;
impl<'expression, Node: ?Sized + 'expression, Capture> SuspensionProfile<'expression, Node, Capture>
    for ReturnFrames
{
    type Frame = EffectReturnStack<'expression, Node, Capture>;
    fn admits(frame: Option<&Self::Frame>, max_frames: usize) -> bool {
        frame.map_or(0, EffectReturnStack::frame_count) < max_frames
    }
    fn append(
        frame: Option<Self::Frame>,
        next: EffectContinuation<'expression, Node, Capture>,
    ) -> Result<Self::Frame, ()> {
        let mut frame = frame.unwrap_or_else(|| EffectReturnStack { frames: Vec::new() });
        frame.frames.push(next);
        Ok(frame)
    }
}
pub(crate) fn stack_failure<Native>(
    error: EffectEvaluationFailure<Native>,
) -> EffectStackFailure<Native> {
    match error {
        CalculationFailure::Scalar(error) => CalculationFailure::Scalar(error),
        // This profile admits nesting; its only shared profile refusal is capacity.
        CalculationFailure::External(EffectEvaluationFault::NestedSuspension) => {
            EffectStackFault::ReturnLimit.into()
        }
        CalculationFailure::External(error) => EffectStackFault::Control(error).into(),
    }
}

impl<'expression, Node: ?Sized, Value, Request, Capture>
    From<StackedEffectOutcome<'expression, Node, Value, Request, Capture>>
    for ControlOutcome<Value, Request, EffectReturnStack<'expression, Node, Capture>>
{
    fn from(outcome: StackedEffectOutcome<'expression, Node, Value, Request, Capture>) -> Self {
        match outcome {
            StackedEffectOutcome::Value(value) => Self::Value(value),
            StackedEffectOutcome::Request(request) => Self::Request(request),
            StackedEffectOutcome::Suspended {
                request,
                continuation,
            } => Self::Suspended {
                request,
                continuation,
            },
        }
    }
}
fn output<'e, Node, Value, Request, Capture>(
    outcome: Outcome<'e, Node, Value, Request, Capture>,
) -> StackedEffectOutcome<'e, Node, Value, Request, Capture> {
    match outcome {
        ControlOutcome::Value(value) => StackedEffectOutcome::Value(value),
        ControlOutcome::Request(request) => StackedEffectOutcome::Request(request),
        ControlOutcome::Suspended {
            request,
            continuation,
        } => StackedEffectOutcome::Suspended {
            request,
            continuation,
        },
    }
}
fn limits<Native>(limits: EffectStackLimits) -> Result<(), EffectStackFailure<Native>> {
    if limits.max_frames > MAX_EFFECT_RETURN_FRAMES {
        Err(EffectStackFault::InvalidLimits.into())
    } else {
        Ok(())
    }
}

/// Evaluate with explicit nested binding returns, never dispatching prepared work.
/// Shared complete cold preflight, lexical execution, native preparation/capture
/// and fuel rules are unchanged. A frame quota refusal precedes that frame's capture
/// hook, but earlier preparation/captures may have occurred and are not rolled back.
/// Zero frames allows pure/final-request execution, not a suspended binding.
/// Native capture must retain exactly its live lexical prefix and preserve framework
/// affinity; no implicit local copying, scheduler, recursive helper or wire is added.
pub fn evaluate_stacked_effects_in_scope<
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
    policy: EffectStackLimits,
) -> StackedControlResult<'expression, Field, Operation, HostEffect, IrResult, Environment>
where
    Environment: EffectEvaluationEnvironment<'expression, Field, Operation, HostEffect, IrResult>,
{
    limits(policy)?;
    let effects = preflight(expression, scope, policy.effects).map_err(stack_failure)?;
    for effect in effects {
        environment
            .preflight_effect(effect)
            .map_err(|error| EffectStackFault::Control(EffectEvaluationFault::Native(error)))?;
    }
    evaluate_with_profile::<_, _, _, _, _, ReturnFrames>(
        expression,
        scope,
        environment,
        fuel,
        policy.effects.pure.max_bindings,
        policy.max_frames,
    )
    .map(output)
    .map_err(stack_failure)
}

/// Consume one accepted native reply, then unwind original logical returns.
/// The complete pending body forest shares one physical budget and all cold current
/// schema checks before any restoration/projection. Only the innermost frame projects
/// the accepted native reply; outer frames bind the already computed value, not a
/// fabricated reply/domain. Each prefix restores through the existing native hook.
/// Restoration must validate the capture's own provenance/current view independently
/// of the latest accepted leaf result domain; that domain is not an outer value type.
/// Newly suspended frames and still pending outer frames share one inclusive quota.
/// Final requests acquire the next original return site without replaying old source.
/// Every error/unwind consumes owned frames; cancellation uses the same PendingReply.
/// Outputs/errors stay local through owned prefix, remaining frame/input and
/// identity cleanup; callback-internal cleanup remains native responsibility.
/// Authentication, current capture integrity, aggregate native costs and dispatch
/// remain host-owned. This is in-memory binding composition, not nested group-member
/// scheduling, a durable VM, external exactly-once execution or product migration.
pub fn resume_accepted_stacked_effects<
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
    accepted: AcceptedEffectStackContinuation<
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
    policy: EffectStackLimits,
) -> StackedControlResult<'expression, Field, Operation, HostEffect, IrResult, Environment>
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
    resume_accepted_stacked_effects_with_scope(accepted, environment, fuel, policy, None)
}

pub(crate) fn resume_accepted_stacked_effects_with_scope<
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
    accepted: AcceptedEffectStackContinuation<
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
    policy: EffectStackLimits,
    execution_scope: Option<&LocalExecutionScope>,
) -> StackedControlResult<'expression, Field, Operation, HostEffect, IrResult, Environment>
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
    let mut frames = continuation.frames.into_iter();
    let mut current = None;
    let mut reply = Some(reply);
    let mut returned = None;
    let fence = CallFence::new(execution_scope);
    let output = (|| {
        limits(policy)?;
        if frames.len() == 0 || frames.len() > policy.max_frames {
            return Err(EffectStackFault::ReturnLimit.into());
        }
        let mut empty = Vec::<(&str, PureValue<Environment::Result>)>::new();
        preflight_scope(&ScopeFrame::new(&mut empty), policy.effects).map_err(stack_failure)?;
        let mut budget =
            StructureBudget::new(policy.effects.pure.max_nodes, policy.effects.pure.max_depth);
        let mut effects = Vec::new();
        for frame in frames.as_slice() {
            preflight_into(frame.body(), policy.effects, &mut budget, &mut effects)
                .map_err(stack_failure)?;
        }
        if policy.effects.pure.max_bindings == 0 {
            return Err(EffectStackFault::Control(EffectEvaluationFault::Pure(
                crate::pure_evaluation::PureEvaluationFault::BindingLimit,
            ))
            .into());
        }
        for effect in effects {
            environment
                .preflight_effect(effect)
                .map_err(|error| EffectStackFault::Control(EffectEvaluationFault::Native(error)))?;
        }
        current = frames.next();
        loop {
            let Some(frame) = current.take() else {
                return Err(EffectStackFault::ReturnLimit.into());
            };
            let (name, body, capture) = frame.into_parts();
            let bindings = restore_effect_scope_with_liveness(
                name,
                |fuel| {
                    environment.restore_capture(
                        &identity,
                        declaration,
                        capture,
                        fuel,
                        policy.effects,
                    )
                },
                fuel,
                policy.effects,
                execution_scope,
            )
            .map_err(stack_failure)?;
            let outcome = with_owned_scope_state(bindings, fuel, |scope, fuel| {
                CallFence::run(execution_scope, || {
                    let value = if let Some(reply) = reply.take() {
                        environment
                            .bind_reply(&identity, declaration, reply, fuel)
                            .map_err(crate::effect_evaluation::native_failure)
                            .map_err(stack_failure)?
                    } else if let Some(value) = returned.take() {
                        value
                    } else {
                        return Err(EffectStackFault::ReturnLimit.into());
                    };
                    bind_effect_value(name, value, scope, policy.effects).map_err(stack_failure)?;
                    evaluate_with_profile::<_, _, _, _, _, ReturnFrames>(
                        body,
                        scope,
                        environment,
                        fuel,
                        policy.effects.pure.max_bindings,
                        policy
                            .max_frames
                            .checked_sub(frames.len())
                            .ok_or(EffectStackFault::ReturnLimit)?,
                    )
                    .map_err(stack_failure)
                })
            })?;
            match outcome {
                ControlOutcome::Value(value) => {
                    if let Some(next) = frames.next() {
                        current = Some(next);
                        returned = Some(value);
                    } else {
                        return Ok(StackedEffectOutcome::Value(value));
                    }
                }
                ControlOutcome::Request(request) => {
                    if frames.len() == 0 {
                        return Ok(StackedEffectOutcome::Request(request));
                    }
                    return Ok(StackedEffectOutcome::Suspended {
                        request,
                        continuation: EffectReturnStack {
                            frames: frames.by_ref().collect(),
                        },
                    });
                }
                ControlOutcome::Suspended {
                    request,
                    mut continuation,
                } => {
                    continuation.frames.extend(frames.by_ref());
                    return Ok(StackedEffectOutcome::Suspended {
                        request,
                        continuation,
                    });
                }
            }
        }
    })();
    // Place the fence after output so cleanup unwind revokes before output disposal.
    let cleanup_fence = fence;
    if output.is_err() {
        cleanup_fence.close();
    }
    // Original native owners stay outside the fallible closure's return place.
    drop((current, frames, reply, returned, identity));
    cleanup_fence.finish();
    output
}
