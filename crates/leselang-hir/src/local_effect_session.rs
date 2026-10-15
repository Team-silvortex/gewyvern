//! Opt-in local scope ownership around the original stacked control session.

use std::{borrow::Borrow, fmt};

pub use crate::effect_session::HostWorkOutcome;

use leselang_runtime_core::{
    Fuel, HostResultDomain, LocalExecutionLease, LocalExecutionScope, ReplyAuthority,
};

use crate::{
    effect_reentry::RestoredEffectBindings,
    effect_session::{EffectSessionAcceptance, EffectSessionEnvironment, EffectSessionStatus},
    effect_stack::{EffectStackLimits, SavedEffectReturnFrame},
    effect_stack_session::{
        StackedControlSession, StackedEffectSessionPollResult, StackedSessionRestoreError,
    },
    ir::Computation,
    local_scope::CallFence,
};

/// One local lifetime owner and the unchanged stacked session, not a second VM.
///
/// The host creates a scope, installs its weak lease in native dispatch/authority
/// and result domains, then moves that scope here. All evaluation, acceptance,
/// status and fuel stay in the original session. Ordinary reply rejection leaves
/// a pending scope open. Completion/failure close it before returning; cancel and
/// owner Drop close before retained session cleanup. An escaping native unwind
/// closes even when the caller catches it and retains this wrapper.
/// Poll also fences retained re-entry owners, restored prefix rejection and
/// successor correlation failure; it does not revoke language-recovered faults.
///
/// This fence acts at the call boundary: callback-local or consumed-frame cleanup
/// inside the original evaluator can run before an unwind reaches it. It neither
/// preempts native code nor authenticates replies, dispatches or cancels external
/// collectors. Hosts still enforce lease/schema/grants checks before actual work,
/// bound aggregate native resources and handle already-applied effects. A second
/// panic during cleanup can abort; forgetting the owner bypasses Drop.
///
/// This opt-in wrapper is !Send/!Sync; the original session's auto-traits and
/// semantics do not change. Native values need no Clone/Debug/serde/'static bound.
/// Closed-scope construction is inert; the first poll/accept cancels the original
/// session without executing it. The core's terminal reason remains sticky.
///
/// ```compile_fail
/// use leselang_hir::local_effect_session::LocalStackedControlSession;
/// fn duplicate(session: LocalStackedControlSession<'_, '_, (), (), (), u8, (), ()>) {
///     let other = session.clone();
/// }
/// ```
#[must_use = "retain the local session until completion or deliberate cancellation"]
pub struct LocalStackedControlSession<
    'expression,
    'schema,
    Node: ?Sized,
    Value,
    Capture,
    Identity,
    Declaration: ?Sized,
    Reply,
> {
    // Drop order revokes native leases before retained session payload cleanup.
    scope: LocalExecutionScope,
    session: StackedControlSession<
        'expression,
        'schema,
        Node,
        Value,
        Capture,
        Identity,
        Declaration,
        Reply,
    >,
}

pub type LocalStackedEffectSession<
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
> = LocalStackedControlSession<
    'expression,
    'schema,
    Computation<Field, Operation, HostEffect, IrResult>,
    Value,
    Capture,
    Identity,
    Declaration,
    Reply,
>;

impl<'expression, 'schema, Node: ?Sized, Value, Capture, Identity, Declaration: ?Sized, Reply>
    LocalStackedControlSession<
        'expression,
        'schema,
        Node,
        Value,
        Capture,
        Identity,
        Declaration,
        Reply,
    >
{
    pub fn new(
        scope: LocalExecutionScope,
        expression: &'expression Node,
        bindings: RestoredEffectBindings<'expression, Value>,
        fuel: Fuel,
        limits: EffectStackLimits,
    ) -> Self {
        Self {
            scope,
            session: StackedControlSession::new(expression, bindings, fuel, limits),
        }
    }

    pub fn lease(&self) -> LocalExecutionLease {
        self.scope.lease()
    }
    pub fn status(&self) -> EffectSessionStatus {
        self.session.status()
    }
    pub const fn fuel_remaining(&self) -> u64 {
        self.session.fuel_remaining()
    }

    /// Revoke before cancelling retained state, preserving any existing terminal reason.
    pub fn cancel(&mut self) -> bool {
        self.scope.close();
        self.session.cancel()
    }

    fn call<Output>(
        &mut self,
        step: impl FnOnce(
            &mut StackedControlSession<
                'expression,
                'schema,
                Node,
                Value,
                Capture,
                Identity,
                Declaration,
                Reply,
            >,
            &LocalExecutionScope,
        ) -> Output,
    ) -> Output {
        if !self.scope.is_open() {
            self.session.cancel();
        }
        let fence = CallFence::new(Some(&self.scope));
        let output = step(&mut self.session, &self.scope);
        if matches!(self.session.status(), EffectSessionStatus::Terminal(_)) {
            self.scope.close();
        }
        fence.finish();
        output
    }

    /// Explicitly fence a host-owned operation while awaiting an external reply.
    /// A non-awaiting session returns the original input without calling `work`;
    /// inspect `status()` for the reason. A normal output (including a native Err)
    /// stays verbatim, pending and unmetered: no reply is accepted or code resumed.
    ///
    /// Once entered, escaping unwind seals the original core HostUncertain and
    /// closes this scope before retained pending-state cleanup, even when a catcher
    /// keeps this VM alive. Rejected work is never entered; its unused closure
    /// captures still have ordinary trusted Rust destruction semantics.
    /// Callback-local cleanup still precedes this fence. Hosts bound native work
    /// and check input identity/schema/grants/lease at the actual effect boundary.
    /// This does not bind an arbitrary input to a request, prevent repeated calls,
    /// cancel an external collector, or roll back already-applied effects. Keep
    /// fallible output handling/disposal inside `work` if it needs this fence too.
    pub fn with_host_work<Input, Output>(
        &mut self,
        input: Input,
        work: impl FnOnce(Input) -> Output,
    ) -> Result<Output, Input> {
        self.with_host_outcome(input, |input| HostWorkOutcome::Pending(work(input)))
    }

    /// Like `with_host_work`, but the adapter explicitly classifies a normal
    /// return. Pending retains the original session. Cancelled/Failed/Uncertain
    /// revoke before retained cleanup and seal Cancelled/Failed/HostUncertain.
    /// The original output is returned without cloning, formatting or accepting
    /// it as a reply. A cleanup unwind preserves the selected terminal reason.
    ///
    /// The outer Result only indicates whether work was entered. A terminal
    /// outcome still returns Ok(output); inspect status or your native output.
    /// Classification is trusted host policy, not proof of effect delivery or
    /// absence. It does not roll back work or cancel external collectors. The
    /// same callback, output-handoff and native cleanup boundaries apply.
    /// Return collectors/queues in Cancelled(output) to hand them back only after
    /// revocation, then explicitly cancel/dispose them. Dropping them inside the
    /// callback is still native-owned cleanup before this boundary.
    pub fn with_host_outcome<Input, Output>(
        &mut self,
        input: Input,
        work: impl FnOnce(Input) -> HostWorkOutcome<Output>,
    ) -> Result<Output, Input> {
        self.call(|session, scope| {
            session.with_host_work(input, |input| {
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

    /// Exact original acceptance: a normal rejection returns the original input
    /// and does not revoke a still-pending execution. No continuation runs here.
    /// Validation unwind revokes before core-owned pending identity/frame/reply
    /// cleanup, not just at the outer call boundary. Callback-local cleanup is
    /// still native-owned and cannot be preempted.
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
        self.call(|session, scope| session.try_accept_in_scope(identity, reply, authority, scope))
    }
}

impl<Node: ?Sized, Value, Capture, Identity, Declaration: ?Sized, Reply> fmt::Debug
    for LocalStackedControlSession<'_, '_, Node, Value, Capture, Identity, Declaration, Reply>
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LocalStackedControlSession")
            .field("open", &self.scope.is_open())
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
    LocalStackedEffectSession<
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
    /// Restore pending binding returns under this local lifetime owner.
    /// Closed scopes refuse before structure/native admission. Refusal or unwind
    /// revokes before retained frame/identity and unused-hook cleanup; callback-
    /// internal cleanup is still trusted native work. Callback revocation wins
    /// over its ordinary result. Success retains the scope and starts Awaiting.
    /// No external collector is cancelled automatically and no durable cancelled
    /// snapshot may be rearmed; the same native admission duties as
    /// `StackedControlSession::restore_pending` apply.
    pub fn restore_pending<Error>(
        scope: LocalExecutionScope,
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
        let session = StackedControlSession::restore_pending_with_scope(
            identity,
            declaration,
            frames,
            fuel,
            limits,
            admit,
            Some(&scope),
        )?;
        Ok(Self { scope, session })
    }

    /// Delegate one slice with the same meter, then mirror only terminal liveness.
    /// Terminal failures revoke before retained re-entry/prefix cleanup at shared
    /// boundaries. Values already consumed by callbacks remain native-owned.
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
        self.call(|session, scope| session.poll_with_scope(environment, Some(scope)))
    }
}
