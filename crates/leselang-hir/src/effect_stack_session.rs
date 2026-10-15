//! Opt-in nested binding sessions over the original reply-gated lifecycle.

use std::{borrow::Borrow, convert::Infallible, fmt};

use leselang_runtime_core::{Fuel, HostResultDomain, LocalExecutionScope, ReplyAuthority};

use crate::effect_evaluation::ControlOutcome;
use crate::effect_reentry::RestoredEffectBindings;
use crate::effect_session::{
    EffectSessionAcceptance, EffectSessionEnvironment, EffectSessionPoll, EffectSessionStatus,
    HostWorkOutcome, SessionCore,
};
use crate::effect_stack::{
    EffectReturnStack, EffectStackFailure, EffectStackLimits, SavedEffectReturnFrame,
    evaluate_stacked_effects_in_scope, preflight_saved_return_frames,
    resume_accepted_stacked_effects_with_scope, return_stack_from_admitted_frames, stack_failure,
};
use crate::ir::Computation;
use crate::local_scope::CallFence;

/// Structural or native admission failure, never an execution/receipt certificate.
pub enum StackedSessionRestoreError<Native> {
    Structure(EffectStackFailure<Infallible>),
    Native(Native),
    ScopeClosed,
}
impl<Native> fmt::Display for StackedSessionRestoreError<Native> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Structure(_) => "saved return stack structure rejected",
            Self::Native(_) => "native pending-session admission rejected",
            Self::ScopeClosed => "pending-session restoration scope is closed",
        })
    }
}
impl<Native> fmt::Debug for StackedSessionRestoreError<Native> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}
impl<Native> std::error::Error for StackedSessionRestoreError<Native> {}

/// One immutable-policy, owned-fuel execution with bounded nested return frames.
/// Uses the same acceptance, terminal fencing and once-only handoff state machine
/// as `ControlSession`; only the explicitly selected evaluator profile differs.
/// New/acceptance/idle polling never executes a successor. Cancellation closes
/// before all pending/accepted frame and input cleanup, retaining Cancelled on
/// cleanup unwind. Execution/acceptance unwind seals HostUncertain; ordinary
/// execution errors close Failed without refill.
/// Native IR/capture/identity/reply need no Clone/Debug/serde/Send/Sync bounds.
///
/// Limits and root fuel cannot be replaced after construction. Hosts still bound
/// aggregate captures, ingress and native work, validate each capture's own live
/// provenance, and own dispatch and cancellation delivery. This is a borrowed
/// in-memory session, not a group scheduler, durable VM or external exactly-once
/// fence. Native callbacks/Drop are trusted and cannot be rolled back/preempted;
/// a second panic during unwind may abort. Existing single-frame APIs stay unchanged.
///
/// ```compile_fail
/// use leselang_hir::effect_stack_session::StackedControlSession;
/// fn copy(session: StackedControlSession<'_, '_, (), (), (), u8, (), ()>) {
///     let copied = session.clone();
/// }
/// ```
///
/// ```compile_fail
/// use leselang_hir::effect_stack_session::StackedControlSession;
/// fn wire(session: StackedControlSession<'_, '_, (), (), (), u8, (), ()>) {
///     let encoded = serde_json::to_string(&session);
/// }
/// ```
#[must_use = "retain the nested control session until completion or cancellation"]
pub struct StackedControlSession<
    'expression,
    'schema,
    Node: ?Sized,
    Value,
    Capture,
    Identity,
    Declaration: ?Sized,
    Reply,
> {
    core: SessionCore<
        'expression,
        'schema,
        Node,
        Value,
        EffectReturnStack<'expression, Node, Capture>,
        Identity,
        Declaration,
        Reply,
    >,
    limits: EffectStackLimits,
}
pub type StackedEffectSession<
    'expression,
    'schema,
    Field,
    Operation,
    HostEffect,
    IrResult,
    Value,
    Capture,
    Identity,
    Declaration,
    Reply,
> = StackedControlSession<
    'expression,
    'schema,
    Computation<Field, Operation, HostEffect, IrResult>,
    Value,
    Capture,
    Identity,
    Declaration,
    Reply,
>;
pub type StackedEffectSessionPollResult<Value, Dispatch, Native> =
    Result<EffectSessionPoll<Value, Dispatch>, EffectStackFailure<Native>>;

impl<'expression, 'schema, Node: ?Sized, Value, Capture, Identity, Declaration: ?Sized, Reply>
    StackedControlSession<'expression, 'schema, Node, Value, Capture, Identity, Declaration, Reply>
{
    /// Store the original borrowed IR and move initial bindings/fuel/policy.
    /// First poll admits the whole cold tree; construction performs no host work.
    pub fn new(
        expression: &'expression Node,
        bindings: RestoredEffectBindings<'expression, Value>,
        fuel: Fuel,
        limits: EffectStackLimits,
    ) -> Self {
        Self {
            core: SessionCore::new(expression, bindings, fuel),
            limits,
        }
    }
    pub fn status(&self) -> EffectSessionStatus {
        self.core.status()
    }
    pub const fn fuel_remaining(&self) -> u64 {
        self.core.fuel_remaining()
    }
    pub fn cancel(&mut self) -> bool {
        self.core.cancel()
    }

    pub(crate) fn with_host_work<Input, Output>(
        &mut self,
        input: Input,
        work: impl FnOnce(Input) -> HostWorkOutcome<Output>,
    ) -> Result<Output, Input> {
        self.core.with_host_work(input, work)
    }

    /// Shared original acceptance; success retains every frame until explicit poll.
    pub fn try_accept<View: ?Sized, Authority>(
        &mut self,
        identity: &Identity,
        reply: Reply,
        authority: &Authority,
    ) -> EffectSessionAcceptance<Reply, Authority::Error, Declaration::Error>
    where
        Identity: PartialEq,
        Reply: Borrow<View>,
        Declaration: HostResultDomain<View>,
        Authority: ReplyAuthority<Identity>,
    {
        self.core.try_accept(identity, reply, authority)
    }

    pub(crate) fn try_accept_in_scope<View: ?Sized, Authority>(
        &mut self,
        identity: &Identity,
        reply: Reply,
        authority: &Authority,
        scope: &LocalExecutionScope,
    ) -> EffectSessionAcceptance<Reply, Authority::Error, Declaration::Error>
    where
        Identity: PartialEq,
        Reply: Borrow<View>,
        Declaration: HostResultDomain<View>,
        Authority: ReplyAuthority<Identity>,
    {
        self.core
            .try_accept_with_scope(identity, reply, authority, Some(scope))
    }
}
impl<Node: ?Sized, Value, Capture, Identity, Declaration: ?Sized, Reply> fmt::Debug
    for StackedControlSession<'_, '_, Node, Value, Capture, Identity, Declaration, Reply>
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StackedControlSession")
            .field("status", &self.status())
            .field("fuel_remaining", &self.fuel_remaining())
            .finish()
    }
}
impl<
    'expression,
    'schema,
    Field,
    Operation,
    HostEffect,
    IrResult,
    Value,
    Capture,
    Identity,
    Declaration: ?Sized,
    Reply,
>
    StackedEffectSession<
        'expression,
        'schema,
        Field,
        Operation,
        HostEffect,
        IrResult,
        Value,
        Capture,
        Identity,
        Declaration,
        Reply,
    >
{
    /// Install host-decoded pending binding returns into the original lifecycle.
    /// Whole frame/body structure is checked before the mandatory native hook.
    /// The hook borrows original identity, domain, ordered frames and remaining
    /// fuel; it must validate committed correspondence, captures, live policy,
    /// budget and nonterminal settlement. Success starts Awaiting, with no
    /// preparation, dispatch, reply acceptance, fuel charge or body execution.
    /// Later acceptance/poll/cancel use the unchanged session and cold preflight.
    ///
    /// Inputs are consumed on refusal; native captures/keys/errors need no Clone,
    /// Debug, serde or Send bounds. Native callbacks and Drop remain trusted.
    /// This is not a codec, authenticator, replay fence, crash settlement or a way
    /// to rearm cancelled work. The host bounds ingress and aggregate storage and
    /// must not treat structural admission or its own hook as proof of provenance.
    pub fn restore_pending<Error>(
        identity: Identity,
        declaration: &'schema Declaration,
        frames: Vec<
            SavedEffectReturnFrame<
                'expression,
                Computation<Field, Operation, HostEffect, IrResult>,
                Capture,
            >,
        >,
        fuel: Fuel,
        limits: EffectStackLimits,
        admit: impl FnOnce(
            &Identity,
            &'schema Declaration,
            &[SavedEffectReturnFrame<
                'expression,
                Computation<Field, Operation, HostEffect, IrResult>,
                Capture,
            >],
            &Fuel,
        ) -> Result<(), Error>,
    ) -> Result<Self, StackedSessionRestoreError<Error>> {
        Self::restore_pending_with_scope(identity, declaration, frames, fuel, limits, admit, None)
    }

    pub(crate) fn restore_pending_with_scope<Error>(
        identity: Identity,
        declaration: &'schema Declaration,
        frames: Vec<
            SavedEffectReturnFrame<
                'expression,
                Computation<Field, Operation, HostEffect, IrResult>,
                Capture,
            >,
        >,
        fuel: Fuel,
        limits: EffectStackLimits,
        admit: impl FnOnce(
            &Identity,
            &'schema Declaration,
            &[SavedEffectReturnFrame<
                'expression,
                Computation<Field, Operation, HostEffect, IrResult>,
                Capture,
            >],
            &Fuel,
        ) -> Result<(), Error>,
        execution_scope: Option<&LocalExecutionScope>,
    ) -> Result<Self, StackedSessionRestoreError<Error>> {
        let fence = CallFence::new(execution_scope);
        let preflight = if execution_scope.is_some_and(|scope| !scope.is_open()) {
            Err(StackedSessionRestoreError::ScopeClosed)
        } else {
            preflight_saved_return_frames(&frames, limits)
                .map_err(StackedSessionRestoreError::Structure)
        };
        if let Err(error) = preflight {
            fence.close();
            let output = Err(error);
            drop((admit, frames, identity, fuel));
            fence.finish();
            return output;
        }
        let admitted = admit(&identity, declaration, &frames, &fuel);
        let admitted = if execution_scope.is_some_and(|scope| !scope.is_open()) {
            drop(admitted);
            Err(StackedSessionRestoreError::ScopeClosed)
        } else {
            admitted.map_err(StackedSessionRestoreError::Native)
        };
        if let Err(error) = admitted {
            fence.close();
            let output = Err(error);
            drop((frames, identity, fuel));
            fence.finish();
            return output;
        }
        let output = Ok(Self {
            core: SessionCore::from_pending(
                identity,
                return_stack_from_admitted_frames(frames),
                declaration,
                fuel,
            ),
            limits,
        });
        fence.finish();
        output
    }

    /// Execute one ready slice using the original session-owned root meter.
    /// Correlation still rechecks policy and publishes prepared work, not dispatch.
    pub fn poll<Environment>(
        &mut self,
        environment: &Environment,
    ) -> StackedEffectSessionPollResult<Value, Environment::Dispatch, Environment::Error>
    where
        Environment: EffectSessionEnvironment<
                'expression,
                'schema,
                Field,
                Operation,
                HostEffect,
                IrResult,
                Result = Value,
                Capture = Capture,
                Identity = Identity,
                Declaration = Declaration,
                Reply = Reply,
            >,
    {
        self.poll_with_scope(environment, None)
    }

    pub(crate) fn poll_with_scope<Environment>(
        &mut self,
        environment: &Environment,
        execution_scope: Option<&LocalExecutionScope>,
    ) -> StackedEffectSessionPollResult<Value, Environment::Dispatch, Environment::Error>
    where
        Environment: EffectSessionEnvironment<
                'expression,
                'schema,
                Field,
                Operation,
                HostEffect,
                IrResult,
                Result = Value,
                Capture = Capture,
                Identity = Identity,
                Declaration = Declaration,
                Reply = Reply,
            >,
    {
        let limits = self.limits;
        self.core.poll_with(
            environment,
            |expression, scope, fuel| {
                evaluate_stacked_effects_in_scope(expression, scope, environment, fuel, limits)
                    .map(ControlOutcome::from)
            },
            |accepted, fuel| {
                resume_accepted_stacked_effects_with_scope(
                    accepted,
                    environment,
                    fuel,
                    limits,
                    execution_scope,
                )
                .map(ControlOutcome::from)
            },
            stack_failure,
            execution_scope,
        )
    }
}
