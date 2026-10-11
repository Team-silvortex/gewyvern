//! Borrowed in-memory control execution with explicit reply-gated polling.

use std::{borrow::Borrow, fmt};

use leselang_runtime_core::{
    AcceptedReply, CalculationFailure, Fuel, HostResultDomain, PendingReply, ReplyAcceptanceError,
    ReplyAuthority, ScopeFrame,
};

use crate::effect_evaluation::{
    ControlOutcome, EffectEvaluationFailure, EffectEvaluationFault, EffectEvaluationLimits,
    evaluate_resumable_effects_in_scope, native_failure,
};
use crate::effect_reentry::{
    EffectContinuation, EffectReentryEnvironment, RestoredEffectBindings, resume_accepted_effects,
};
use crate::ir::Computation;
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
                match pending.try_accept(identity, reply, authority) {
                    Ok(accepted) => {
                        self.state = State::AcceptedBinding(accepted);
                        Ok(())
                    }
                    Err(rejected) => {
                        self.state = State::AwaitingBinding(pending);
                        Err(EffectSessionRejected {
                            reply: rejected.reply,
                            error: EffectSessionReplyError::Reply(rejected.error),
                        })
                    }
                }
            }
            State::AwaitingFinal(mut pending) => {
                match pending.try_accept(identity, reply, authority) {
                    Ok(accepted) => {
                        self.state = State::AcceptedFinal(accepted);
                        Ok(())
                    }
                    Err(rejected) => {
                        self.state = State::AwaitingFinal(pending);
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
                    mut bindings,
                } => {
                    let outcome = begin(
                        expression,
                        &mut ScopeFrame::new(&mut bindings),
                        &mut self.fuel,
                    );
                    // Native prefix drops must finish before request/value handoff.
                    drop(bindings);
                    outcome?
                }
                State::AcceptedBinding(accepted) => resume(accepted, &mut self.fuel)?,
                State::AcceptedFinal(accepted) => {
                    let (identity, (), declaration, reply) = accepted.into_parts();
                    let value =
                        environment.bind_reply(&identity, declaration, reply, &mut self.fuel);
                    drop(identity);
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
                    let correlated = environment
                        .correlate_request(request, &mut self.fuel)
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
                    let correlated = environment
                        .correlate_request(request, &mut self.fuel)
                        .map_err(native_failure)
                        .map_err(&map_failure)?;
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
