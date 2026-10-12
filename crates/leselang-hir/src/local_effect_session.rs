//! Opt-in local scope ownership around the original stacked control session.

use std::{borrow::Borrow, fmt};

use leselang_runtime_core::{
    Fuel, HostResultDomain, LocalExecutionLease, LocalExecutionScope, ReplyAuthority,
};

use crate::{
    effect_reentry::RestoredEffectBindings,
    effect_session::{EffectSessionAcceptance, EffectSessionEnvironment, EffectSessionStatus},
    effect_stack::EffectStackLimits,
    effect_stack_session::{StackedControlSession, StackedEffectSessionPollResult},
    ir::Computation,
};

struct CallFence<'scope> {
    scope: &'scope LocalExecutionScope,
    returned: bool,
}
impl CallFence<'_> {
    fn finish(mut self) {
        self.returned = true;
    }
}
impl Drop for CallFence<'_> {
    fn drop(&mut self) {
        if !self.returned {
            self.scope.close();
        }
    }
}

/// One local lifetime owner and the unchanged stacked session, not a second VM.
///
/// The host creates a scope, installs its weak lease in native dispatch/authority
/// and result domains, then moves that scope here. All evaluation, acceptance,
/// status and fuel stay in the original session. Ordinary reply rejection leaves
/// a pending scope open. Completion/failure close it before returning; cancel and
/// owner Drop close before retained session cleanup. An escaping native unwind
/// closes even when the caller catches it and retains this wrapper.
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
        ) -> Output,
    ) -> Output {
        if !self.scope.is_open() {
            self.session.cancel();
        }
        let fence = CallFence {
            scope: &self.scope,
            returned: false,
        };
        let output = step(&mut self.session);
        if matches!(self.session.status(), EffectSessionStatus::Terminal(_)) {
            self.scope.close();
        }
        fence.finish();
        output
    }

    /// Exact original acceptance: a normal rejection returns the original input
    /// and does not revoke a still-pending execution. No continuation runs here.
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
        self.call(|session| session.try_accept(identity, reply, authority))
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
    /// Delegate one slice with the same meter, then mirror only terminal liveness.
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
        self.call(|session| session.poll(environment))
    }
}
