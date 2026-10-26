//! Complete scoped correlation with fresh, one-at-a-time sequential publication.

use std::{borrow::Borrow, collections::VecDeque, fmt};

use leselang_runtime_core::{Fuel, HostResultDomain, LocalExecutionScope, ReplyAuthority};

use crate::ir::{Computation, ComputedBranch, GroupKind};
use crate::local_scope::CallFence;
use crate::native_group_preparation::PreparedNativeGroup;
use crate::sequence_evaluation::{
    PreparedSequenceMember, SavedSequenceMember, SequenceAcceptance, SequenceAccepted,
    SequenceEvaluationError, SequenceEvaluationLimits, SequencePhase, SequencePoll,
    SequenceReplyCore, SequenceRestoreError, SequenceStatus, admit_sequence_shape,
};

pub enum PreparedSequenceError<Error> {
    OriginalGroup,
    Sequence(SequenceEvaluationError<Error>),
    DuplicateIdentity { index: usize, previous: usize },
    InternalState,
}
impl<Error> fmt::Display for PreparedSequenceError<Error> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::OriginalGroup => "prepared sequence rows do not belong to the original group",
            Self::Sequence(_) => "prepared sequence admission or correlation rejected",
            Self::DuplicateIdentity { .. } => {
                "prepared sequence member identities are not distinct"
            }
            Self::InternalState => "prepared sequence internal ownership mismatch",
        })
    }
}
impl<Error> fmt::Debug for PreparedSequenceError<Error> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}
impl<Error> std::error::Error for PreparedSequenceError<Error> {}

/// Opt-in complete operand/correlation preparation, then one outstanding request.
/// Start consumes complete original Seq preparations: shared shape/candidates and
/// exact row correspondence, then all fresh native admissions, precede correlation
/// or additional fuel. A root unit plus one per correlation uses the enclosing
/// original meter; each native callback bounds/charges its own work and consumes
/// an already prepared operand once. Every identity must be distinct before start
/// returns. Correlation is eager, publication is sequential; callbacks must not
/// dispatch. Arguments are not reevaluated and no second meter is created.
///
/// Poll requires fresh native admission of the original row, identity, domain and
/// retained request before releasing exactly one request. Admission may inspect
/// current schema/grants/provenance, not mutate operands or dispatch. Native policy
/// work/aggregate memory/ingress limits and actual dispatch remain host-owned;
/// an admission success is not a future grant or authentic external receipt.
/// Waiting/terminal polls do no native work. This wrapper owns no fuel; bounded
/// publication checks do not debit the core's meter or run argument calculations.
/// Hosts reserve/limit that native work explicitly, not via an implicit refill.
///
/// The same private SequenceReplyCore as legacy SequenceSession checks identity,
/// live authority and original result domain and returns each accepted original
/// row once. Acceptance does not publish a successor or assemble a group result.
/// Rejection preserves input and queued work. Cancellation, publication refusal
/// or any native unwind fences the lifecycle before dropping all retained future
/// requests/identities. Accepted rows and already published requests are external
/// ownership; cancellation delivery/aggregation remain explicit host orchestration.
/// No native Clone/Debug/serde/Send bound is added beyond identity PartialEq.
/// This is not scheduling, transactional external dispatch, durable recovery,
/// nested member execution, product VM migration or independent VM promotion.
/// Callbacks/Drop are trusted, cannot be rolled back/preempted, and a second panic
/// during cleanup may abort. Legacy lazy preparation and fuel costs are unchanged.
///
/// ```compile_fail
/// use leselang_hir_core::prepared_sequence::PreparedSequenceSession;
/// fn copy(session: PreparedSequenceSession<'_, (), (), u8, (), ()>) {
///     let copied = session.clone();
/// }
/// ```
///
/// ```compile_fail
/// use leselang_hir_core::prepared_sequence::PreparedSequenceSession;
/// fn wire(session: PreparedSequenceSession<'_, (), (), u8, (), ()>) {
///     let encoded = serde_json::to_string(&session);
/// }
/// ```
#[must_use = "retain the prepared sequence until completion or cancellation"]
pub struct PreparedSequenceSession<
    'expression,
    Node,
    IrResult,
    Identity,
    Declaration: ?Sized,
    Request,
> {
    core: SequenceReplyCore<'expression, Node, IrResult, Identity, Declaration>,
    prepared: VecDeque<PreparedSequenceMember<'expression, Identity, Declaration, Request>>,
}

pub type RestoredPreparedSequence<
    'expression,
    Node,
    IrResult,
    Identity,
    Declaration,
    Request,
    Reply,
    Error,
> = Result<
    (
        PreparedSequenceSession<'expression, Node, IrResult, Identity, Declaration, Request>,
        Vec<SequenceAccepted<'expression, Node, IrResult, Identity, Declaration, Reply>>,
    ),
    SequenceRestoreError<Error>,
>;

impl<'expression, Field, Operation, HostEffect, IrResult, Identity, Declaration: ?Sized, Request>
    PreparedSequenceSession<
        'expression,
        Computation<Field, Operation, HostEffect, IrResult>,
        IrResult,
        Identity,
        Declaration,
        Request,
    >
{
    /// Restore a complete saved Seq as a contiguous accepted prefix, at most one
    /// awaiting member, then correlated unpublished requests. The cursor is
    /// derived, never supplied. Whole shape/order/frontier checks precede native
    /// equality; distinct identities precede mandatory complete-snapshot admission.
    /// The original core resumes without preparing operands or creating fuel.
    /// Awaiting work is not republished; each queued request still requires fresh
    /// poll admission. The original accepted rows move back to host aggregation;
    /// an all-accepted group returns a Completed session, never a Ready past-end.
    ///
    /// The host must authenticate original graph, domains, requests, receipts,
    /// publication/settlement state, saved budget and current authority/deadline.
    /// Accepted wrappers and well-formed saved data are not live/replay authority.
    /// Do not rearm cancelled or settled work. No codec, journal, dispatch, meter,
    /// retry or crash transaction is supplied. Native Eq/admission/Drop are trusted
    /// bounded work; unwind consumes inputs and a second cleanup panic may abort.
    #[expect(
        clippy::type_complexity,
        reason = "restored session and prefix preserve original IR, request and reply ownership"
    )]
    pub fn restore<Reply, Error>(
        group: &'expression Computation<Field, Operation, HostEffect, IrResult>,
        rows: Vec<SavedSequenceMember<'expression, Identity, Declaration, Request, Reply>>,
        limits: SequenceEvaluationLimits,
        admit: impl FnOnce(
            &[SavedSequenceMember<'expression, Identity, Declaration, Request, Reply>],
        ) -> Result<(), Error>,
    ) -> RestoredPreparedSequence<
        'expression,
        Computation<Field, Operation, HostEffect, IrResult>,
        IrResult,
        Identity,
        Declaration,
        Request,
        Reply,
        Error,
    >
    where
        Identity: PartialEq,
    {
        Self::restore_with_scope(group, rows, limits, admit, None)
    }

    /// Restore with explicit local liveness, retaining no scope in the collector.
    /// A closed scope refuses before structural/native checks. Shape, equality
    /// or native refusal/unwind revokes before retained rows and unused-hook
    /// cleanup. Revocation inside equality/admission wins over ordinary results
    /// and stops later hooks. Success retains original prefix/request owners and
    /// leaves the scope open. Later publication and acceptance must use the same
    /// scope. No dispatch, meter, external cancellation or durable replay is
    /// supplied; callback-internal cleanup and native Drop remain trusted work.
    #[expect(
        clippy::type_complexity,
        reason = "restored session and prefix preserve distinct native request and reply owners"
    )]
    pub fn restore_in_scope<Reply, Error>(
        group: &'expression Computation<Field, Operation, HostEffect, IrResult>,
        rows: Vec<SavedSequenceMember<'expression, Identity, Declaration, Request, Reply>>,
        limits: SequenceEvaluationLimits,
        admit: impl FnOnce(
            &[SavedSequenceMember<'expression, Identity, Declaration, Request, Reply>],
        ) -> Result<(), Error>,
        scope: &LocalExecutionScope,
    ) -> RestoredPreparedSequence<
        'expression,
        Computation<Field, Operation, HostEffect, IrResult>,
        IrResult,
        Identity,
        Declaration,
        Request,
        Reply,
        Error,
    >
    where
        Identity: PartialEq,
    {
        Self::restore_with_scope(group, rows, limits, admit, Some(scope))
    }

    #[expect(
        clippy::type_complexity,
        reason = "shared restoration returns the original session and separate accepted prefix"
    )]
    fn restore_with_scope<Reply, Error>(
        group: &'expression Computation<Field, Operation, HostEffect, IrResult>,
        rows: Vec<SavedSequenceMember<'expression, Identity, Declaration, Request, Reply>>,
        limits: SequenceEvaluationLimits,
        admit: impl FnOnce(
            &[SavedSequenceMember<'expression, Identity, Declaration, Request, Reply>],
        ) -> Result<(), Error>,
        scope: Option<&LocalExecutionScope>,
    ) -> RestoredPreparedSequence<
        'expression,
        Computation<Field, Operation, HostEffect, IrResult>,
        IrResult,
        Identity,
        Declaration,
        Request,
        Reply,
        Error,
    >
    where
        Identity: PartialEq,
    {
        let fence = CallFence::new(scope);
        let shape = if scope.is_some_and(|scope| !scope.is_open()) {
            Err(SequenceRestoreError::ScopeClosed)
        } else {
            admit_sequence_shape(group, limits).map_err(SequenceRestoreError::Sequence)
        };
        let branches = match shape {
            Ok(branches) => branches,
            Err(error) => {
                fence.close();
                drop((admit, rows));
                fence.finish();
                return Err(error);
            }
        };
        let output = SequenceReplyCore::restore(branches, rows, admit, scope)
            .map(|(core, prepared, accepted)| (Self { core, prepared }, accepted));
        fence.finish();
        output
    }

    #[expect(
        clippy::type_complexity,
        reason = "native prepared operands and original reply domains retain their distinct lifetimes"
    )]
    pub fn start<Prepared, Error>(
        group: &'expression Computation<Field, Operation, HostEffect, IrResult>,
        prepared: PreparedNativeGroup<
            'expression,
            ComputedBranch<Computation<Field, Operation, HostEffect, IrResult>, IrResult>,
            Prepared,
        >,
        fuel: &mut Fuel,
        limits: SequenceEvaluationLimits,
        mut admit: impl FnMut(
            usize,
            &'expression ComputedBranch<
                Computation<Field, Operation, HostEffect, IrResult>,
                IrResult,
            >,
            &Prepared,
        ) -> Result<(), Error>,
        mut correlate: impl FnMut(
            usize,
            &'expression ComputedBranch<
                Computation<Field, Operation, HostEffect, IrResult>,
                IrResult,
            >,
            Prepared,
            &mut Fuel,
        ) -> Result<
            PreparedSequenceMember<'expression, Identity, Declaration, Request>,
            Error,
        >,
    ) -> Result<Self, PreparedSequenceError<Error>>
    where
        Identity: PartialEq,
    {
        let branches =
            admit_sequence_shape(group, limits).map_err(PreparedSequenceError::Sequence)?;
        if prepared.metadata().kind() != GroupKind::Sequence
            || !std::ptr::eq(prepared.metadata().members(), branches)
            || prepared.members().len() != branches.len()
        {
            return Err(PreparedSequenceError::OriginalGroup);
        }
        for (index, row) in prepared.members().iter().enumerate() {
            if row.index() != index || !std::ptr::eq(row.original(), &branches[index]) {
                return Err(PreparedSequenceError::OriginalGroup);
            }
        }
        for (index, row) in prepared.members().iter().enumerate() {
            admit(index, row.original(), row.prepared()).map_err(|error| {
                PreparedSequenceError::Sequence(SequenceEvaluationError::Native {
                    index,
                    phase: SequencePhase::Preflight,
                    error,
                })
            })?;
        }
        fuel.charge(1)
            .map_err(|_| PreparedSequenceError::Sequence(SequenceEvaluationError::Fuel))?;
        let (_, rows) = prepared.into_parts();
        let mut correlated = VecDeque::with_capacity(branches.len());
        for row in rows {
            let (index, original, operand) = row.into_parts();
            fuel.charge(1)
                .map_err(|_| PreparedSequenceError::Sequence(SequenceEvaluationError::Fuel))?;
            correlated.push_back(correlate(index, original, operand, fuel).map_err(|error| {
                PreparedSequenceError::Sequence(SequenceEvaluationError::Native {
                    index,
                    phase: SequencePhase::Prepare,
                    error,
                })
            })?);
        }
        for (index, member) in correlated.iter().enumerate() {
            for (previous, earlier) in correlated.iter().take(index).enumerate() {
                if member.identity == earlier.identity {
                    return Err(PreparedSequenceError::DuplicateIdentity { index, previous });
                }
            }
        }
        Ok(Self {
            core: SequenceReplyCore::new(branches),
            prepared: correlated,
        })
    }
}
impl<'expression, Node, IrResult, Identity, Declaration: ?Sized, Request>
    PreparedSequenceSession<'expression, Node, IrResult, Identity, Declaration, Request>
{
    pub fn status(&self) -> SequenceStatus {
        self.core.status()
    }

    /// Move the original accepted prefix, pending identity/domain and unpublished
    /// requests into ordered saved rows. Prefix length, original row pointers and
    /// indices must match the current frontier; refusal returns both owners intact.
    /// No native equality, admission, cloning, dispatch or fuel change occurs.
    /// The bounded row vector allocates. A completed queue with its full prefix
    /// yields all-accepted rows, which restore as Completed, never runnable work.
    /// Cancelled/failed/uncertain queues refuse even with a well-formed prefix.
    ///
    /// Original IR correspondence is not receipt provenance: the host must prove
    /// the prefix belongs to this execution, current authority, original domains,
    /// request inventory, budget and nonterminal settlement during restore.
    /// Scope, meter, published requests and cancellation delivery stay host-owned.
    /// This is not a codec, durable commit, replay grant or external cancellation.
    #[expect(
        clippy::type_complexity,
        reason = "failed transfer returns the original queue and separate native accepted prefix"
    )]
    pub fn try_into_saved<Reply>(
        self,
        accepted: Vec<SequenceAccepted<'expression, Node, IrResult, Identity, Declaration, Reply>>,
    ) -> Result<
        Vec<SavedSequenceMember<'expression, Identity, Declaration, Request, Reply>>,
        (
            Self,
            Vec<SequenceAccepted<'expression, Node, IrResult, Identity, Declaration, Reply>>,
        ),
    > {
        self.core
            .try_into_saved(self.prepared, accepted)
            .map_err(|(core, prepared, accepted)| (Self { core, prepared }, accepted))
    }

    /// Fresh publication policy runs only for the next ready original member.
    pub fn poll<Error>(
        &mut self,
        admit: impl FnOnce(
            usize,
            &'expression ComputedBranch<Node, IrResult>,
            &Identity,
            &'expression Declaration,
            &Request,
        ) -> Result<(), Error>,
    ) -> Result<SequencePoll<Request>, PreparedSequenceError<Error>> {
        self.poll_with_scope(admit, None)
    }

    /// Explicit liveness for publication, without storing a scope. Closure in
    /// the admission hook discards this request and the unpublished suffix.
    pub fn poll_in_scope<Error>(
        &mut self,
        admit: impl FnOnce(
            usize,
            &'expression ComputedBranch<Node, IrResult>,
            &Identity,
            &'expression Declaration,
            &Request,
        ) -> Result<(), Error>,
        scope: &LocalExecutionScope,
    ) -> Result<SequencePoll<Request>, PreparedSequenceError<Error>> {
        self.poll_with_scope(admit, Some(scope))
    }

    fn poll_with_scope<Error>(
        &mut self,
        admit: impl FnOnce(
            usize,
            &'expression ComputedBranch<Node, IrResult>,
            &Identity,
            &'expression Declaration,
            &Request,
        ) -> Result<(), Error>,
        scope: Option<&LocalExecutionScope>,
    ) -> Result<SequencePoll<Request>, PreparedSequenceError<Error>> {
        // Local ownership releases the entire retained suffix on callback unwind.
        let mut prepared = std::mem::take(&mut self.prepared);
        let fence = CallFence::new(scope);
        let outcome = self.core.publish_with(
            |index, original| {
                let member = prepared
                    .front()
                    .ok_or(PreparedSequenceError::InternalState)?;
                admit(
                    index,
                    original,
                    &member.identity,
                    member.declaration,
                    &member.request,
                )
                .map_err(|error| {
                    PreparedSequenceError::Sequence(SequenceEvaluationError::Native {
                        index,
                        phase: SequencePhase::Preflight,
                        error,
                    })
                })?;
                prepared
                    .pop_front()
                    .ok_or(PreparedSequenceError::InternalState)
            },
            scope,
        );
        if !matches!(self.status(), SequenceStatus::Terminal(_)) {
            self.prepared = prepared;
        } else {
            drop(prepared);
        }
        fence.finish();
        outcome
    }
    /// Accept one original result, without running the next publication hook.
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
        self.try_accept_with_scope(identity, reply, authority, None)
    }

    /// Cancel this collector on explicit scope revocation, before releasing its
    /// retained suffix. This does not cancel external dispatch or its parent VM.
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
        self.try_accept_with_scope(identity, reply, authority, Some(scope))
    }

    fn try_accept_with_scope<Reply, View: ?Sized, Authority>(
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
        let prepared = std::mem::take(&mut self.prepared);
        let fence = CallFence::new(scope);
        let outcome = self
            .core
            .try_accept_with_scope(identity, reply, authority, scope);
        if !matches!(self.status(), SequenceStatus::Terminal(_)) {
            self.prepared = prepared;
        } else {
            drop(prepared);
        }
        fence.finish();
        outcome
    }
    pub fn cancel(&mut self) -> bool {
        let prepared = std::mem::take(&mut self.prepared);
        let closed = self.core.cancel();
        drop(prepared);
        closed
    }
}
impl<Node, IrResult, Identity, Declaration: ?Sized, Request> fmt::Debug
    for PreparedSequenceSession<'_, Node, IrResult, Identity, Declaration, Request>
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PreparedSequenceSession")
            .field("status", &self.status())
            .field("members", &self.core.member_count())
            .field("retained", &self.prepared.len())
            .finish()
    }
}
