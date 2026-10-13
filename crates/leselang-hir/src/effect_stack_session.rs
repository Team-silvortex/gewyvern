//! Opt-in nested binding sessions over the original reply-gated lifecycle.

use std::{borrow::Borrow, fmt};

use leselang_runtime_core::{Fuel, HostResultDomain, LocalExecutionScope, ReplyAuthority};

use crate::effect_evaluation::ControlOutcome;
use crate::effect_reentry::RestoredEffectBindings;
use crate::effect_session::{
    EffectSessionAcceptance, EffectSessionEnvironment, EffectSessionPoll, EffectSessionStatus,
    HostWorkOutcome, SessionCore,
};
use crate::effect_stack::{
    EffectReturnStack, EffectStackFailure, EffectStackLimits, evaluate_stacked_effects_in_scope,
    resume_accepted_stacked_effects_with_scope, stack_failure,
};
use crate::ir::Computation;

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
