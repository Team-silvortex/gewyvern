//! Prepared scoped operands joined to the existing original-row reply barrier.

use std::{borrow::Borrow, convert::Infallible, fmt};

use leselang_runtime_core::{Fuel, HostResultDomain, LocalExecutionScope, ReplyAuthority};

use crate::ir::{Computation, ComputedBranch, GroupKind};
use crate::local_scope::CallFence;
use crate::native_group_preparation::PreparedNativeGroup;
use crate::parallel_evaluation::{
    ParallelAcceptance, ParallelEvaluationError, ParallelEvaluationLimits, ParallelEvaluationPoll,
    ParallelPhase, ParallelPoll, ParallelReplyCore, ParallelRequest, ParallelRestoreError,
    ParallelStatus, PreparedParallelMember, SavedParallelMember, admit_parallel_shape,
};

pub enum PreparedParallelError<Error> {
    OriginalGroup,
    Parallel(ParallelEvaluationError<Error>),
}
impl<Error> fmt::Display for PreparedParallelError<Error> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::OriginalGroup => "prepared parallel rows do not belong to the original group",
            Self::Parallel(_) => "prepared parallel admission or correlation rejected",
        })
    }
}
impl<Error> fmt::Debug for PreparedParallelError<Error> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}
impl<Error> std::error::Error for PreparedParallelError<Error> {}

pub type PreparedParallelStart<
    'expression,
    Node,
    IrResult,
    Identity,
    Declaration,
    Reply,
    Request,
    Error,
> = Result<
    (
        PreparedParallelSession<'expression, Node, IrResult, Identity, Declaration, Reply>,
        Vec<ParallelRequest<Request>>,
    ),
    PreparedParallelError<Error>,
>;

/// Opt-in complete scoped preparations, with the exact existing All reply barrier.
/// Start consumes one complete PreparedNativeGroup against its original flat IR.
/// Whole shape/candidate/row correspondence and every native preparation admission
/// precede correlation/fuel; all identities must be distinct before handoff.
/// One correlation root unit and one per member use the caller's borrowed meter;
/// native callbacks must bound/charge correlation and dispatch work and recheck
/// original domains/live policy. No original argument is reevaluated or copied.
/// Earlier argument-preparation costs are separate and are never refunded/reset.
///
/// Returned requests represent complete local publication, not atomic external
/// dispatch. Acceptance/poll/cancel delegate to the same private ParallelReplyCore
/// as legacy ParallelSession: arbitrary arrival order, original index/domain/live
/// authority, unchanged rejection, all-success ordered output, once-only terminal
/// handoff and cleanup fencing. This session owns no fuel and cannot refill or
/// redeem a root meter; the enclosing caller retains that original meter.
///
/// Native preparation/identity/declaration/reply require no Clone/Debug/serde/Send
/// bounds (only identity PartialEq). No scheduling, threads, automatic grants,
/// receipt authentication, deduplication, durable replay or product migration.
/// Ingress/capture/aggregate native memory limits, provenance, actual dispatch and
/// cancellation delivery remain host-owned. Native callbacks and Drop are trusted;
/// side effects cannot be rolled back/preempted, and a second cleanup panic may abort.
///
/// ```compile_fail
/// use leselang_hir::prepared_parallel::PreparedParallelSession;
/// fn copy(session: PreparedParallelSession<'_, (), (), u8, (), ()>) {
///     let copied = session.clone();
/// }
/// ```
///
/// ```compile_fail
/// use leselang_hir::prepared_parallel::PreparedParallelSession;
/// fn wire(session: PreparedParallelSession<'_, (), (), u8, (), ()>) {
///     let encoded = serde_json::to_string(&session);
/// }
/// ```
#[must_use = "retain the complete reply barrier until completion or cancellation"]
pub struct PreparedParallelSession<
    'expression,
    Node,
    IrResult,
    Identity,
    Declaration: ?Sized,
    Reply,
> {
    core: ParallelReplyCore<'expression, Node, IrResult, Identity, Declaration, Reply>,
}
impl<'expression, Field, Operation, HostEffect, IrResult, Identity, Declaration: ?Sized, Reply>
    PreparedParallelSession<
        'expression,
        Computation<Field, Operation, HostEffect, IrResult>,
        IrResult,
        Identity,
        Declaration,
        Reply,
    >
{
    /// Restore an already-published flat All barrier without preparation or fuel.
    /// Whole shape, count and original row order precede native identity equality;
    /// distinct identities precede mandatory complete-batch admission. No partial
    /// barrier or request escapes refusal/unwind. Accepted rows stay accepted;
    /// awaiting rows use the same reply core, ordered handoff and cancellation.
    ///
    /// The host must validate the same committed graph, exact original identities
    /// and domains, authentic accepted payloads, current authority/deadline, saved
    /// budget and nonterminal dispatch/settlement state. An AcceptedReply is not
    /// fresh authority. This entry has no meter, codec, journal or retry path and
    /// cannot distinguish a fabricated well-formed snapshot. It cannot safely
    /// rearm cancelled/settled work; preventing that remains host-owned.
    /// Native Eq/admission/Drop are bounded trusted work; cleanup may unwind and
    /// a second panic may abort. Use the owning scope on later accept/poll calls.
    pub fn restore<Error>(
        group: &'expression Computation<Field, Operation, HostEffect, IrResult>,
        rows: Vec<SavedParallelMember<'expression, Identity, Declaration, Reply>>,
        limits: ParallelEvaluationLimits,
        admit: impl FnOnce(
            &[SavedParallelMember<'expression, Identity, Declaration, Reply>],
        ) -> Result<(), Error>,
    ) -> Result<Self, ParallelRestoreError<Error>>
    where
        Identity: PartialEq,
    {
        Self::restore_with_scope(group, rows, limits, admit, None)
    }

    /// Restore under an existing local lifetime without taking its ownership.
    /// Closed scopes refuse before shape/equality/admission. Failure or unwind
    /// revokes before retained saved rows and unused-hook cleanup. Revocation
    /// inside native equality or admission stops later hooks and wins over its
    /// ordinary result. Success leaves the scope open; use it on later poll and
    /// acceptance too. No dispatch, fuel, durable settlement or external work
    /// cancellation is added. Callback-internal cleanup remains trusted native
    /// work, and a second panic during cleanup may abort.
    pub fn restore_in_scope<Error>(
        group: &'expression Computation<Field, Operation, HostEffect, IrResult>,
        rows: Vec<SavedParallelMember<'expression, Identity, Declaration, Reply>>,
        limits: ParallelEvaluationLimits,
        admit: impl FnOnce(
            &[SavedParallelMember<'expression, Identity, Declaration, Reply>],
        ) -> Result<(), Error>,
        scope: &LocalExecutionScope,
    ) -> Result<Self, ParallelRestoreError<Error>>
    where
        Identity: PartialEq,
    {
        Self::restore_with_scope(group, rows, limits, admit, Some(scope))
    }

    fn restore_with_scope<Error>(
        group: &'expression Computation<Field, Operation, HostEffect, IrResult>,
        rows: Vec<SavedParallelMember<'expression, Identity, Declaration, Reply>>,
        limits: ParallelEvaluationLimits,
        admit: impl FnOnce(
            &[SavedParallelMember<'expression, Identity, Declaration, Reply>],
        ) -> Result<(), Error>,
        scope: Option<&LocalExecutionScope>,
    ) -> Result<Self, ParallelRestoreError<Error>>
    where
        Identity: PartialEq,
    {
        let fence = CallFence::new(scope);
        let shape = if scope.is_some_and(|scope| !scope.is_open()) {
            Err(ParallelRestoreError::ScopeClosed)
        } else {
            admit_parallel_shape(group, limits).map_err(ParallelRestoreError::Parallel)
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
        let output =
            ParallelReplyCore::restore(branches, rows, admit, scope).map(|core| Self { core });
        fence.finish();
        output
    }

    /// Consume all original prepared operands before returning any native request.
    /// Admission borrows every preparation; correlation consumes each once in order.
    #[expect(
        clippy::type_complexity,
        reason = "original prepared IR and native reply domains retain their distinct lifetimes"
    )]
    pub fn start<Prepared, Request, Error>(
        group: &'expression Computation<Field, Operation, HostEffect, IrResult>,
        prepared: PreparedNativeGroup<
            'expression,
            ComputedBranch<Computation<Field, Operation, HostEffect, IrResult>, IrResult>,
            Prepared,
        >,
        fuel: &mut Fuel,
        limits: ParallelEvaluationLimits,
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
            PreparedParallelMember<'expression, Identity, Declaration, Request>,
            Error,
        >,
    ) -> PreparedParallelStart<
        'expression,
        Computation<Field, Operation, HostEffect, IrResult>,
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
        let branches =
            admit_parallel_shape(group, limits).map_err(PreparedParallelError::Parallel)?;
        if prepared.metadata().kind() != GroupKind::Parallel
            || !std::ptr::eq(prepared.metadata().members(), branches)
            || prepared.members().len() != branches.len()
        {
            return Err(PreparedParallelError::OriginalGroup);
        }
        for (index, row) in prepared.members().iter().enumerate() {
            if row.index() != index || !std::ptr::eq(row.original(), &branches[index]) {
                return Err(PreparedParallelError::OriginalGroup);
            }
        }
        for (index, row) in prepared.members().iter().enumerate() {
            admit(index, row.original(), row.prepared()).map_err(|error| {
                PreparedParallelError::Parallel(ParallelEvaluationError::Native {
                    index,
                    phase: ParallelPhase::Preflight,
                    error,
                })
            })?;
        }
        fuel.charge(1)
            .map_err(|_| PreparedParallelError::Parallel(ParallelEvaluationError::Fuel))?;
        let (_, rows) = prepared.into_parts();
        let mut rows = rows.into_iter();
        let mut core = ParallelReplyCore::new(branches);
        let outcome = core
            .publish_with(
                fuel,
                |index, branch, fuel| {
                    let row = rows.next().ok_or(ParallelEvaluationError::InternalState)?;
                    let (_, _, value) = row.into_parts();
                    correlate(index, branch, value, fuel).map_err(|error| {
                        ParallelEvaluationError::Native {
                            index,
                            phase: ParallelPhase::Prepare,
                            error,
                        }
                    })
                },
                None,
            )
            .map_err(PreparedParallelError::Parallel)?;
        let ParallelPoll::Requests(requests) = outcome else {
            return Err(PreparedParallelError::Parallel(
                ParallelEvaluationError::InternalState,
            ));
        };
        Ok((Self { core }, requests))
    }
}
impl<'expression, Node, IrResult, Identity, Declaration: ?Sized, Reply>
    PreparedParallelSession<'expression, Node, IrResult, Identity, Declaration, Reply>
{
    pub fn status(&self) -> ParallelStatus {
        self.core.status()
    }
    /// Consume an awaiting or ready-results barrier into original ordered rows.
    /// Waiting identities/domains and accepted replies move without native
    /// callbacks, equality, cloning, projection, dispatch or fuel changes. The
    /// bounded row vector allocates, but no native owner is discarded on success.
    /// Terminal barriers return unchanged, including after once-only handoff.
    ///
    /// Rows are unauthenticated host data, not a durable snapshot or permission
    /// to replay. Restore requires fresh whole-batch admission, original policy
    /// and authentic nonterminal settlement. Published requests, the enclosing
    /// meter and execution scope remain host-owned; dropping rows does not cancel
    /// external work. Hosts must reject cancelled/revoked owners before restoring.
    pub fn try_into_saved(
        self,
    ) -> Result<Vec<SavedParallelMember<'expression, Identity, Declaration, Reply>>, Self> {
        self.core.try_into_saved().map_err(|core| Self { core })
    }
    /// Waiting and terminal polls do no native work; complete results move once.
    pub fn poll(
        &mut self,
    ) -> ParallelEvaluationPoll<
        'expression,
        Node,
        IrResult,
        Identity,
        Declaration,
        Reply,
        Infallible,
        Infallible,
    > {
        self.core
            .poll_results(None)?
            .ok_or(ParallelEvaluationError::InternalState)
    }
    /// Cancel before handing off retained results when the explicit scope is
    /// closed. Requests returned by start and parent cancellation stay host-owned.
    pub fn poll_in_scope(
        &mut self,
        scope: &LocalExecutionScope,
    ) -> ParallelEvaluationPoll<
        'expression,
        Node,
        IrResult,
        Identity,
        Declaration,
        Reply,
        Infallible,
        Infallible,
    > {
        self.core
            .poll_results(Some(scope))?
            .ok_or(ParallelEvaluationError::InternalState)
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
    /// Reuse the shared scoped barrier; cancellation discards unpublished results.
    /// The host still owns dispatched requests and enclosing execution policy.
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
impl<Node, IrResult, Identity, Declaration: ?Sized, Reply> fmt::Debug
    for PreparedParallelSession<'_, Node, IrResult, Identity, Declaration, Reply>
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PreparedParallelSession")
            .field("status", &self.status())
            .field("members", &self.core.member_count())
            .finish()
    }
}
