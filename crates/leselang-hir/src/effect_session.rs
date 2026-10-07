//! Borrowed in-memory control execution with explicit reply-gated polling.

use std::{borrow::Borrow, fmt};

use leselang_runtime_core::{
    AcceptedReply, CalculationFailure, Fuel, HostResultDomain, PendingReply, ReplyAcceptanceError,
    ReplyAuthority, ScopeFrame,
};

use crate::effect_evaluation::{
    EffectEvaluationFailure, EffectEvaluationFault, EffectEvaluationLimits,
    evaluate_resumable_effects_in_scope, native_failure,
};
use crate::effect_reentry::{
    EffectContinuation, EffectReentryEnvironment, RestoredEffectBindings, ResumableEffectOutcome,
    resume_accepted_effects,
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

enum State<'expression, 'schema, Node: ?Sized, Value, Capture, Identity, Declaration: ?Sized, Reply>
{
    Initial {
        expression: &'expression Node,
        bindings: RestoredEffectBindings<'expression, Value>,
    },
    AwaitingBinding(
        PendingReply<
            'schema,
            Identity,
            EffectContinuation<'expression, Node, Capture>,
            Declaration,
        >,
    ),
    AwaitingFinal(PendingReply<'schema, Identity, (), Declaration>),
    AcceptedBinding(
        AcceptedReply<
            'schema,
            Identity,
            EffectContinuation<'expression, Node, Capture>,
            Declaration,
            Reply,
        >,
    ),
    AcceptedFinal(AcceptedReply<'schema, Identity, (), Declaration, Reply>),
    Terminal(EffectSessionEnd),
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
    state: State<'expression, 'schema, Node, Value, Capture, Identity, Declaration, Reply>,
    fuel: Fuel,
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
            state: State::Initial {
                expression,
                bindings,
            },
            fuel,
            limits,
        }
    }
    pub fn status(&self) -> EffectSessionStatus {
        match &self.state {
            State::Initial { .. } | State::AcceptedBinding(_) | State::AcceptedFinal(_) => {
                EffectSessionStatus::Ready
            }
            State::AwaitingBinding(_) | State::AwaitingFinal(_) => EffectSessionStatus::Awaiting,
            State::Terminal(end) => EffectSessionStatus::Terminal(*end),
        }
    }
    pub const fn fuel_remaining(&self) -> u64 {
        self.fuel.remaining()
    }

    /// Close before pending/accepted/initial native payload cleanup. Repeated
    /// cancellation preserves existing terminal reasons, including cleanup unwind.
    pub fn cancel(&mut self) -> bool {
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
                    let outcome = evaluate_resumable_effects_in_scope(
                        expression,
                        &mut ScopeFrame::new(&mut bindings),
                        environment,
                        &mut self.fuel,
                        self.limits,
                    );
                    // Native prefix drops must finish before request/value handoff.
                    drop(bindings);
                    outcome?
                }
                State::AcceptedBinding(accepted) => {
                    resume_accepted_effects(accepted, environment, &mut self.fuel, self.limits)?
                }
                State::AcceptedFinal(accepted) => {
                    let (identity, (), declaration, reply) = accepted.into_parts();
                    let value =
                        environment.bind_reply(&identity, declaration, reply, &mut self.fuel);
                    drop(identity);
                    ResumableEffectOutcome::Value(value.map_err(native_failure)?)
                }
                _ => {
                    return Err(
                        EffectEvaluationFault::Pure(PureEvaluationFault::InvalidContract).into(),
                    );
                }
            };
            match outcome {
                ResumableEffectOutcome::Value(value) => {
                    if matches!(&value, PureValue::Scalar(value) if !value.is_bounded()) {
                        return Err(
                            EffectEvaluationFault::Pure(PureEvaluationFault::InvalidScope).into(),
                        );
                    }
                    self.state = State::Terminal(EffectSessionEnd::Completed);
                    Ok(EffectSessionPoll::Value(value))
                }
                ResumableEffectOutcome::Request(request) => {
                    let correlated = environment
                        .correlate_request(request, &mut self.fuel)
                        .map_err(native_failure)?;
                    self.state = State::AwaitingFinal(PendingReply::new(
                        correlated.identity,
                        (),
                        correlated.declaration,
                    ));
                    Ok(EffectSessionPoll::Request(correlated.dispatch))
                }
                ResumableEffectOutcome::Suspended {
                    request,
                    continuation,
                } => {
                    let correlated = environment
                        .correlate_request(request, &mut self.fuel)
                        .map_err(native_failure)?;
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
