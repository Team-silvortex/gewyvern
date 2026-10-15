//! Borrowed in-memory control execution with explicit reply-gated polling.

use std::{borrow::Borrow, fmt};

use leselang_runtime_core::{
    AcceptedReply, CalculationFailure, Fuel, HostResultDomain, LocalExecutionScope, PendingReply,
    ReplyAcceptanceError, ReplyAuthority, ReplyEnd, ScopeFrame,
};

use crate::effect_evaluation::{
    ControlOutcome, EffectEvaluationFailure, EffectEvaluationFault, EffectEvaluationLimits,
    evaluate_resumable_effects_in_scope, native_failure,
};
use crate::effect_reentry::{
    EffectContinuation, EffectReentryEnvironment, RestoredEffectBindings, resume_accepted_effects,
};
use crate::ir::Computation;
use crate::local_scope::CallFence;
use crate::pure_evaluation::{PureEvaluationFault, PureValue};

/// Exact native correlation and original result domain, not receipt authority.
/// The consumed dispatch payload need not be the evaluator's preparation type.
pub struct CorrelatedEffectRequest<'schema, Identity, Declaration: ?Sized, Dispatch> {
    pub identity: Identity,
    pub declaration: &'schema Declaration,
    pub dispatch: Dispatch,
}
pub type EffectCorrelation<'schema, Identity, Declaration, Dispatch, Native> = Result<
    CorrelatedEffectRequest<'schema, Identity, Declaration, Dispatch>,
    CalculationFailure<Native>,
>;

/// Non-dispatching correlation of a selected, already prepared native request.
/// The hook must retain its original domain, encode exact owner/generation/site
/// identity, recheck live policy and bound/charge metadata work. No authentication,
/// global identity uniqueness or external delivery is supplied by this core.
/// Restoration/projection also own native view compatibility and live policy.
pub trait EffectSessionEnvironment<'expression, 'schema, Field, Operation, HostEffect, IrResult>:
    EffectReentryEnvironment<
        'expression,
        Field,
        Operation,
        HostEffect,
        IrResult,
        Self::Identity,
        Self::Declaration,
        Self::Reply,
    >
{
    type Identity;
    type Declaration: ?Sized + 'schema;
    type Reply;
    type Dispatch;

    fn correlate_request(
        &self,
        request: Self::Request,
        fuel: &mut Fuel,
    ) -> EffectCorrelation<'schema, Self::Identity, Self::Declaration, Self::Dispatch, Self::Error>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EffectSessionEnd {
    Completed,
    Cancelled,
    Failed,
    HostUncertain,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EffectSessionStatus {
    Ready,
    Awaiting,
    Terminal(EffectSessionEnd),
}

/// Explicit native policy, not inference from an error code or a Rust Result.
/// All variants hand the original output back; none accepts a reply or completes
/// the language computation. Failure/cancellation does not prove that no effect
/// occurred or that external work has stopped.
#[must_use = "return the outcome through the local host-work boundary"]
pub enum HostWorkOutcome<Output> {
    /// Preserve the pending session, including for recoverable native refusals.
    Pending(Output),
    /// End this execution at the host's request, without claiming external rollback.
    Cancelled(Output),
    /// End this execution with a known, non-recoverable native failure.
    Failed(Output),
    /// End this execution because native publication/completion is uncertain.
    Uncertain(Output),
}
impl<Output> fmt::Debug for HostWorkOutcome<Output> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Pending(_) => "Pending",
            Self::Cancelled(_) => "Cancelled",
            Self::Failed(_) => "Failed",
            Self::Uncertain(_) => "Uncertain",
        })
    }
}

#[must_use = "consume the request/value or observe waiting/terminal state"]
pub enum EffectSessionPoll<Value, Dispatch> {
    Request(Dispatch),
    Value(PureValue<Value>),
    Awaiting,
    Terminal(EffectSessionEnd),
}
impl<Value, Dispatch> fmt::Debug for EffectSessionPoll<Value, Dispatch> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Request(_) => "Request",
            Self::Value(_) => "Value",
            Self::Awaiting => "Awaiting",
            Self::Terminal(_) => "Terminal",
        })
    }
}
pub type EffectSessionPollResult<Value, Dispatch, Native> =
    Result<EffectSessionPoll<Value, Dispatch>, EffectEvaluationFailure<Native>>;

pub enum EffectSessionReplyError<AuthorityError, ResultError> {
    NotAwaiting(EffectSessionStatus),
    Reply(ReplyAcceptanceError<AuthorityError, ResultError>),
}
impl<AuthorityError, ResultError> fmt::Display
    for EffectSessionReplyError<AuthorityError, ResultError>
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotAwaiting(_) => formatter.write_str("control session is not awaiting a reply"),
            Self::Reply(error) => fmt::Display::fmt(error, formatter),
        }
    }
}
impl<AuthorityError, ResultError> fmt::Debug
    for EffectSessionReplyError<AuthorityError, ResultError>
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, formatter)
    }
}
impl<AuthorityError, ResultError> std::error::Error
    for EffectSessionReplyError<AuthorityError, ResultError>
{
}
#[must_use = "handle the unchanged rejected input without implicit retries"]
pub struct EffectSessionRejected<Reply, AuthorityError, ResultError> {
    pub reply: Reply,
    pub error: EffectSessionReplyError<AuthorityError, ResultError>,
}
impl<Reply, AuthorityError, ResultError> fmt::Debug
    for EffectSessionRejected<Reply, AuthorityError, ResultError>
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.error, formatter)
    }
}
pub type EffectSessionAcceptance<Reply, AuthorityError, ResultError> =
    Result<(), EffectSessionRejected<Reply, AuthorityError, ResultError>>;

enum State<
    'expression,
    'schema,
    Node: ?Sized,
    Value,
    Continuation,
    Identity,
    Declaration: ?Sized,
    Reply,
> {
    Initial {
        expression: &'expression Node,
        bindings: RestoredEffectBindings<'expression, Value>,
    },
    AwaitingBinding(PendingReply<'schema, Identity, Continuation, Declaration>),
    AwaitingFinal(PendingReply<'schema, Identity, (), Declaration>),
    AcceptedBinding(AcceptedReply<'schema, Identity, Continuation, Declaration, Reply>),
    AcceptedFinal(AcceptedReply<'schema, Identity, (), Declaration, Reply>),
    Terminal(EffectSessionEnd),
}

/// Shared reply-gated lifecycle; execution profiles supply only begin/re-entry.
pub(crate) struct SessionCore<
    'expression,
    'schema,
    Node: ?Sized,
    Value,
    Continuation,
    Identity,
    Declaration: ?Sized,
    Reply,
> {
    state: State<'expression, 'schema, Node, Value, Continuation, Identity, Declaration, Reply>,
    fuel: Fuel,
}

/// One borrowed control execution with owned fuel and reply/capture state.
/// No native Clone/Debug/serde/Send/Sync bound is imposed; result views retain the
/// pure evaluator's explicit Clone contract only when execution uses local reads.
/// Poll while awaiting/terminal never invokes hooks, charges or returns data twice.
/// Acceptance only marks Ready: restoration, successor preparation and final reply
/// projection require a later explicit poll and can be cancelled before it.
/// Cancellation closes before native cleanup; every execution/acceptance unwind
/// seals HostUncertain before callbacks or owned-prefix destructors can replay.
/// Normal execution errors close Failed, with no retry, refund or fuel refill.
///
/// This is neither a durable VM nor a dispatcher, scheduler, group aggregator,
/// receipt authenticator or global/external exactly-once fence. The host bounds
/// ingress/aggregate sessions/native work and owns live policy, mutable views,
/// correlation uniqueness and cancellation delivery. Hooks may mutate or unwind;
/// native work cannot be preempted/rolled back and a second cleanup panic aborts.
/// Recreating an execution is not deduplicated. Completed means one value was
/// handed off, not a durable commit or evidence the caller consumed its output.
///
/// ```compile_fail
/// use leselang_hir::effect_session::ControlSession;
/// use leselang_runtime_core::ScalarTypeSet;
/// fn duplicate(session: ControlSession<'_, '_, (), (), (), u64, ScalarTypeSet, u8>) {
///     let copy = session.clone();
/// }
/// ```
///
/// ```compile_fail
/// use leselang_hir::effect_session::ControlSession;
/// use leselang_runtime_core::ScalarTypeSet;
/// fn wire(session: ControlSession<'_, '_, (), (), (), u64, ScalarTypeSet, u8>) {
///     let wire = serde_json::to_string(&session).unwrap();
/// }
/// ```
#[must_use = "retain the control session until completed, cancelled or deliberately dropped"]
pub struct ControlSession<
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
        EffectContinuation<'expression, Node, Capture>,
        Identity,
        Declaration,
        Reply,
    >,
    limits: EffectEvaluationLimits,
}
pub type EffectSession<
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
> = ControlSession<
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
    ControlSession<'expression, 'schema, Node, Value, Capture, Identity, Declaration, Reply>
{
    /// Store immutable IR and move initial bindings/fuel, without native work.
    /// First poll performs the shared whole physical/prefix/cold schema preflight
    /// before fuel/execution. Construction is not admission or an ingress bound.
    pub fn new(
        expression: &'expression Node,
        bindings: RestoredEffectBindings<'expression, Value>,
        fuel: Fuel,
        limits: EffectEvaluationLimits,
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

    /// Close before native cleanup; preserve every existing terminal reason.
    pub fn cancel(&mut self) -> bool {
        self.core.cancel()
    }

    /// Acceptance only marks Ready, without restoration, projection or execution.
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
}

impl<'expression, 'schema, Node: ?Sized, Value, Continuation, Identity, Declaration: ?Sized, Reply>
    SessionCore<'expression, 'schema, Node, Value, Continuation, Identity, Declaration, Reply>
{
    pub(crate) fn new(
        expression: &'expression Node,
        bindings: RestoredEffectBindings<'expression, Value>,
        fuel: Fuel,
    ) -> Self {
        Self {
            state: State::Initial {
                expression,
                bindings,
            },
            fuel,
        }
    }
    pub(crate) fn status(&self) -> EffectSessionStatus {
        match &self.state {
            State::Initial { .. } | State::AcceptedBinding(_) | State::AcceptedFinal(_) => {
                EffectSessionStatus::Ready
            }
            State::AwaitingBinding(_) | State::AwaitingFinal(_) => EffectSessionStatus::Awaiting,
            State::Terminal(end) => EffectSessionStatus::Terminal(*end),
        }
    }

    pub(crate) fn from_pending(
        identity: Identity,
        continuation: Continuation,
        declaration: &'schema Declaration,
        fuel: Fuel,
    ) -> Self {
        Self {
            state: State::AwaitingBinding(PendingReply::new(identity, continuation, declaration)),
            fuel,
        }
    }
    pub(crate) const fn fuel_remaining(&self) -> u64 {
        self.fuel.remaining()
    }

    /// Close before pending/accepted/initial native payload cleanup. Repeated
    /// cancellation preserves existing terminal reasons, including cleanup unwind.
    pub(crate) fn cancel(&mut self) -> bool {
        match std::mem::replace(
            &mut self.state,
            State::Terminal(EffectSessionEnd::Cancelled),
        ) {
            State::Terminal(end) => {
                self.state = State::Terminal(end);
                false
            }
            old => {
                drop(old);
                true
            }
        }
    }

    /// Temporarily own the original pending state while the adapter does native
    /// work. Only an explicit pending outcome restores it; unwind or a terminal
    /// native outcome cannot make it replayable.
    pub(crate) fn with_host_work<Input, Output>(
        &mut self,
        input: Input,
        work: impl FnOnce(Input) -> HostWorkOutcome<Output>,
    ) -> Result<Output, Input> {
        self.with_metered_host_work(input, |input, _| work(input))
    }

    pub(crate) fn with_metered_host_work<Input, Output>(
        &mut self,
        input: Input,
        work: impl FnOnce(Input, &mut Fuel) -> HostWorkOutcome<Output>,
    ) -> Result<Output, Input> {
        if self.status() != EffectSessionStatus::Awaiting {
            return Err(input);
        }
        let pending = std::mem::replace(
            &mut self.state,
            State::Terminal(EffectSessionEnd::HostUncertain),
        );
        let output = work(input, &mut self.fuel);
        let output = match output {
            HostWorkOutcome::Pending(output) => {
                self.state = pending;
                output
            }
            HostWorkOutcome::Cancelled(output) => {
                self.state = State::Terminal(EffectSessionEnd::Cancelled);
                drop(pending);
                output
            }
            HostWorkOutcome::Failed(output) => {
                self.state = State::Terminal(EffectSessionEnd::Failed);
                drop(pending);
                output
            }
            HostWorkOutcome::Uncertain(output) => {
                drop(pending);
                output
            }
        };
        Ok(output)
    }

    /// Shared identity -> live authority -> borrowed type/value checks only.
    /// Success stores the moved accepted frame/input without restoring or running.
    /// Ordinary rejection returns the exact input and retains the pending state.
    pub(crate) fn try_accept<View: ?Sized, Authority>(
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
        self.try_accept_with_scope(identity, reply, authority, None)
    }

    pub(crate) fn try_accept_with_scope<View: ?Sized, Authority>(
        &mut self,
        identity: &Identity,
        reply: Reply,
        authority: &Authority,
        scope: Option<&LocalExecutionScope>,
    ) -> EffectSessionAcceptance<Reply, Authority::Error, Declaration::Error>
    where
        Identity: PartialEq,
        Reply: Borrow<View>,
        Declaration: HostResultDomain<View>,
        Authority: ReplyAuthority<Identity>,
    {
        if self.status() != EffectSessionStatus::Awaiting {
            return Err(EffectSessionRejected {
                reply,
                error: EffectSessionReplyError::NotAwaiting(self.status()),
            });
        }
        let old = std::mem::replace(
            &mut self.state,
            State::Terminal(EffectSessionEnd::HostUncertain),
        );
        match old {
            State::AwaitingBinding(mut pending) => {
                let accepted = match scope {
                    Some(scope) => pending.try_accept_in_scope(identity, reply, authority, scope),
                    None => pending.try_accept(identity, reply, authority),
                };
                match accepted {
                    Ok(accepted) => {
                        self.state = State::AcceptedBinding(accepted);
                        Ok(())
                    }
                    Err(rejected) => {
                        self.state = if matches!(
                            &rejected.error,
                            ReplyAcceptanceError::Closed(ReplyEnd::Cancelled)
                        ) {
                            State::Terminal(EffectSessionEnd::Cancelled)
                        } else {
                            State::AwaitingBinding(pending)
                        };
                        Err(EffectSessionRejected {
                            reply: rejected.reply,
                            error: EffectSessionReplyError::Reply(rejected.error),
                        })
                    }
                }
            }
            State::AwaitingFinal(mut pending) => {
                let accepted = match scope {
                    Some(scope) => pending.try_accept_in_scope(identity, reply, authority, scope),
                    None => pending.try_accept(identity, reply, authority),
                };
                match accepted {
                    Ok(accepted) => {
                        self.state = State::AcceptedFinal(accepted);
                        Ok(())
                    }
                    Err(rejected) => {
                        self.state = if matches!(
                            &rejected.error,
                            ReplyAcceptanceError::Closed(ReplyEnd::Cancelled)
                        ) {
                            State::Terminal(EffectSessionEnd::Cancelled)
                        } else {
                            State::AwaitingFinal(pending)
                        };
                        Err(EffectSessionRejected {
                            reply: rejected.reply,
                            error: EffectSessionReplyError::Reply(rejected.error),
                        })
                    }
                }
            }
            other => {
                self.state = other;
                Err(EffectSessionRejected {
                    reply,
                    error: EffectSessionReplyError::NotAwaiting(self.status()),
                })
            }
        }
    }
}

impl<Node: ?Sized, Value, Capture, Identity, Declaration: ?Sized, Reply> fmt::Debug
    for ControlSession<'_, '_, Node, Value, Capture, Identity, Declaration, Reply>
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ControlSession")
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
    EffectSession<
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
    /// Run one ready slice and return a request or final value at most once.
    /// Awaiting/terminal polls are inert. Native prefix/identity cleanup finishes
    /// while sealed HostUncertain, before installing pending state or completion.
    pub fn poll<Environment>(
        &mut self,
        environment: &Environment,
    ) -> EffectSessionPollResult<Value, Environment::Dispatch, Environment::Error>
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
                evaluate_resumable_effects_in_scope(expression, scope, environment, fuel, limits)
                    .map(ControlOutcome::from)
            },
            |accepted, fuel| {
                resume_accepted_effects(accepted, environment, fuel, limits)
                    .map(ControlOutcome::from)
            },
            |error| error,
            None,
        )
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
    Continuation,
    Identity,
    Declaration: ?Sized,
    Reply,
>
    SessionCore<
        'expression,
        'schema,
        Computation<Field, Operation, HostEffect, IrResult>,
        Value,
        Continuation,
        Identity,
        Declaration,
        Reply,
    >
{
    pub(crate) fn poll_with<Environment, Failure, Begin, Resume, MapFailure>(
        &mut self,
        environment: &Environment,
        begin: Begin,
        resume: Resume,
        map_failure: MapFailure,
        execution_scope: Option<&LocalExecutionScope>,
    ) -> Result<EffectSessionPoll<Value, Environment::Dispatch>, Failure>
    where
        Environment: EffectSessionEnvironment<
                'expression,
                'schema,
                Field,
                Operation,
                HostEffect,
                IrResult,
                Result = Value,
                Identity = Identity,
                Declaration = Declaration,
                Reply = Reply,
            >,
        Begin: FnOnce(
            &'expression Computation<Field, Operation, HostEffect, IrResult>,
            &mut ScopeFrame<'_, 'expression, PureValue<Value>>,
            &mut Fuel,
        )
            -> Result<ControlOutcome<Value, Environment::Request, Continuation>, Failure>,
        Resume:
            FnOnce(
                AcceptedReply<'schema, Identity, Continuation, Declaration, Reply>,
                &mut Fuel,
            )
                -> Result<ControlOutcome<Value, Environment::Request, Continuation>, Failure>,
        MapFailure: Fn(EffectEvaluationFailure<Environment::Error>) -> Failure,
    {
        match self.status() {
            EffectSessionStatus::Awaiting => return Ok(EffectSessionPoll::Awaiting),
            EffectSessionStatus::Terminal(end) => return Ok(EffectSessionPoll::Terminal(end)),
            EffectSessionStatus::Ready => {}
        }
        let old = std::mem::replace(
            &mut self.state,
            State::Terminal(EffectSessionEnd::HostUncertain),
        );
        let result = (|| {
            let outcome = match old {
                State::Initial {
                    expression,
                    bindings,
                } => leselang_runtime_core::with_owned_scope_state(
                    bindings,
                    &mut self.fuel,
                    |scope, fuel| {
                        CallFence::run(execution_scope, || begin(expression, scope, fuel))
                    },
                )?,
                State::AcceptedBinding(accepted) => resume(accepted, &mut self.fuel)?,
                State::AcceptedFinal(accepted) => {
                    let (identity, (), declaration, reply) = accepted.into_parts();
                    let value = CallFence::run(execution_scope, || {
                        environment.bind_reply(&identity, declaration, reply, &mut self.fuel)
                    });
                    let fence = CallFence::new(execution_scope);
                    if matches!(&value, Ok(PureValue::Scalar(value)) if !value.is_bounded()) {
                        fence.close();
                    }
                    drop(identity);
                    fence.finish();
                    ControlOutcome::Value(value.map_err(native_failure).map_err(&map_failure)?)
                }
                _ => {
                    return Err(map_failure(
                        EffectEvaluationFault::Pure(PureEvaluationFault::InvalidContract).into(),
                    ));
                }
            };
            match outcome {
                ControlOutcome::Value(value) => {
                    if matches!(&value, PureValue::Scalar(value) if !value.is_bounded()) {
                        return Err(map_failure(
                            EffectEvaluationFault::Pure(PureEvaluationFault::InvalidScope).into(),
                        ));
                    }
                    self.state = State::Terminal(EffectSessionEnd::Completed);
                    Ok(EffectSessionPoll::Value(value))
                }
                ControlOutcome::Request(request) => {
                    let correlated = CallFence::run(execution_scope, || {
                        environment.correlate_request(request, &mut self.fuel)
                    })
                    .map_err(native_failure)
                    .map_err(&map_failure)?;
                    self.state = State::AwaitingFinal(PendingReply::new(
                        correlated.identity,
                        (),
                        correlated.declaration,
                    ));
                    Ok(EffectSessionPoll::Request(correlated.dispatch))
                }
                ControlOutcome::Suspended {
                    request,
                    continuation,
                } => {
                    let correlated = CallFence::run(execution_scope, || {
                        environment.correlate_request(request, &mut self.fuel)
                    })
                    .map_err(native_failure)
                    .map_err(&map_failure);
                    let correlated = match correlated {
                        Ok(correlated) => correlated,
                        Err(error) => {
                            let output = Err(error);
                            drop(continuation);
                            return output;
                        }
                    };
                    self.state = State::AwaitingBinding(PendingReply::new(
                        correlated.identity,
                        continuation,
                        correlated.declaration,
                    ));
                    Ok(EffectSessionPoll::Request(correlated.dispatch))
                }
            }
        })();
        if result.is_err() {
            self.state = State::Terminal(EffectSessionEnd::Failed);
        }
        result
    }
}

#[cfg(test)]
mod scoped_reply_cancellation {
    use super::*;
    use leselang_runtime_core::{ScalarType, ScalarTypeSet, ScalarValue};
    use std::cell::Cell;

    struct Authority<'a> {
        scope: &'a LocalExecutionScope,
        calls: Cell<usize>,
        refuse: bool,
    }
    impl ReplyAuthority<u8> for Authority<'_> {
        type Error = &'static str;
        fn authorize(&self, _: &u8) -> Result<(), Self::Error> {
            self.calls.set(self.calls.get() + 1);
            self.scope.close();
            if self.refuse {
                Err("native refusal")
            } else {
                Ok(())
            }
        }
    }
    struct Reply {
        value: ScalarValue,
        buffer: Box<[u8]>,
    }
    impl Borrow<ScalarValue> for Reply {
        fn borrow(&self) -> &ScalarValue {
            &self.value
        }
    }

    struct FragileIdentity<'a> {
        scope: &'a LocalExecutionScope,
        cleanup_closed: &'a Cell<bool>,
        unwind: bool,
    }
    impl PartialEq for FragileIdentity<'_> {
        fn eq(&self, other: &Self) -> bool {
            std::ptr::eq(self.scope, other.scope)
        }
    }
    impl Drop for FragileIdentity<'_> {
        fn drop(&mut self) {
            if self.unwind {
                self.cleanup_closed.set(!self.scope.is_open());
                panic!("pending native identity cleanup unwind");
            }
        }
    }
    impl ReplyAuthority<FragileIdentity<'_>> for Authority<'_> {
        type Error = &'static str;
        fn authorize(&self, _: &FragileIdentity<'_>) -> Result<(), Self::Error> {
            <Self as ReplyAuthority<u8>>::authorize(self, &7)
        }
    }

    #[test]
    fn scoped_cancellation_cleanup_unwind_keeps_enclosing_session_uncertain_and_closed() {
        for binding in [false, true] {
            for refuse in [false, true] {
                let scope = LocalExecutionScope::new();
                let cleanup_closed = Cell::new(false);
                let authority = Authority {
                    scope: &scope,
                    calls: Cell::new(0),
                    refuse,
                };
                let domain = ScalarTypeSet::only(ScalarType::Integer);
                let actual = FragileIdentity {
                    scope: &scope,
                    cleanup_closed: &cleanup_closed,
                    unwind: false,
                };
                let pending = PendingReply::new(
                    FragileIdentity {
                        unwind: true,
                        ..actual
                    },
                    (),
                    &domain,
                );
                let mut core: SessionCore<
                    '_,
                    '_,
                    (),
                    (),
                    (),
                    FragileIdentity<'_>,
                    ScalarTypeSet,
                    Reply,
                > = SessionCore {
                    state: if binding {
                        State::AwaitingBinding(pending)
                    } else {
                        State::AwaitingFinal(pending)
                    },
                    fuel: Fuel::new(97),
                };
                let reply = Reply {
                    value: ScalarValue::Integer(3),
                    buffer: vec![11; 17].into_boxed_slice(),
                };
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    core.try_accept_with_scope(&actual, reply, &authority, Some(&scope))
                }));
                let unwound = result.is_err();
                let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| drop(result)));
                assert!(unwound);
                assert!(cleanup_closed.get());
                assert!(!scope.is_open());
                assert_eq!(
                    core.status(),
                    EffectSessionStatus::Terminal(EffectSessionEnd::HostUncertain)
                );
                assert!(!core.cancel());
                assert_eq!(authority.calls.get(), 1);
                assert_eq!(core.fuel.remaining(), 97);
            }
        }
    }

    #[test]
    fn scoped_cancelled_rejection_never_rearms_binding_or_final_pending_state() {
        for binding in [false, true] {
            for preclosed in [false, true] {
                for refuse in [false, true] {
                    let scope = LocalExecutionScope::new();
                    let domain = ScalarTypeSet::only(ScalarType::Integer);
                    let authority = Authority {
                        scope: &scope,
                        calls: Cell::new(0),
                        refuse,
                    };
                    let pending = PendingReply::new(7, (), &domain);
                    let mut core: SessionCore<'_, '_, (), (), (), u8, ScalarTypeSet, Reply> =
                        SessionCore {
                            state: if binding {
                                State::AwaitingBinding(pending)
                            } else {
                                State::AwaitingFinal(pending)
                            },
                            fuel: Fuel::new(97),
                        };
                    if preclosed {
                        scope.close();
                    }
                    let reply = Reply {
                        value: ScalarValue::Integer(3),
                        buffer: vec![11; 17].into_boxed_slice(),
                    };
                    let pointer = reply.buffer.as_ptr();
                    let rejected = core
                        .try_accept_with_scope(&7, reply, &authority, Some(&scope))
                        .unwrap_err();
                    assert_eq!(rejected.reply.buffer.as_ptr(), pointer);
                    assert!(matches!(
                        rejected.error,
                        EffectSessionReplyError::Reply(ReplyAcceptanceError::Closed(
                            ReplyEnd::Cancelled
                        ))
                    ));
                    assert_eq!(
                        core.status(),
                        EffectSessionStatus::Terminal(EffectSessionEnd::Cancelled)
                    );
                    assert_eq!(core.fuel.remaining(), 97);
                    assert_eq!(authority.calls.get(), usize::from(!preclosed));
                    assert!(!core.cancel());
                    let late = core
                        .try_accept_with_scope(&7, rejected.reply, &authority, Some(&scope))
                        .unwrap_err();
                    assert_eq!(late.reply.buffer.as_ptr(), pointer);
                    assert!(matches!(
                        late.error,
                        EffectSessionReplyError::NotAwaiting(EffectSessionStatus::Terminal(
                            EffectSessionEnd::Cancelled
                        ))
                    ));
                    assert_eq!(authority.calls.get(), usize::from(!preclosed));
                    assert_eq!(core.fuel.remaining(), 97);
                }
            }
        }
    }
}
