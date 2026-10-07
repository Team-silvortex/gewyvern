//! Borrowed, in-memory sequential request/reply ownership, not durable dispatch.

use std::{borrow::Borrow, convert::Infallible, fmt};

use leselang_runtime_core::{
    AcceptedReply, Fuel, HostResultDomain, PendingReply, ReplyAcceptanceError, ReplyAuthority,
};

use crate::group_exports::{
    GroupExportError, GroupExportLimits, MAX_GROUP_EXPORT_MEMBERS, atomic_candidate, physical,
};
use crate::ir::{Computation, ComputedBranch, GroupKind};
use crate::pure_typing::{MAX_TYPE_INFERENCE_DEPTH, MAX_TYPE_INFERENCE_NODES};

type Node<Field, Operation, HostEffect, IrResult> =
    Computation<Field, Operation, HostEffect, IrResult>;
type Branch<Field, Operation, HostEffect, IrResult> =
    ComputedBranch<Node<Field, Operation, HostEffect, IrResult>, IrResult>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SequenceEvaluationLimits {
    pub max_nodes: usize,
    pub max_depth: usize,
    pub max_branches: usize,
}

/// Host-prepared data, not a dispatched operation or an authentic receipt. The
/// declaration must be the original row's exact domain, not a look-alike schema.
pub struct PreparedSequenceMember<'schema, Identity, Declaration: ?Sized, Request> {
    pub identity: Identity,
    pub declaration: &'schema Declaration,
    pub request: Request,
}

pub type SequencePreparation<'schema, Identity, Declaration, Request, Error> =
    Result<PreparedSequenceMember<'schema, Identity, Declaration, Request>, Error>;

/// Mandatory native policy and preparation; neither callback may dispatch.
/// Preflight corroborates every cold type, exact declaration, argument schema,
/// opaque graph, version and grant. Prepare revalidates live policy, evaluates
/// only this original member and bounds scope/values/work against the same fuel.
/// The session already charges one root unit per member. Native allocation,
/// interior mutation, work, Drop and unwind are trusted, not sandboxed/preempted.
pub trait SequenceEvaluationEnvironment<'expression, Field, Operation, HostEffect, IrResult> {
    type Identity;
    type Declaration: ?Sized;
    type Request;
    type Error;

    fn preflight_member(
        &self,
        index: usize,
        branch: &'expression Branch<Field, Operation, HostEffect, IrResult>,
    ) -> Result<(), Self::Error>;

    fn prepare_member(
        &self,
        index: usize,
        branch: &'expression Branch<Field, Operation, HostEffect, IrResult>,
        fuel: &mut Fuel,
    ) -> SequencePreparation<
        'expression,
        Self::Identity,
        Self::Declaration,
        Self::Request,
        Self::Error,
    >;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SequenceEnd {
    Completed,
    Cancelled,
    Failed,
    HostUncertain,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SequenceStatus {
    Ready { index: usize },
    Awaiting { index: usize },
    Terminal(SequenceEnd),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SequencePhase {
    Preflight,
    Prepare,
}

pub enum SequenceEvaluationError<Native> {
    InvalidLimits,
    NotSequence,
    Shape(GroupExportError<Infallible>),
    Candidate {
        index: usize,
    },
    Fuel,
    Native {
        index: usize,
        phase: SequencePhase,
        error: Native,
    },
}
impl<Native> fmt::Display for SequenceEvaluationError<Native> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidLimits => "sequence session limits exceed safety ceilings",
            Self::NotSequence => "sequence session requires a flat sequential group",
            Self::Shape(_) => "sequence session IR has invalid physical or language shape",
            Self::Candidate { .. } => "sequence session member is not an atomic candidate",
            Self::Fuel => "sequence session fuel exhausted",
            Self::Native { .. } => "sequence session native preparation failed",
        })
    }
}
impl<Native> fmt::Debug for SequenceEvaluationError<Native> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, formatter)
    }
}
impl<Native> std::error::Error for SequenceEvaluationError<Native> {}

#[must_use = "the host must handle the request or observe the waiting/terminal state"]
pub enum SequencePoll<Request> {
    Request { index: usize, request: Request },
    Awaiting { index: usize },
    Terminal(SequenceEnd),
}
impl<Request> fmt::Debug for SequencePoll<Request> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Request { .. } => "Request",
            Self::Awaiting { .. } => "Awaiting",
            Self::Terminal(_) => "Terminal",
        })
    }
}

pub enum SequenceReplyError<AuthorityError, ResultError> {
    NotAwaiting(SequenceStatus),
    Reply(ReplyAcceptanceError<AuthorityError, ResultError>),
}
impl<AuthorityError, ResultError> fmt::Display for SequenceReplyError<AuthorityError, ResultError> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotAwaiting(_) => formatter.write_str("sequence session is not awaiting a reply"),
            Self::Reply(error) => fmt::Display::fmt(error, formatter),
        }
    }
}
impl<AuthorityError, ResultError> fmt::Debug for SequenceReplyError<AuthorityError, ResultError> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, formatter)
    }
}
impl<AuthorityError, ResultError> std::error::Error
    for SequenceReplyError<AuthorityError, ResultError>
{
}

#[must_use = "handle the unchanged rejected input without implicit retries"]
pub struct SequenceRejected<Reply, AuthorityError, ResultError> {
    pub reply: Reply,
    pub error: SequenceReplyError<AuthorityError, ResultError>,
}
impl<Reply, AuthorityError, ResultError> fmt::Debug
    for SequenceRejected<Reply, AuthorityError, ResultError>
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.error, formatter)
    }
}

/// One accepted reply and its original row. No result aggregation, snapshot,
/// alias installation or continuation restoration is performed by this handoff.
#[must_use = "consume or deliberately drop the accepted native result"]
pub struct SequenceAccepted<'expression, Node, IrResult, Identity, Declaration: ?Sized, Reply> {
    index: usize,
    branch: &'expression ComputedBranch<Node, IrResult>,
    accepted: AcceptedReply<'expression, Identity, (), Declaration, Reply>,
}
impl<'expression, Node, IrResult, Identity, Declaration: ?Sized, Reply>
    SequenceAccepted<'expression, Node, IrResult, Identity, Declaration, Reply>
{
    pub fn into_parts(
        self,
    ) -> (
        usize,
        &'expression ComputedBranch<Node, IrResult>,
        Identity,
        &'expression Declaration,
        Reply,
    ) {
        let (identity, (), declaration, reply) = self.accepted.into_parts();
        (self.index, self.branch, identity, declaration, reply)
    }
}
impl<Node, IrResult, Identity, Declaration: ?Sized, Reply> fmt::Debug
    for SequenceAccepted<'_, Node, IrResult, Identity, Declaration, Reply>
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SequenceAccepted")
    }
}

enum State<'schema, Identity, Declaration: ?Sized> {
    Ready,
    Awaiting(PendingReply<'schema, Identity, (), Declaration>),
    Terminal(SequenceEnd),
}

/// One borrowed, non-serializable sequential execution with an owned fuel meter.
/// Only one request can await a reply. Polling while waiting never prepares,
/// charges or redispatches; successful identity/live-authority/type/value checks
/// advance the cursor before returning original row/declaration/reply ownership.
/// Ordinary rejection returns the unchanged input and leaves the current member
/// waiting. Cancellation closes before native cleanup. Preparation/acceptance
/// unwind closes as HostUncertain; no retry, fuel refund or automatic restoration.
///
/// This is an in-memory flat-Seq lifecycle, not a complete VM, batch admission,
/// receipt authentication, global deduplication or durable/exactly-once execution.
/// The host owns correlation/generation uniqueness, dispatch, cancellation delivery,
/// reply provenance, mutable native values and aggregation. Accepted output may be
/// dropped and does not itself grant authority. New sessions for the same group
/// are not deduplicated. No native Clone/Debug/serde/Send/Sync bound is imposed.
/// Host callbacks may unwind; a second panic in native cleanup can abort.
///
/// ```compile_fail
/// use leselang_hir::sequence_evaluation::SequenceEvaluation;
/// use leselang_runtime_core::ScalarTypeSet;
/// fn duplicate(session: SequenceEvaluation<'_, (), u32, (), (), u64, ScalarTypeSet>) {
///     let second = session.clone();
/// }
/// ```
///
/// ```compile_fail
/// use leselang_hir::sequence_evaluation::SequenceEvaluation;
/// use leselang_runtime_core::ScalarTypeSet;
/// fn persist(session: SequenceEvaluation<'_, (), u32, (), (), u64, ScalarTypeSet>) {
///     let encoded = serde_json::to_string(&session).unwrap();
/// }
/// ```
#[must_use = "retain the sequence session until completed, cancelled or deliberately dropped"]
pub struct SequenceSession<'expression, Node, IrResult, Identity, Declaration: ?Sized> {
    branches: &'expression [ComputedBranch<Node, IrResult>],
    next: usize,
    fuel: Fuel,
    state: State<'expression, Identity, Declaration>,
}

pub type SequenceEvaluation<
    'expression,
    Field,
    Operation,
    HostEffect,
    IrResult,
    Identity,
    Declaration,
> = SequenceSession<
    'expression,
    Node<Field, Operation, HostEffect, IrResult>,
    IrResult,
    Identity,
    Declaration,
>;

pub type SequenceAcceptance<
    'expression,
    Node,
    IrResult,
    Identity,
    Declaration,
    Reply,
    AuthorityError,
    ResultError,
> = Result<
    SequenceAccepted<'expression, Node, IrResult, Identity, Declaration, Reply>,
    SequenceRejected<Reply, AuthorityError, ResultError>,
>;

impl<'expression, Field, Operation, HostEffect, IrResult, Identity, Declaration: ?Sized>
    SequenceEvaluation<'expression, Field, Operation, HostEffect, IrResult, Identity, Declaration>
{
    /// Whole bounded physical/language shape and all atomic candidates precede
    /// every native hook. Then all cold declarations precede the single group-root
    /// debit. Inclusive ceilings are 16,384 nodes, depth 64 and 64 members. Zero
    /// capacity denies entry. Complete cold typing/opaque validation is native policy.
    pub fn start<Environment>(
        expression: &'expression Node<Field, Operation, HostEffect, IrResult>,
        environment: &Environment,
        mut fuel: Fuel,
        limits: SequenceEvaluationLimits,
    ) -> Result<Self, SequenceEvaluationError<Environment::Error>>
    where
        Environment: SequenceEvaluationEnvironment<
                'expression,
                Field,
                Operation,
                HostEffect,
                IrResult,
                Identity = Identity,
                Declaration = Declaration,
            >,
    {
        if limits.max_nodes > MAX_TYPE_INFERENCE_NODES
            || limits.max_depth > MAX_TYPE_INFERENCE_DEPTH
            || limits.max_branches > MAX_GROUP_EXPORT_MEMBERS
        {
            return Err(SequenceEvaluationError::InvalidLimits);
        }
        let Node::Group {
            group_kind: GroupKind::Sequence,
            branches,
        } = expression
        else {
            return Err(SequenceEvaluationError::NotSequence);
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
        .map_err(SequenceEvaluationError::Shape)?;
        for (index, branch) in branches.iter().enumerate() {
            if !atomic_candidate(&branch.value) {
                return Err(SequenceEvaluationError::Candidate { index });
            }
        }
        for (index, branch) in branches.iter().enumerate() {
            environment
                .preflight_member(index, branch)
                .map_err(|error| SequenceEvaluationError::Native {
                    index,
                    phase: SequencePhase::Preflight,
                    error,
                })?;
        }
        fuel.charge(1).map_err(|_| SequenceEvaluationError::Fuel)?;
        Ok(Self {
            branches,
            next: 0,
            fuel,
            state: State::Ready,
        })
    }

    /// Prepare exactly the ready member; no host callback runs while awaiting or
    /// terminal. Close before charging/native preparation so unwind cannot replay.
    pub fn poll<Environment>(
        &mut self,
        environment: &Environment,
    ) -> Result<SequencePoll<Environment::Request>, SequenceEvaluationError<Environment::Error>>
    where
        Environment: SequenceEvaluationEnvironment<
                'expression,
                Field,
                Operation,
                HostEffect,
                IrResult,
                Identity = Identity,
                Declaration = Declaration,
            >,
    {
        match self.status() {
            SequenceStatus::Awaiting { index } => return Ok(SequencePoll::Awaiting { index }),
            SequenceStatus::Terminal(end) => return Ok(SequencePoll::Terminal(end)),
            SequenceStatus::Ready { .. } => {}
        }
        self.state = State::Terminal(SequenceEnd::HostUncertain);
        if self.fuel.charge(1).is_err() {
            self.state = State::Terminal(SequenceEnd::Failed);
            return Err(SequenceEvaluationError::Fuel);
        }
        let prepared = match environment.prepare_member(
            self.next,
            &self.branches[self.next],
            &mut self.fuel,
        ) {
            Ok(prepared) => prepared,
            Err(error) => {
                self.state = State::Terminal(SequenceEnd::Failed);
                return Err(SequenceEvaluationError::Native {
                    index: self.next,
                    phase: SequencePhase::Prepare,
                    error,
                });
            }
        };
        self.state = State::Awaiting(PendingReply::new(
            prepared.identity,
            (),
            prepared.declaration,
        ));
        Ok(SequencePoll::Request {
            index: self.next,
            request: prepared.request,
        })
    }
}

impl<'expression, Node, IrResult, Identity, Declaration: ?Sized>
    SequenceSession<'expression, Node, IrResult, Identity, Declaration>
{
    pub fn status(&self) -> SequenceStatus {
        match &self.state {
            State::Ready => SequenceStatus::Ready { index: self.next },
            State::Awaiting(_) => SequenceStatus::Awaiting { index: self.next },
            State::Terminal(end) => SequenceStatus::Terminal(*end),
        }
    }

    pub const fn fuel_remaining(&self) -> u64 {
        self.fuel.remaining()
    }

    /// Reuse the shared reply order: exact identity, live authority, borrowed
    /// view, type, value. Rejection preserves input and waiting state. No next
    /// request is prepared implicitly, even after the last accepted result.
    pub fn try_accept<Reply, View: ?Sized, Authority>(
        &mut self,
        identity: &Identity,
        reply: Reply,
        authority: &Authority,
    ) -> SequenceAcceptance<
        'expression,
        Node,
        IrResult,
        Identity,
        Declaration,
        Reply,
        Authority::Error,
        Declaration::Error,
    >
    where
        Identity: PartialEq,
        Reply: Borrow<View>,
        Declaration: HostResultDomain<View>,
        Authority: ReplyAuthority<Identity>,
    {
        let status = self.status();
        if !matches!(status, SequenceStatus::Awaiting { .. }) {
            return Err(SequenceRejected {
                reply,
                error: SequenceReplyError::NotAwaiting(status),
            });
        }
        let State::Awaiting(mut pending) =
            std::mem::replace(&mut self.state, State::Terminal(SequenceEnd::HostUncertain))
        else {
            return Err(SequenceRejected {
                reply,
                error: SequenceReplyError::NotAwaiting(status),
            });
        };
        match pending.try_accept(identity, reply, authority) {
            Ok(accepted) => {
                let index = self.next;
                self.next += 1;
                self.state = if self.next == self.branches.len() {
                    State::Terminal(SequenceEnd::Completed)
                } else {
                    State::Ready
                };
                Ok(SequenceAccepted {
                    index,
                    branch: &self.branches[index],
                    accepted,
                })
            }
            Err(rejected) => {
                self.state = State::Awaiting(pending);
                Err(SequenceRejected {
                    reply: rejected.reply,
                    error: SequenceReplyError::Reply(rejected.error),
                })
            }
        }
    }

    /// Close before dropping pending native identity. Existing terminal reasons
    /// are preserved, including after a destructor panic or repeated cancellation.
    pub fn cancel(&mut self) -> bool {
        match std::mem::replace(&mut self.state, State::Terminal(SequenceEnd::Cancelled)) {
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

impl<Node, IrResult, Identity, Declaration: ?Sized> fmt::Debug
    for SequenceSession<'_, Node, IrResult, Identity, Declaration>
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SequenceEvaluation")
            .field("status", &self.status())
            .field("members", &self.branches.len())
            .field("fuel", &self.fuel_remaining())
            .finish()
    }
}
