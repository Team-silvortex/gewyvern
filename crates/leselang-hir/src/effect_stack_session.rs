//! Opt-in nested binding sessions over the original reply-gated lifecycle.

use std::{borrow::Borrow, convert::Infallible, fmt};

use leselang_runtime_core::{Fuel, HostResultDomain, LocalExecutionScope, ReplyAuthority};

use crate::effect_evaluation::ControlOutcome;
use crate::effect_reentry::RestoredEffectBindings;
use crate::effect_session::{
    EffectSessionAcceptance, EffectSessionEnvironment, EffectSessionPoll, EffectSessionStatus,
    HostWorkOutcome, PendingSessionParts, SessionCore,
};
use crate::effect_stack::{
    EffectReturnStack, EffectStackFailure, EffectStackLimits, SavedEffectReturnFrame,
    evaluate_stacked_effects_in_scope, preflight_final_return_limits,
    preflight_saved_return_frames, resume_accepted_stacked_effects_with_scope,
    return_stack_from_admitted_frames, stack_failure,
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

/// Original pending binding state moved out of a session, not a durable image.
/// Convert the original continuation frames into host-owned saved frames and
/// use `StackedControlSession::restore_pending` with native admission to resume.
/// The original meter and limits are retained; fields are not authenticated.
/// Hosts own codecs, storage, settlement, live policy and cancellation delivery.
/// Dropping this value releases native owners, but does not cancel external work.
///
/// ```compile_fail
/// use leselang_hir::effect_stack_session::PendingStackedBinding;
/// fn duplicate(saved: PendingStackedBinding<'_, '_, (), (), u8, ()>) {
///     let copy = saved.clone();
/// }
/// ```
///
/// ```compile_fail
/// use leselang_hir::effect_stack_session::PendingStackedBinding;
/// fn wire(saved: PendingStackedBinding<'_, '_, (), (), u8, ()>) {
///     let encoded = serde_json::to_string(&saved);
/// }
/// ```
#[must_use = "persist, restore or explicitly dispose of the original pending state"]
pub struct PendingStackedBinding<
    'expression,
    'schema,
    Node: ?Sized,
    Capture,
    Identity,
    Declaration: ?Sized,
> {
    pub identity: Identity,
    pub continuation: EffectReturnStack<'expression, Node, Capture>,
    pub declaration: &'schema Declaration,
    pub fuel: Fuel,
    pub limits: EffectStackLimits,
}

impl<Node: ?Sized, Capture, Identity, Declaration: ?Sized> fmt::Debug
    for PendingStackedBinding<'_, '_, Node, Capture, Identity, Declaration>
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("PendingStackedBinding")
    }
}

/// Original final-request ownership, with no binding return stack or receipt.
/// Fields move without cloning and remain unauthenticated host data. Restoring
/// requires native admission of the original request, domain, budget and live
/// nonterminal settlement. This is not a codec or a permission to redispatch.
///
/// ```compile_fail
/// use leselang_hir::effect_stack_session::PendingStackedFinal;
/// fn duplicate(saved: PendingStackedFinal<'_, u8, ()>) { let copy = saved.clone(); }
/// ```
///
/// ```compile_fail
/// use leselang_hir::effect_stack_session::PendingStackedFinal;
/// fn wire(saved: PendingStackedFinal<'_, u8, ()>) {
///     let encoded = serde_json::to_string(&saved);
/// }
/// ```
#[must_use = "restore or explicitly dispose of the original final-request ownership"]
pub struct PendingStackedFinal<'schema, Identity, Declaration: ?Sized> {
    pub identity: Identity,
    pub declaration: &'schema Declaration,
    pub fuel: Fuel,
    pub limits: EffectStackLimits,
}

impl<Identity, Declaration: ?Sized> fmt::Debug for PendingStackedFinal<'_, Identity, Declaration> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("PendingStackedFinal")
    }
}

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

    /// Revoke the coordinator's scope before releasing retained state.
    /// Existing terminal reasons remain sticky, including cleanup unwind.
    /// This borrows the scope; it does not cancel external host work.
    pub fn cancel_in_scope(&mut self, scope: &LocalExecutionScope) -> bool {
        scope.close();
        self.cancel()
    }

    pub(crate) fn call_in_scope<Output>(
        &mut self,
        scope: &LocalExecutionScope,
        step: impl FnOnce(&mut Self, &LocalExecutionScope) -> Output,
    ) -> Output {
        if !scope.is_open() {
            self.cancel();
        }
        let fence = CallFence::new(Some(scope));
        let output = step(self, scope);
        if matches!(self.status(), EffectSessionStatus::Terminal(_)) {
            scope.close();
        }
        fence.finish();
        output
    }

    /// Consume only a waiting binding session and move its original owners,
    /// remaining meter and immutable policy out without executing native code.
    /// Initial, final-request, accepted and terminal sessions return unchanged.
    /// There is no live session left after success, no accepted receipt and no
    /// fuel refill. This is an in-memory handoff, not commit, replay permission
    /// or external cancellation. `LocalStackedControlSession` deliberately has
    /// no such escape: its local scope must remain owned until closure.
    pub fn try_into_pending_binding(
        self,
    ) -> Result<
        PendingStackedBinding<'expression, 'schema, Node, Capture, Identity, Declaration>,
        Self,
    > {
        let Self { core, limits } = self;
        match core.try_into_pending_binding() {
            Ok(PendingSessionParts {
                identity,
                continuation,
                declaration,
                fuel,
            }) => Ok(PendingStackedBinding {
                identity,
                continuation,
                declaration,
                fuel,
                limits,
            }),
            Err(core) => Err(Self { core, limits }),
        }
    }

    /// Consume only a waiting final request, preserving its exact owners and
    /// accounting without accepting a reply, executing code or allocating.
    /// Binding, initial, accepted and terminal sessions return unchanged.
    /// External cancellation, authentic storage and settlement remain host-owned.
    pub fn try_into_pending_final(
        self,
    ) -> Result<PendingStackedFinal<'schema, Identity, Declaration>, Self> {
        let Self { core, limits } = self;
        match core.try_into_pending_final() {
            Ok(PendingSessionParts {
                identity,
                declaration,
                fuel,
                continuation: (),
            }) => Ok(PendingStackedFinal {
                identity,
                declaration,
                fuel,
                limits,
            }),
            Err(core) => Err(Self { core, limits }),
        }
    }

    /// Restore a final request through the original acceptance/projection path.
    /// Policy ceilings precede mandatory native admission; zero return frames and
    /// zero remaining fuel are allowed, never repaired. The host must prove the
    /// original final-request/domain/budget correspondence, current authority and
    /// nonterminal settlement. There is no saved body to execute or redispatch.
    /// Success starts Awaiting; acceptance only marks Ready and explicit poll
    /// projects the reply once using the original meter and host policy.
    pub fn restore_pending_final<Error>(
        identity: Identity,
        declaration: &'schema Declaration,
        fuel: Fuel,
        limits: EffectStackLimits,
        admit: impl FnOnce(&Identity, &'schema Declaration, &Fuel) -> Result<(), Error>,
    ) -> Result<Self, StackedSessionRestoreError<Error>> {
        Self::restore_pending_final_with_scope(identity, declaration, fuel, limits, admit, None)
    }

    /// Restore a final request under a coordinator's existing local fence.
    ///
    /// Like `restore_pending_in_scope`, this borrows but does not retain the
    /// scope. Refusal, native revocation or escaping unwind closes it before
    /// retained inputs are released. Success preserves the original final
    /// request and meter, without redispatch or invented return frames. Native
    /// admission and callback-internal cleanup remain trusted. Use the same scope
    /// for scoped acceptance, polling and cancellation; this raw session does not
    /// remember it or cancel external collectors.
    pub fn restore_pending_final_in_scope<Error>(
        identity: Identity,
        declaration: &'schema Declaration,
        fuel: Fuel,
        limits: EffectStackLimits,
        admit: impl FnOnce(&Identity, &'schema Declaration, &Fuel) -> Result<(), Error>,
        scope: &LocalExecutionScope,
    ) -> Result<Self, StackedSessionRestoreError<Error>> {
        Self::restore_pending_final_with_scope(
            identity,
            declaration,
            fuel,
            limits,
            admit,
            Some(scope),
        )
    }

    pub(crate) fn restore_pending_final_with_scope<Error>(
        identity: Identity,
        declaration: &'schema Declaration,
        fuel: Fuel,
        limits: EffectStackLimits,
        admit: impl FnOnce(&Identity, &'schema Declaration, &Fuel) -> Result<(), Error>,
        execution_scope: Option<&LocalExecutionScope>,
    ) -> Result<Self, StackedSessionRestoreError<Error>> {
        let fence = CallFence::new(execution_scope);
        let preflight = if execution_scope.is_some_and(|scope| !scope.is_open()) {
            Err(StackedSessionRestoreError::ScopeClosed)
        } else {
            preflight_final_return_limits(limits).map_err(StackedSessionRestoreError::Structure)
        };
        if let Err(error) = preflight {
            fence.close();
            let output = Err(error);
            drop((admit, identity, fuel));
            fence.finish();
            return output;
        }
        let admitted = admit(&identity, declaration, &fuel);
        let admitted = if execution_scope.is_some_and(|scope| !scope.is_open()) {
            drop(admitted);
            Err(StackedSessionRestoreError::ScopeClosed)
        } else {
            admitted.map_err(StackedSessionRestoreError::Native)
        };
        if let Err(error) = admitted {
            fence.close();
            let output = Err(error);
            drop((identity, fuel));
            fence.finish();
            return output;
        }
        let output = Ok(Self {
            core: SessionCore::from_pending_final(identity, declaration, fuel),
            limits,
        });
        fence.finish();
        output
    }

    /// Fence unmetered host work under the coordinator's original scope.
    /// Only Awaiting enters `work`; otherwise return the original input.
    /// A normal output (including Err) stays pending and is not a receipt.
    /// Escaping unwind seals HostUncertain and closes the scope before retained
    /// pending-state cleanup. Callback-local destruction remains native-owned.
    /// This does not bind input to a request, prevent repeated work, cancel
    /// external effects or roll them back. Use the same scope for every call.
    pub fn with_host_work_in_scope<Input, Output>(
        &mut self,
        input: Input,
        work: impl FnOnce(Input) -> Output,
        scope: &LocalExecutionScope,
    ) -> Result<Output, Input> {
        self.with_host_outcome_in_scope(input, |input| HostWorkOutcome::Pending(work(input)), scope)
    }

    /// Like `with_host_work_in_scope`, with explicit native outcome policy.
    /// Pending retains the session. Cancelled/Failed/Uncertain revoke before
    /// retained cleanup and preserve the corresponding terminal reason.
    /// Ok(output) means work was entered, not that its outcome was successful.
    /// Return collectors in a terminal outcome to receive them after revocation;
    /// cancelling/disposal of external work remains the coordinator's duty.
    pub fn with_host_outcome_in_scope<Input, Output>(
        &mut self,
        input: Input,
        work: impl FnOnce(Input) -> HostWorkOutcome<Output>,
        scope: &LocalExecutionScope,
    ) -> Result<Output, Input> {
        self.call_in_scope(scope, |session, scope| {
            session.core.with_host_work(input, |input| {
                // Revoke before the core's locally owned pending frames unwind.
                let fence = CallFence::new(Some(scope));
                let output = work(input);
                if !matches!(&output, HostWorkOutcome::Pending(_)) {
                    scope.close();
                }
                fence.finish();
                output
            })
        })
    }

    /// Classify host work using the original session-owned meter and scope.
    /// Only Awaiting lends Fuel; refusal returns the input without charging.
    /// Debits survive pending returns, terminal outcomes and unwind. No new
    /// meter, automatic charge/refund, reply acceptance or re-entry is supplied.
    /// Trusted native code must charge correctly and never replace/refill Fuel;
    /// exhaustion is not inferred from output. Callback-local cleanup, live
    /// dispatch authorization and external cancellation remain host duties.
    ///
    /// ```compile_fail
    /// use leselang_hir::effect_stack_session::StackedControlSession;
    /// use leselang_hir::effect_session::HostWorkOutcome;
    /// use leselang_runtime_core::LocalExecutionScope;
    /// fn leak(vm: &mut StackedControlSession<'_, '_, (), (), (), u8, (), ()>,
    ///         scope: &LocalExecutionScope) {
    ///     let _ = vm.with_metered_host_outcome_in_scope(
    ///         (), |_, fuel| HostWorkOutcome::Pending(fuel), scope);
    /// }
    /// ```
    pub fn with_metered_host_outcome_in_scope<Input, Output>(
        &mut self,
        input: Input,
        work: impl FnOnce(Input, &mut Fuel) -> HostWorkOutcome<Output>,
        scope: &LocalExecutionScope,
    ) -> Result<Output, Input> {
        self.call_in_scope(scope, |session, scope| {
            session.core.with_metered_host_work(input, |input, fuel| {
                let fence = CallFence::new(Some(scope));
                let output = work(input, fuel);
                if !matches!(&output, HostWorkOutcome::Pending(_)) {
                    scope.close();
                }
                fence.finish();
                output
            })
        })
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

    /// Accept under the coordinator's original local scope without retaining it.
    /// Closed scopes cancel before validation; ordinary reply rejection remains
    /// retryable. Validation unwind revokes before core-owned pending cleanup.
    /// Callback-local cleanup is trusted, just as for the owning local wrapper.
    /// Hosts must supply the same scope used for restoration and later polls.
    pub fn try_accept_in_scope<View: ?Sized, Authority>(
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
        self.call_in_scope(scope, |session, scope| {
            session
                .core
                .try_accept_with_scope(identity, reply, authority, Some(scope))
        })
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

    /// Restore a raw pending VM under a coordinator's existing local fence.
    ///
    /// This borrows, but does not retain or replace, the unique scope. Structural
    /// refusal, native refusal, revocation or unwind closes it before retained
    /// inputs are released. Success preserves the original meter and frames.
    /// Native admission and callback-internal cleanup remain trusted. Use the
    /// same scope with `try_accept_in_scope`, `poll_in_scope` and
    /// `cancel_in_scope`; the caller still owns dispatch checks and external
    /// cancellation. The raw session does not remember its scope.
    pub fn restore_pending_in_scope<Error>(
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
        scope: &LocalExecutionScope,
    ) -> Result<Self, StackedSessionRestoreError<Error>> {
        Self::restore_pending_with_scope(
            identity,
            declaration,
            frames,
            fuel,
            limits,
            admit,
            Some(scope),
        )
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

    /// Execute one slice under the coordinator's original local scope.
    /// A closed scope cancels before native callbacks. Completion, failure or
    /// unwind revokes the shared owner, using the same lifecycle as `LocalVm`.
    /// No scope is retained: callers must keep using this same owner for all
    /// scoped operations and check its lease at external dispatch boundaries.
    pub fn poll_in_scope<Environment>(
        &mut self,
        environment: &Environment,
        scope: &LocalExecutionScope,
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
        self.call_in_scope(scope, |session, scope| {
            session.poll_with_scope(environment, Some(scope))
        })
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
