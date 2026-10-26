//! Borrowed, in-memory sequential request/reply ownership, not durable dispatch.

#[cfg(test)]
#[path = "sequence_evaluation/transfer_tests.rs"]
mod transfer_tests;

use std::{borrow::Borrow, collections::VecDeque, convert::Infallible, fmt};

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
    pub(crate) fn new(
        index: usize,
        branch: &'expression ComputedBranch<Node, IrResult>,
        accepted: AcceptedReply<'expression, Identity, (), Declaration, Reply>,
    ) -> Self {
        Self {
            index,
            branch,
            accepted,
        }
    }

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

    /// Read-only original row and owned input borrows, not fresh native admission.
    pub fn as_parts(
        &self,
    ) -> (
        usize,
        &'expression ComputedBranch<Node, IrResult>,
        &Identity,
        &'expression Declaration,
        &Reply,
    ) {
        let (identity, (), declaration, reply) = self.accepted.as_parts();
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

/// Unchecked saved row state. Prepared means correlated but not yet published;
/// Awaiting means already published, not permission to dispatch it again.
/// Accepted data still requires current whole-snapshot host admission.
pub enum SavedSequenceReply<'schema, Identity, Declaration: ?Sized, Request, Reply> {
    Prepared(PreparedSequenceMember<'schema, Identity, Declaration, Request>),
    Awaiting {
        identity: Identity,
        declaration: &'schema Declaration,
    },
    Accepted(AcceptedReply<'schema, Identity, (), Declaration, Reply>),
}
impl<Identity, Declaration: ?Sized, Request, Reply>
    SavedSequenceReply<'_, Identity, Declaration, Request, Reply>
{
    pub fn identity(&self) -> &Identity {
        match self {
            Self::Prepared(member) => &member.identity,
            Self::Awaiting { identity, .. } => identity,
            Self::Accepted(accepted) => accepted.as_parts().0,
        }
    }
}
impl<Identity, Declaration: ?Sized, Request, Reply> fmt::Debug
    for SavedSequenceReply<'_, Identity, Declaration, Request, Reply>
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Prepared(_) => "Prepared",
            Self::Awaiting { .. } => "Awaiting",
            Self::Accepted(_) => "Accepted",
        })
    }
}

/// Original ordered saved data, not a receipt, graph certificate or replay grant.
pub struct SavedSequenceMember<'schema, Identity, Declaration: ?Sized, Request, Reply> {
    pub index: usize,
    pub name: &'schema str,
    pub reply: SavedSequenceReply<'schema, Identity, Declaration, Request, Reply>,
}
impl<Identity, Declaration: ?Sized, Request, Reply> fmt::Debug
    for SavedSequenceMember<'_, Identity, Declaration, Request, Reply>
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SavedSequenceMember")
            .field("index", &self.index)
            .field("reply", &self.reply)
            .finish()
    }
}

pub enum SequenceRestoreError<Native> {
    ScopeClosed,
    Sequence(SequenceEvaluationError<Native>),
    Count,
    Row { index: usize },
    Frontier { index: usize },
    DuplicateIdentity { index: usize, previous: usize },
    Native(Native),
}
impl<Native> fmt::Display for SequenceRestoreError<Native> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::ScopeClosed => "sequence restoration scope is closed",
            Self::Sequence(_) => "saved sequence shape rejected",
            Self::Count => "saved sequence rows must cover the complete group",
            Self::Row { .. } => "saved sequence row differs from original declaration order",
            Self::Frontier { .. } => "saved sequence completion or publication frontier differs",
            Self::DuplicateIdentity { .. } => "saved sequence member identities are not distinct",
            Self::Native(_) => "saved sequence whole-snapshot native admission rejected",
        })
    }
}
impl<Native> fmt::Debug for SequenceRestoreError<Native> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}
impl<Native> std::error::Error for SequenceRestoreError<Native> {}

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
/// use leselang_hir_core::sequence_evaluation::SequenceEvaluation;
/// use leselang_runtime_core::ScalarTypeSet;
/// fn duplicate(session: SequenceEvaluation<'_, (), u32, (), (), u64, ScalarTypeSet>) {
///     let second = session.clone();
/// }
/// ```
///
/// ```compile_fail
/// use leselang_hir_core::sequence_evaluation::SequenceEvaluation;
/// use leselang_runtime_core::ScalarTypeSet;
/// fn persist(session: SequenceEvaluation<'_, (), u32, (), (), u64, ScalarTypeSet>) {
///     let encoded = serde_json::to_string(&session);
/// }
/// ```
#[must_use = "retain the sequence session until completed, cancelled or deliberately dropped"]
pub struct SequenceSession<'expression, Node, IrResult, Identity, Declaration: ?Sized> {
    core: SequenceReplyCore<'expression, Node, IrResult, Identity, Declaration>,
    fuel: Fuel,
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

#[expect(
    clippy::type_complexity,
    reason = "shape admission retains the original native IR slots"
)]
pub(crate) fn admit_sequence_shape<Field, Operation, HostEffect, IrResult, Error>(
    expression: &Node<Field, Operation, HostEffect, IrResult>,
    limits: SequenceEvaluationLimits,
) -> Result<&[Branch<Field, Operation, HostEffect, IrResult>], SequenceEvaluationError<Error>> {
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
    Ok(branches)
}

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
        let branches = admit_sequence_shape(expression, limits)?;
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
            core: SequenceReplyCore::new(branches),
            fuel,
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
        self.poll_with_scope(environment, None)
    }

    /// Recheck explicit local liveness before publication and after native
    /// preparation. Cancellation publishes no request; spent fuel is retained.
    pub fn poll_in_scope<Environment>(
        &mut self,
        environment: &Environment,
        scope: &LocalExecutionScope,
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
        self.poll_with_scope(environment, Some(scope))
    }

    fn poll_with_scope<Environment>(
        &mut self,
        environment: &Environment,
        scope: Option<&LocalExecutionScope>,
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
        self.core.publish_with(
            |index, branch| {
                self.fuel
                    .charge(1)
                    .map_err(|_| SequenceEvaluationError::Fuel)?;
                environment
                    .prepare_member(index, branch, &mut self.fuel)
                    .map_err(|error| SequenceEvaluationError::Native {
                        index,
                        phase: SequencePhase::Prepare,
                        error,
                    })
            },
            scope,
        )
    }
}

impl<'expression, Node, IrResult, Identity, Declaration: ?Sized>
    SequenceSession<'expression, Node, IrResult, Identity, Declaration>
{
    pub fn status(&self) -> SequenceStatus {
        self.core.status()
    }

    pub const fn fuel_remaining(&self) -> u64 {
        self.fuel.remaining()
    }

    /// Move the original meter only after all accepted member handoffs. This is
    /// budget ownership, not a refill, group-result certificate or continuation.
    /// Other states return the unchanged session without native work or cleanup.
    pub fn into_completed_fuel(self) -> Result<Fuel, Self> {
        if self.status() == SequenceStatus::Terminal(SequenceEnd::Completed) {
            Ok(self.fuel)
        } else {
            Err(self)
        }
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
        self.core.try_accept(identity, reply, authority)
    }

    /// Explicit local liveness for this acceptance, not stored scope ownership.
    /// Revocation cancels the collector; ordinary rejection remains retryable.
    /// Already handed-off requests/results and enclosing execution are host-owned.
    pub fn try_accept_in_scope<Reply, View: ?Sized, Authority>(
        &mut self,
        identity: &Identity,
        reply: Reply,
        authority: &Authority,
        scope: &LocalExecutionScope,
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
        self.core
            .try_accept_with_scope(identity, reply, authority, Some(scope))
    }

    /// Close before dropping pending native identity. Existing terminal reasons
    /// are preserved, including after a destructor panic or repeated cancellation.
    pub fn cancel(&mut self) -> bool {
        self.core.cancel()
    }
}

pub(crate) struct SequenceReplyCore<'expression, Node, IrResult, Identity, Declaration: ?Sized> {
    branches: &'expression [ComputedBranch<Node, IrResult>],
    next: usize,
    state: State<'expression, Identity, Declaration>,
}
impl<'expression, Node, IrResult, Identity, Declaration: ?Sized>
    SequenceReplyCore<'expression, Node, IrResult, Identity, Declaration>
{
    pub(crate) fn new(branches: &'expression [ComputedBranch<Node, IrResult>]) -> Self {
        Self {
            branches,
            next: 0,
            state: State::Ready,
        }
    }
    pub(crate) fn member_count(&self) -> usize {
        self.branches.len()
    }

    #[expect(
        clippy::type_complexity,
        reason = "refusal returns all original core, queued request and accepted prefix owners"
    )]
    pub(crate) fn try_into_saved<Request, Reply>(
        mut self,
        prepared: VecDeque<PreparedSequenceMember<'expression, Identity, Declaration, Request>>,
        accepted: Vec<SequenceAccepted<'expression, Node, IrResult, Identity, Declaration, Reply>>,
    ) -> Result<
        Vec<SavedSequenceMember<'expression, Identity, Declaration, Request, Reply>>,
        (
            Self,
            VecDeque<PreparedSequenceMember<'expression, Identity, Declaration, Request>>,
            Vec<SequenceAccepted<'expression, Node, IrResult, Identity, Declaration, Reply>>,
        ),
    > {
        if self.next > self.branches.len()
            || accepted.len() != self.next
            || accepted.iter().enumerate().any(|(index, row)| {
                row.index != index || !std::ptr::eq(row.branch, &self.branches[index])
            })
        {
            return Err((self, prepared, accepted));
        }
        let awaiting = match &self.state {
            State::Ready if self.next < self.branches.len() => false,
            State::Awaiting(_) if self.next < self.branches.len() => true,
            State::Terminal(SequenceEnd::Completed) if self.next == self.branches.len() => false,
            _ => return Err((self, prepared, accepted)),
        };
        if prepared.len() != self.branches.len() - self.next - usize::from(awaiting) {
            return Err((self, prepared, accepted));
        }
        let mut rows = Vec::with_capacity(self.branches.len());
        let waiting =
            match std::mem::replace(&mut self.state, State::Terminal(SequenceEnd::HostUncertain)) {
                State::Awaiting(pending) => match pending.try_into_parts() {
                    Ok((identity, (), declaration)) => Some((identity, declaration)),
                    Err(pending) => {
                        self.state = State::Awaiting(pending);
                        return Err((self, prepared, accepted));
                    }
                },
                state => {
                    self.state = state;
                    None
                }
            };
        for row in accepted {
            rows.push(SavedSequenceMember {
                index: row.index,
                name: &row.branch.name,
                reply: SavedSequenceReply::Accepted(row.accepted),
            });
        }
        if let Some((identity, declaration)) = waiting {
            rows.push(SavedSequenceMember {
                index: self.next,
                name: &self.branches[self.next].name,
                reply: SavedSequenceReply::Awaiting {
                    identity,
                    declaration,
                },
            });
        }
        for member in prepared {
            let index = rows.len();
            rows.push(SavedSequenceMember {
                index,
                name: &self.branches[index].name,
                reply: SavedSequenceReply::Prepared(member),
            });
        }
        Ok(rows)
    }

    #[expect(
        clippy::type_complexity,
        reason = "restore moves the original core, unpublished requests and accepted prefix separately"
    )]
    pub(crate) fn restore<Request, Reply, Error>(
        branches: &'expression [ComputedBranch<Node, IrResult>],
        rows: Vec<SavedSequenceMember<'expression, Identity, Declaration, Request, Reply>>,
        admit: impl FnOnce(
            &[SavedSequenceMember<'expression, Identity, Declaration, Request, Reply>],
        ) -> Result<(), Error>,
        scope: Option<&LocalExecutionScope>,
    ) -> Result<
        (
            Self,
            VecDeque<PreparedSequenceMember<'expression, Identity, Declaration, Request>>,
            Vec<SequenceAccepted<'expression, Node, IrResult, Identity, Declaration, Reply>>,
        ),
        SequenceRestoreError<Error>,
    >
    where
        Identity: PartialEq,
    {
        let fence = CallFence::new(scope);
        let preflight = (|| {
            if scope.is_some_and(|scope| !scope.is_open()) {
                return Err(SequenceRestoreError::ScopeClosed);
            }
            if rows.len() != branches.len() {
                return Err(SequenceRestoreError::Count);
            }
            for (index, row) in rows.iter().enumerate() {
                if row.index != index || row.name != branches[index].name {
                    return Err(SequenceRestoreError::Row { index });
                }
            }
            let next = rows
                .iter()
                .take_while(|row| matches!(row.reply, SavedSequenceReply::Accepted(_)))
                .count();
            for (index, row) in rows.iter().enumerate().skip(next) {
                if matches!(row.reply, SavedSequenceReply::Accepted(_))
                    || (index != next && matches!(row.reply, SavedSequenceReply::Awaiting { .. }))
                {
                    return Err(SequenceRestoreError::Frontier { index });
                }
            }
            for (index, row) in rows.iter().enumerate() {
                for (previous, earlier) in rows[..index].iter().enumerate() {
                    let duplicate = row.reply.identity() == earlier.reply.identity();
                    if scope.is_some_and(|scope| !scope.is_open()) {
                        return Err(SequenceRestoreError::ScopeClosed);
                    }
                    if duplicate {
                        return Err(SequenceRestoreError::DuplicateIdentity { index, previous });
                    }
                }
            }
            Ok(next)
        })();
        let next = match preflight {
            Ok(next) => next,
            Err(error) => {
                fence.close();
                drop((admit, rows));
                fence.finish();
                return Err(error);
            }
        };
        let admitted = admit(&rows);
        let admitted = if scope.is_some_and(|scope| !scope.is_open()) {
            drop(admitted);
            Err(SequenceRestoreError::ScopeClosed)
        } else {
            admitted.map_err(SequenceRestoreError::Native)
        };
        if let Err(error) = admitted {
            fence.close();
            drop(rows);
            fence.finish();
            return Err(error);
        }
        let mut prepared = VecDeque::new();
        let mut accepted = Vec::with_capacity(next);
        let mut state = if next == branches.len() {
            State::Terminal(SequenceEnd::Completed)
        } else {
            State::Ready
        };
        for (index, row) in rows.into_iter().enumerate() {
            match row.reply {
                SavedSequenceReply::Prepared(member) => prepared.push_back(member),
                SavedSequenceReply::Awaiting {
                    identity,
                    declaration,
                } => {
                    state = State::Awaiting(PendingReply::new(identity, (), declaration));
                }
                SavedSequenceReply::Accepted(reply) => {
                    accepted.push(SequenceAccepted::new(index, &branches[index], reply));
                }
            }
        }
        let output = Ok((
            Self {
                branches,
                next,
                state,
            },
            prepared,
            accepted,
        ));
        fence.finish();
        output
    }

    pub(crate) fn status(&self) -> SequenceStatus {
        match &self.state {
            State::Ready => SequenceStatus::Ready { index: self.next },
            State::Awaiting(_) => SequenceStatus::Awaiting { index: self.next },
            State::Terminal(end) => SequenceStatus::Terminal(*end),
        }
    }
    pub(crate) fn publish_with<Request, Error>(
        &mut self,
        prepare: impl FnOnce(
            usize,
            &'expression ComputedBranch<Node, IrResult>,
        ) -> Result<
            PreparedSequenceMember<'expression, Identity, Declaration, Request>,
            Error,
        >,
        scope: Option<&LocalExecutionScope>,
    ) -> Result<SequencePoll<Request>, Error> {
        if scope.is_some_and(|scope| !scope.is_open()) {
            self.cancel();
        }
        match self.status() {
            SequenceStatus::Awaiting { index } => return Ok(SequencePoll::Awaiting { index }),
            SequenceStatus::Terminal(end) => return Ok(SequencePoll::Terminal(end)),
            SequenceStatus::Ready { .. } => {}
        }
        self.state = State::Terminal(SequenceEnd::HostUncertain);
        let fence = CallFence::new(scope);
        let outcome = prepare(self.next, &self.branches[self.next]);
        if scope.is_some_and(|scope| !scope.is_open()) {
            self.state = State::Terminal(SequenceEnd::Cancelled);
            drop(outcome);
            fence.finish();
            return Ok(SequencePoll::Terminal(SequenceEnd::Cancelled));
        }
        let prepared = match outcome {
            Ok(prepared) => prepared,
            Err(error) => {
                self.state = State::Terminal(SequenceEnd::Failed);
                fence.close();
                fence.finish();
                return Err(error);
            }
        };
        self.state = State::Awaiting(PendingReply::new(
            prepared.identity,
            (),
            prepared.declaration,
        ));
        fence.finish();
        Ok(SequencePoll::Request {
            index: self.next,
            request: prepared.request,
        })
    }
    pub(crate) fn try_accept<Reply, View: ?Sized, Authority>(
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
        self.try_accept_with_scope(identity, reply, authority, None)
    }

    pub(crate) fn try_accept_with_scope<Reply, View: ?Sized, Authority>(
        &mut self,
        identity: &Identity,
        reply: Reply,
        authority: &Authority,
        scope: Option<&LocalExecutionScope>,
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
        if scope.is_some_and(|scope| !scope.is_open()) {
            self.cancel();
        }
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
        let outcome = match scope {
            Some(scope) => pending.try_accept_in_scope(identity, reply, authority, scope),
            None => pending.try_accept(identity, reply, authority),
        };
        match outcome {
            Ok(accepted) => {
                let index = self.next;
                self.next += 1;
                self.state = if self.next == self.branches.len() {
                    State::Terminal(SequenceEnd::Completed)
                } else {
                    State::Ready
                };
                Ok(SequenceAccepted::new(
                    index,
                    &self.branches[index],
                    accepted,
                ))
            }
            Err(rejected) => {
                self.state = if matches!(
                    rejected.error,
                    ReplyAcceptanceError::Closed(ReplyEnd::Cancelled)
                ) {
                    State::Terminal(SequenceEnd::Cancelled)
                } else {
                    State::Awaiting(pending)
                };
                Err(SequenceRejected {
                    reply: rejected.reply,
                    error: SequenceReplyError::Reply(rejected.error),
                })
            }
        }
    }

    /// Close before dropping pending native identity. Existing terminal reasons
    /// are preserved, including after a destructor panic or repeated cancellation.
    pub(crate) fn cancel(&mut self) -> bool {
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
            .field("members", &self.core.member_count())
            .field("fuel", &self.fuel_remaining())
            .finish()
    }
}
