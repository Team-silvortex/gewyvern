//! Bounded, borrowed parallel batch ownership with an all-success reply barrier.

use std::{borrow::Borrow, convert::Infallible, fmt};

use leselang_runtime_core::{
    AcceptedReply, Fuel, HostResultDomain, LocalExecutionScope, PendingReply, ReplyAcceptanceError,
    ReplyAuthority, ReplyEnd,
};

use crate::group_exports::{
    GroupExportError, GroupExportLimits, MAX_GROUP_EXPORT_MEMBERS, atomic_candidate, physical,
};
use crate::ir::{Computation, ComputedBranch, GroupKind};
use crate::local_scope::CallFence;
use crate::pure_typing::{MAX_TYPE_INFERENCE_DEPTH, MAX_TYPE_INFERENCE_NODES};

type Node<Field, Operation, HostEffect, IrResult> =
    Computation<Field, Operation, HostEffect, IrResult>;

// Both modes adapt the same atomic member and exact reply-domain protocol.
pub use crate::sequence_evaluation::{
    PreparedSequenceMember as PreparedParallelMember, SequenceAccepted as ParallelAccepted,
    SequenceEvaluationEnvironment as ParallelEvaluationEnvironment,
    SequenceEvaluationLimits as ParallelEvaluationLimits, SequencePhase as ParallelPhase,
    SequencePreparation as ParallelPreparation,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ParallelEnd {
    Completed,
    Cancelled,
    Failed,
    HostUncertain,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ParallelStatus {
    Ready,
    Awaiting { remaining: usize },
    ReadyResults,
    Terminal(ParallelEnd),
}

pub enum ParallelEvaluationError<Native> {
    InvalidLimits,
    NotParallel,
    Shape(GroupExportError<Infallible>),
    Candidate {
        index: usize,
    },
    Fuel,
    DuplicateIdentity {
        index: usize,
        previous: usize,
    },
    Native {
        index: usize,
        phase: ParallelPhase,
        error: Native,
    },
    InternalState,
}
impl<Native> fmt::Display for ParallelEvaluationError<Native> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidLimits => "parallel session limits exceed safety ceilings",
            Self::NotParallel => "parallel session requires a flat parallel group",
            Self::Shape(_) => "parallel session IR has invalid physical or language shape",
            Self::Candidate { .. } => "parallel session member is not an atomic candidate",
            Self::Fuel => "parallel session fuel exhausted",
            Self::DuplicateIdentity { .. } => "parallel session member identities must be distinct",
            Self::Native { .. } => "parallel session native preparation failed",
            Self::InternalState => "parallel session result barrier is inconsistent",
        })
    }
}
impl<Native> fmt::Debug for ParallelEvaluationError<Native> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}
impl<Native> std::error::Error for ParallelEvaluationError<Native> {}

#[must_use = "handle or deliberately drop this once-published native request"]
pub struct ParallelRequest<Request> {
    pub index: usize,
    pub request: Request,
}
impl<Request> fmt::Debug for ParallelRequest<Request> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ParallelRequest")
            .field("index", &self.index)
            .finish()
    }
}

#[must_use = "handle the complete batch or observe the waiting/terminal state"]
pub enum ParallelPoll<Request, Accepted> {
    Requests(Vec<ParallelRequest<Request>>),
    Awaiting { remaining: usize },
    Results(Vec<Accepted>),
    Terminal(ParallelEnd),
}
impl<Request, Accepted> fmt::Debug for ParallelPoll<Request, Accepted> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Requests(_) => "Requests",
            Self::Awaiting { .. } => "Awaiting",
            Self::Results(_) => "Results",
            Self::Terminal(_) => "Terminal",
        })
    }
}

pub enum ParallelReplyError<AuthorityError, ResultError> {
    NotAwaiting(ParallelStatus),
    MemberNotAwaiting { index: usize },
    Reply(ReplyAcceptanceError<AuthorityError, ResultError>),
}
impl<AuthorityError, ResultError> fmt::Display for ParallelReplyError<AuthorityError, ResultError> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotAwaiting(_) => f.write_str("parallel session is not awaiting replies"),
            Self::MemberNotAwaiting { .. } => {
                f.write_str("parallel member is not awaiting a reply")
            }
            Self::Reply(error) => fmt::Display::fmt(error, f),
        }
    }
}
impl<AuthorityError, ResultError> fmt::Debug for ParallelReplyError<AuthorityError, ResultError> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}
impl<AuthorityError, ResultError> std::error::Error
    for ParallelReplyError<AuthorityError, ResultError>
{
}

#[must_use = "handle the unchanged rejected reply without implicit retries"]
pub struct ParallelRejected<Reply, AuthorityError, ResultError> {
    pub reply: Reply,
    pub error: ParallelReplyError<AuthorityError, ResultError>,
}
impl<Reply, AuthorityError, ResultError> fmt::Debug
    for ParallelRejected<Reply, AuthorityError, ResultError>
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.error, f)
    }
}
pub type ParallelAcceptance<Reply, AuthorityError, ResultError> =
    Result<(), ParallelRejected<Reply, AuthorityError, ResultError>>;

enum Member<'schema, Identity, Declaration: ?Sized, Reply> {
    Pending(PendingReply<'schema, Identity, (), Declaration>),
    Accepted(AcceptedReply<'schema, Identity, (), Declaration, Reply>),
    Moving,
}
enum State<'schema, Identity, Declaration: ?Sized, Reply> {
    Ready,
    Awaiting {
        members: Vec<Member<'schema, Identity, Declaration, Reply>>,
        remaining: usize,
    },
    ReadyResults(Vec<Member<'schema, Identity, Declaration, Reply>>),
    Terminal(ParallelEnd),
}

/// One in-memory flat-All execution, using the same atomic adapter as Seq.
/// Whole cold shape/candidate/policy admission precedes the group-root fuel debit.
/// First poll prepares every member once in declaration order, charging one root
/// unit per member plus adapter work; exact identities must be pairwise distinct.
/// No request is handed off until all preparation and identity checks succeed.
/// This is complete local batch publication, NOT atomic external dispatch.
///
/// Replies may arrive in any order, but must match both member index and identity,
/// current authority and the original borrowed result domain. Rejection preserves
/// the input and waiting state. Accepted replies are retained, never exposed as a
/// partial result. Only an explicit poll after the all-success barrier moves the
/// ordered original rows/declarations/replies to the caller, exactly once.
///
/// Cancellation closes before cleanup, including accepted-but-unpublished results.
/// Preparation, identity comparison or acceptance unwind seals HostUncertain;
/// ordinary preparation failure closes Failed after prefix cleanup. No retries,
/// fuel refunds, thread creation, native Clone/Debug/serde/Send/Sync requirements,
/// group aggregation, continuation restoration, deadline clock or persistence.
/// Host adapters still own cold typing, live policy, correlation/generation scope,
/// bounded native work, provenance, actual dispatch and cancellation delivery.
/// Recreating this session is not deduplicated; native work is not preemptible or
/// rolled back. Two cleanup panics can abort. Completed is a handoff, not a commit.
///
/// ```compile_fail
/// use leselang_hir::parallel_evaluation::ParallelSession;
/// fn duplicate(session: ParallelSession<'_, (), (), u64, (), ()>) {
///     let duplicate = session.clone();
/// }
/// ```
///
/// ```compile_fail
/// use leselang_hir::parallel_evaluation::ParallelSession;
/// fn persist(session: ParallelSession<'_, (), (), u64, (), ()>) {
///     let encoded = serde_json::to_string(&session);
/// }
/// ```
#[must_use = "retain the batch until completed, cancelled or deliberately dropped"]
pub struct ParallelSession<'expression, Node, IrResult, Identity, Declaration: ?Sized, Reply> {
    core: ParallelReplyCore<'expression, Node, IrResult, Identity, Declaration, Reply>,
    fuel: Fuel,
}
pub type ParallelEvaluation<
    'expression,
    Field,
    Operation,
    HostEffect,
    IrResult,
    Identity,
    Declaration,
    Reply,
> = ParallelSession<
    'expression,
    Node<Field, Operation, HostEffect, IrResult>,
    IrResult,
    Identity,
    Declaration,
    Reply,
>;
pub type ParallelEvaluationPoll<
    'expression,
    Node,
    IrResult,
    Identity,
    Declaration,
    Reply,
    Request,
    Error,
> = Result<
    ParallelPoll<
        Request,
        ParallelAccepted<'expression, Node, IrResult, Identity, Declaration, Reply>,
    >,
    ParallelEvaluationError<Error>,
>;

#[expect(
    clippy::type_complexity,
    reason = "shape admission retains the original native IR slots"
)]
pub(crate) fn admit_parallel_shape<Field, Operation, HostEffect, IrResult, Error>(
    expression: &Node<Field, Operation, HostEffect, IrResult>,
    limits: ParallelEvaluationLimits,
) -> Result<
    &[ComputedBranch<Node<Field, Operation, HostEffect, IrResult>, IrResult>],
    ParallelEvaluationError<Error>,
> {
    if limits.max_nodes > MAX_TYPE_INFERENCE_NODES
        || limits.max_depth > MAX_TYPE_INFERENCE_DEPTH
        || limits.max_branches > MAX_GROUP_EXPORT_MEMBERS
    {
        return Err(ParallelEvaluationError::InvalidLimits);
    }
    let Computation::Group {
        group_kind: GroupKind::Parallel,
        branches,
    } = expression
    else {
        return Err(ParallelEvaluationError::NotParallel);
    };
    physical(
        expression,
        GroupExportLimits {
            max_nodes: limits.max_nodes,
            max_depth: limits.max_depth,
            max_groups: 1,
            max_members: limits.max_branches,
        },
    )
    .map_err(ParallelEvaluationError::Shape)?;
    for (index, branch) in branches.iter().enumerate() {
        if !atomic_candidate(&branch.value) {
            return Err(ParallelEvaluationError::Candidate { index });
        }
    }
    Ok(branches)
}

impl<'expression, Field, Operation, HostEffect, IrResult, Identity, Declaration: ?Sized, Reply>
    ParallelEvaluation<
        'expression,
        Field,
        Operation,
        HostEffect,
        IrResult,
        Identity,
        Declaration,
        Reply,
    >
{
    pub fn start<Environment>(
        expression: &'expression Node<Field, Operation, HostEffect, IrResult>,
        environment: &Environment,
        mut fuel: Fuel,
        limits: ParallelEvaluationLimits,
    ) -> Result<Self, ParallelEvaluationError<Environment::Error>>
    where
        Environment: ParallelEvaluationEnvironment<
                'expression,
                Field,
                Operation,
                HostEffect,
                IrResult,
                Identity = Identity,
                Declaration = Declaration,
            >,
    {
        let branches = admit_parallel_shape(expression, limits)?;
        for (index, branch) in branches.iter().enumerate() {
            environment
                .preflight_member(index, branch)
                .map_err(|error| ParallelEvaluationError::Native {
                    index,
                    phase: ParallelPhase::Preflight,
                    error,
                })?;
        }
        fuel.charge(1).map_err(|_| ParallelEvaluationError::Fuel)?;
        Ok(Self {
            core: ParallelReplyCore::new(branches),
            fuel,
        })
    }

    #[expect(
        clippy::type_complexity,
        reason = "the result retains original IR and unsized native domain ownership without erasing lifetimes"
    )]
    pub fn poll<Environment>(
        &mut self,
        environment: &Environment,
    ) -> ParallelEvaluationPoll<
        'expression,
        Node<Field, Operation, HostEffect, IrResult>,
        IrResult,
        Identity,
        Declaration,
        Reply,
        Environment::Request,
        Environment::Error,
    >
    where
        Identity: PartialEq,
        Environment: ParallelEvaluationEnvironment<
                'expression,
                Field,
                Operation,
                HostEffect,
                IrResult,
                Identity = Identity,
                Declaration = Declaration,
            >,
    {
        self.poll_with_scope(environment, None)
    }

    /// Borrow local liveness for both complete-batch publication and ordered
    /// result handoff. This neither stores a scope nor cancels external dispatch.
    #[expect(
        clippy::type_complexity,
        reason = "original native result and request ownership"
    )]
    pub fn poll_in_scope<Environment>(
        &mut self,
        environment: &Environment,
        scope: &LocalExecutionScope,
    ) -> ParallelEvaluationPoll<
        'expression,
        Node<Field, Operation, HostEffect, IrResult>,
        IrResult,
        Identity,
        Declaration,
        Reply,
        Environment::Request,
        Environment::Error,
    >
    where
        Identity: PartialEq,
        Environment: ParallelEvaluationEnvironment<
                'expression,
                Field,
                Operation,
                HostEffect,
                IrResult,
                Identity = Identity,
                Declaration = Declaration,
            >,
    {
        self.poll_with_scope(environment, Some(scope))
    }

    #[expect(
        clippy::type_complexity,
        reason = "original native result and request ownership"
    )]
    fn poll_with_scope<Environment>(
        &mut self,
        environment: &Environment,
        scope: Option<&LocalExecutionScope>,
    ) -> ParallelEvaluationPoll<
        'expression,
        Node<Field, Operation, HostEffect, IrResult>,
        IrResult,
        Identity,
        Declaration,
        Reply,
        Environment::Request,
        Environment::Error,
    >
    where
        Identity: PartialEq,
        Environment: ParallelEvaluationEnvironment<
                'expression,
                Field,
                Operation,
                HostEffect,
                IrResult,
                Identity = Identity,
                Declaration = Declaration,
            >,
    {
        if let Some(outcome) = self.core.poll_results(scope)? {
            return Ok(outcome);
        }
        self.core.publish_with(
            &mut self.fuel,
            |index, branch, fuel| {
                environment
                    .prepare_member(index, branch, fuel)
                    .map_err(|error| ParallelEvaluationError::Native {
                        index,
                        phase: ParallelPhase::Prepare,
                        error,
                    })
            },
            scope,
        )
    }
}

impl<'expression, Node, IrResult, Identity, Declaration: ?Sized, Reply>
    ParallelSession<'expression, Node, IrResult, Identity, Declaration, Reply>
{
    pub fn status(&self) -> ParallelStatus {
        self.core.status()
    }
    pub const fn fuel_remaining(&self) -> u64 {
        self.fuel.remaining()
    }

    /// Move the original meter only after the complete result batch was handed
    /// off. ReadyResults, waiting and failed/cancelled states retain ownership.
    /// This never polls, dispatches, refunds fuel or grants continuation authority.
    pub fn into_completed_fuel(self) -> Result<Fuel, Self> {
        if self.status() == ParallelStatus::Terminal(ParallelEnd::Completed) {
            Ok(self.fuel)
        } else {
            Err(self)
        }
    }
    pub fn try_accept<View: ?Sized, Authority>(
        &mut self,
        index: usize,
        identity: &Identity,
        reply: Reply,
        authority: &Authority,
    ) -> ParallelAcceptance<Reply, Authority::Error, Declaration::Error>
    where
        Identity: PartialEq,
        Reply: Borrow<View>,
        Declaration: HostResultDomain<View>,
        Authority: ReplyAuthority<Identity>,
    {
        self.core.try_accept(index, identity, reply, authority)
    }
    /// Explicit local liveness without storing a scope or changing unscoped use.
    /// Revocation cancels the barrier and drops accepted-but-unpublished results.
    /// External dispatch and enclosing execution cancellation remain host-owned.
    pub fn try_accept_in_scope<View: ?Sized, Authority>(
        &mut self,
        index: usize,
        identity: &Identity,
        reply: Reply,
        authority: &Authority,
        scope: &LocalExecutionScope,
    ) -> ParallelAcceptance<Reply, Authority::Error, Declaration::Error>
    where
        Identity: PartialEq,
        Reply: Borrow<View>,
        Declaration: HostResultDomain<View>,
        Authority: ReplyAuthority<Identity>,
    {
        self.core
            .try_accept_with_scope(index, identity, reply, authority, Some(scope))
    }
    pub fn cancel(&mut self) -> bool {
        self.core.cancel()
    }
}

pub(crate) struct ParallelReplyCore<
    'expression,
    Node,
    IrResult,
    Identity,
    Declaration: ?Sized,
    Reply,
> {
    branches: &'expression [ComputedBranch<Node, IrResult>],
    state: State<'expression, Identity, Declaration, Reply>,
}

impl<'expression, Node, IrResult, Identity, Declaration: ?Sized, Reply>
    ParallelReplyCore<'expression, Node, IrResult, Identity, Declaration, Reply>
{
    pub(crate) fn new(branches: &'expression [ComputedBranch<Node, IrResult>]) -> Self {
        Self {
            branches,
            state: State::Ready,
        }
    }
    pub(crate) fn member_count(&self) -> usize {
        self.branches.len()
    }

    #[expect(
        clippy::type_complexity,
        reason = "the shared barrier moves original rows and unsized native domains"
    )]
    pub(crate) fn poll_results<Request, Error>(
        &mut self,
        scope: Option<&LocalExecutionScope>,
    ) -> Result<
        Option<
            ParallelPoll<
                Request,
                ParallelAccepted<'expression, Node, IrResult, Identity, Declaration, Reply>,
            >,
        >,
        ParallelEvaluationError<Error>,
    > {
        if scope.is_some_and(|scope| !scope.is_open()) {
            self.cancel();
        }
        match self.status() {
            ParallelStatus::Awaiting { remaining } => {
                Ok(Some(ParallelPoll::Awaiting { remaining }))
            }
            ParallelStatus::Terminal(end) => Ok(Some(ParallelPoll::Terminal(end))),
            ParallelStatus::ReadyResults => {
                let State::ReadyResults(members) =
                    std::mem::replace(&mut self.state, State::Terminal(ParallelEnd::HostUncertain))
                else {
                    return Err(ParallelEvaluationError::InternalState);
                };
                let mut results = Vec::with_capacity(members.len());
                for (index, member) in members.into_iter().enumerate() {
                    let Member::Accepted(accepted) = member else {
                        return Err(ParallelEvaluationError::InternalState);
                    };
                    results.push(ParallelAccepted::new(
                        index,
                        &self.branches[index],
                        accepted,
                    ));
                }
                self.state = State::Terminal(ParallelEnd::Completed);
                Ok(Some(ParallelPoll::Results(results)))
            }
            ParallelStatus::Ready => Ok(None),
        }
    }

    pub(crate) fn publish_with<Request, Error>(
        &mut self,
        fuel: &mut Fuel,
        mut prepare: impl FnMut(
            usize,
            &'expression ComputedBranch<Node, IrResult>,
            &mut Fuel,
        ) -> Result<
            PreparedParallelMember<'expression, Identity, Declaration, Request>,
            ParallelEvaluationError<Error>,
        >,
        scope: Option<&LocalExecutionScope>,
    ) -> ParallelEvaluationPoll<
        'expression,
        Node,
        IrResult,
        Identity,
        Declaration,
        Reply,
        Request,
        Error,
    >
    where
        Identity: PartialEq,
    {
        if scope.is_some_and(|scope| !scope.is_open()) {
            self.cancel();
            return self
                .poll_results(scope)?
                .ok_or(ParallelEvaluationError::InternalState);
        }
        if !matches!(self.state, State::Ready) {
            return Err(ParallelEvaluationError::InternalState);
        }
        self.state = State::Terminal(ParallelEnd::HostUncertain);
        // Prefixes stay local until the complete batch passes, so errors/unwind
        // cannot publish a subset or leave the session ready to replay preparation.
        let batch = (|| {
            let mut prepared = Vec::with_capacity(self.branches.len());
            // Revocation must precede retained prefix cleanup on callback unwind.
            let fence = CallFence::new(scope);
            let admission = (|| {
                for (index, branch) in self.branches.iter().enumerate() {
                    fuel.charge(1).map_err(|_| ParallelEvaluationError::Fuel)?;
                    let member = prepare(index, branch, fuel);
                    if scope.is_some_and(|scope| !scope.is_open()) {
                        self.state = State::Terminal(ParallelEnd::Cancelled);
                        drop(member);
                        return Ok(false);
                    }
                    prepared.push(member?);
                }
                for (index, member) in prepared.iter().enumerate() {
                    for (previous, other) in prepared[..index].iter().enumerate() {
                        let duplicate = member.identity == other.identity;
                        if scope.is_some_and(|scope| !scope.is_open()) {
                            self.state = State::Terminal(ParallelEnd::Cancelled);
                            return Ok(false);
                        }
                        if duplicate {
                            return Err(ParallelEvaluationError::DuplicateIdentity {
                                index,
                                previous,
                            });
                        }
                    }
                }
                Ok(true)
            })();
            match admission {
                Ok(true) => {}
                Ok(false) => {
                    drop(prepared);
                    fence.finish();
                    return Ok(None);
                }
                Err(error) => {
                    fence.close();
                    fence.finish();
                    return Err(error);
                }
            }
            let mut members = Vec::with_capacity(prepared.len());
            let mut requests = Vec::with_capacity(prepared.len());
            for (index, member) in prepared.into_iter().enumerate() {
                members.push(Member::Pending(PendingReply::new(
                    member.identity,
                    (),
                    member.declaration,
                )));
                requests.push(ParallelRequest {
                    index,
                    request: member.request,
                });
            }
            fence.finish();
            Ok(Some((members, requests)))
        })();
        match batch {
            Ok(Some((members, requests))) => {
                self.state = State::Awaiting {
                    remaining: members.len(),
                    members,
                };
                Ok(ParallelPoll::Requests(requests))
            }
            Ok(None) => Ok(ParallelPoll::Terminal(ParallelEnd::Cancelled)),
            Err(error) => {
                self.state = State::Terminal(ParallelEnd::Failed);
                Err(error)
            }
        }
    }

    pub(crate) fn status(&self) -> ParallelStatus {
        match &self.state {
            State::Ready => ParallelStatus::Ready,
            State::Awaiting { remaining, .. } => ParallelStatus::Awaiting {
                remaining: *remaining,
            },
            State::ReadyResults(_) => ParallelStatus::ReadyResults,
            State::Terminal(end) => ParallelStatus::Terminal(*end),
        }
    }
    pub(crate) fn try_accept<View: ?Sized, Authority>(
        &mut self,
        index: usize,
        identity: &Identity,
        reply: Reply,
        authority: &Authority,
    ) -> ParallelAcceptance<Reply, Authority::Error, Declaration::Error>
    where
        Identity: PartialEq,
        Reply: Borrow<View>,
        Declaration: HostResultDomain<View>,
        Authority: ReplyAuthority<Identity>,
    {
        self.try_accept_with_scope(index, identity, reply, authority, None)
    }

    pub(crate) fn try_accept_with_scope<View: ?Sized, Authority>(
        &mut self,
        index: usize,
        identity: &Identity,
        reply: Reply,
        authority: &Authority,
        scope: Option<&LocalExecutionScope>,
    ) -> ParallelAcceptance<Reply, Authority::Error, Declaration::Error>
    where
        Identity: PartialEq,
        Reply: Borrow<View>,
        Declaration: HostResultDomain<View>,
        Authority: ReplyAuthority<Identity>,
    {
        if scope.is_some_and(|scope| !scope.is_open()) {
            self.cancel();
        }
        let status = self.status();
        let State::Awaiting { members, .. } = &self.state else {
            return Err(ParallelRejected {
                reply,
                error: ParallelReplyError::NotAwaiting(status),
            });
        };
        if !matches!(members.get(index), Some(Member::Pending(_))) {
            return Err(ParallelRejected {
                reply,
                error: ParallelReplyError::MemberNotAwaiting { index },
            });
        }
        let State::Awaiting {
            mut members,
            remaining,
        } = std::mem::replace(&mut self.state, State::Terminal(ParallelEnd::HostUncertain))
        else {
            return Err(ParallelRejected {
                reply,
                error: ParallelReplyError::NotAwaiting(status),
            });
        };
        let Member::Pending(mut pending) = std::mem::replace(&mut members[index], Member::Moving)
        else {
            return Err(ParallelRejected {
                reply,
                error: ParallelReplyError::MemberNotAwaiting { index },
            });
        };
        // Revoke before disposing retained siblings on native callback unwind.
        let fence = CallFence::new(scope);
        let outcome = match scope {
            Some(scope) => pending.try_accept_in_scope(identity, reply, authority, scope),
            None => pending.try_accept(identity, reply, authority),
        };
        let outcome = match outcome {
            Ok(accepted) => {
                members[index] = Member::Accepted(accepted);
                self.state = if remaining == 1 {
                    State::ReadyResults(members)
                } else {
                    State::Awaiting {
                        members,
                        remaining: remaining - 1,
                    }
                };
                Ok(())
            }
            Err(rejected) => {
                if matches!(
                    rejected.error,
                    ReplyAcceptanceError::Closed(ReplyEnd::Cancelled)
                ) {
                    self.state = State::Terminal(ParallelEnd::Cancelled);
                    drop(members);
                } else {
                    members[index] = Member::Pending(pending);
                    self.state = State::Awaiting { members, remaining };
                }
                Err(ParallelRejected {
                    reply: rejected.reply,
                    error: ParallelReplyError::Reply(rejected.error),
                })
            }
        };
        fence.finish();
        outcome
    }

    pub(crate) fn cancel(&mut self) -> bool {
        match std::mem::replace(&mut self.state, State::Terminal(ParallelEnd::Cancelled)) {
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
}
impl<Node, IrResult, Identity, Declaration: ?Sized, Reply> fmt::Debug
    for ParallelSession<'_, Node, IrResult, Identity, Declaration, Reply>
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ParallelSession")
            .field("status", &self.status())
            .field("members", &self.core.member_count())
            .field("fuel", &self.fuel_remaining())
            .finish()
    }
}
