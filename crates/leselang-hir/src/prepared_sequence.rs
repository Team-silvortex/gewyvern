//! Complete scoped correlation with fresh, one-at-a-time sequential publication.

use std::{borrow::Borrow, collections::VecDeque, fmt};

use leselang_runtime_core::{Fuel, HostResultDomain, ReplyAuthority};

use crate::ir::{Computation, ComputedBranch, GroupKind};
use crate::native_group_preparation::PreparedNativeGroup;
use crate::sequence_evaluation::{
    PreparedSequenceMember, SequenceAcceptance, SequenceEvaluationError, SequenceEvaluationLimits,
    SequencePhase, SequencePoll, SequenceReplyCore, SequenceStatus, admit_sequence_shape,
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
/// use leselang_hir::prepared_sequence::PreparedSequenceSession;
/// fn copy(session: PreparedSequenceSession<'_, (), (), u8, (), ()>) {
///     let copied = session.clone();
/// }
/// ```
///
/// ```compile_fail
/// use leselang_hir::prepared_sequence::PreparedSequenceSession;
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
        // Local ownership releases the entire retained suffix on callback unwind.
        let mut prepared = std::mem::take(&mut self.prepared);
        let outcome = self.core.publish_with(|index, original| {
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
        });
        if !matches!(self.status(), SequenceStatus::Terminal(_)) {
            self.prepared = prepared;
        }
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
        let prepared = std::mem::take(&mut self.prepared);
        let outcome = self.core.try_accept(identity, reply, authority);
        if !matches!(self.status(), SequenceStatus::Terminal(_)) {
            self.prepared = prepared;
        }
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
