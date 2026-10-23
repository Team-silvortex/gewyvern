//! Product batch recovery policy over an owner-held, already bounded snapshot.
//!
//! Rows stay in the owner's original order. Inspection neither authenticates
//! storage nor grants allocation, restoration or checkpoint publication authority.

use leselang_runtime_core::{
    Fault, GroupRecoveryBatchError, GroupRecoveryBatchHost, GroupRecoveryBatchLimits,
    inspect_group_recovery_batch,
};

use crate::continuation_identity::{
    MAX_EFFECT_SEQUENCE, canonical_continuation_sequence, canonical_merge_sequence,
    valid_continuation_token,
};
use crate::reference_merge::{invalid_group_binding, validate_merge_plan};
use crate::reference_recovery::decode_journal_value;
use crate::retention_policy::MAX_JOURNAL_RECORDS;
use crate::{
    ContinuationToken, ExecutionOrder, MAX_CONTINUATION_BYTES, MAX_MERGE_BRANCHES, MergePlan,
};

/// Token, plan bytes, state, optional terminal bytes and stored order label.
/// Untrusted observations, not a portable storage ABI or admitted checkpoint.
pub type StoredMergeGroup<Bytes = Vec<u8>> = (String, Bytes, String, Option<Bytes>, String);

/// Plan-admitted data for a same-snapshot group proof. Not a recovery certificate:
/// the original state, terminal payload and complete graph still require checking.
/// Owned bytes move into the proof without Clone, Send or serialization bounds.
pub struct AdmittedMergeGroup<Bytes = Vec<u8>> {
    pub token: ContinuationToken,
    pub plan: MergePlan,
    pub state: String,
    pub terminal: Option<Bytes>,
    order: String,
}

/// Borrowed original storage owner. Neither hook may repair, dispatch or publish.
/// Native failures retain their diagnostics; unwind is not retried or swallowed.
pub trait ReferenceGroupBatchProofs<Bytes> {
    fn token_conflicts(&mut self, token: &ContinuationToken) -> Result<bool, Fault>;
    fn inspect_group(&mut self, group: AdmittedMergeGroup<Bytes>) -> Result<(), Fault>;
}

/// Group count -> token/collision/plan -> reservations -> order -> full group proof.
/// Aggregate bytes and row loading must already be bounded by the storage owner.
/// The returned maximum is an observation only, never permission to repair a
/// watermark. Every group must succeed before the shared runner returns it.
pub fn inspect_group_batch<
    Bytes: AsRef<[u8]>,
    Proofs: ReferenceGroupBatchProofs<Bytes> + ?Sized,
>(
    groups: Vec<StoredMergeGroup<Bytes>>,
    proofs: &mut Proofs,
) -> Result<u64, Fault> {
    inspect_group_recovery_batch(
        groups,
        GroupRecoveryBatchLimits {
            max_groups: MAX_JOURNAL_RECORDS,
            max_reservations: MAX_JOURNAL_RECORDS * MAX_MERGE_BRANCHES,
            max_sequence: MAX_EFFECT_SEQUENCE,
        },
        &mut BatchPolicy(proofs),
    )
    .map(|summary| summary.highest_sequence)
    .map_err(|error| match error {
        GroupRecoveryBatchError::Admission(error)
        | GroupRecoveryBatchError::Order(error)
        | GroupRecoveryBatchError::Group(error) => error,
        GroupRecoveryBatchError::Reservation(_) => invalid_group_binding(),
        GroupRecoveryBatchError::Limit => Fault {
            code: "LSV4006".into(),
            message: "journal group count exceeds limit".into(),
        },
    })
}

struct BatchPolicy<'a, Proofs: ?Sized>(&'a mut Proofs);

impl<Bytes: AsRef<[u8]>, Proofs: ReferenceGroupBatchProofs<Bytes> + ?Sized>
    GroupRecoveryBatchHost<StoredMergeGroup<Bytes>> for BatchPolicy<'_, Proofs>
{
    type Group = AdmittedMergeGroup<Bytes>;
    type Error = Fault;

    fn admit_group(&mut self, stored: StoredMergeGroup<Bytes>) -> Result<Self::Group, Fault> {
        let (token, plan_bytes, state, terminal, order) = stored;
        let token = ContinuationToken::from_wire(token);
        if !valid_continuation_token(&token) {
            return Err(fault("merge group token is invalid"));
        }
        if self.0.token_conflicts(&token)? {
            return Err(fault("merge group token conflicts with an effect token"));
        }
        let plan: MergePlan = decode_journal_value(plan_bytes.as_ref(), MAX_CONTINUATION_BYTES)?;
        validate_merge_plan(&plan).map_err(|_| fault("merge group plan is invalid"))?;
        Ok(AdmittedMergeGroup {
            token,
            plan,
            state,
            terminal,
            order,
        })
    }
    fn highest_sequence(&mut self, group: &Self::Group) -> u64 {
        merge_reserved_sequence(&group.token, &group.plan)
    }
    fn reserved_sequences<'a>(
        &'a mut self,
        group: &'a Self::Group,
    ) -> impl Iterator<Item = u64> + 'a {
        group
            .plan
            .result_binding
            .iter()
            .flat_map(|binding| binding.reserved_sequences())
    }
    fn validate_order(&mut self, group: &Self::Group) -> Result<(), Fault> {
        if execution_order_label(group.plan.order) != group.order {
            return Err(fault("execution order conflicts with group plan"));
        }
        Ok(())
    }
    fn inspect_group(&mut self, group: Self::Group) -> Result<(), Fault> {
        self.0.inspect_group(group)
    }
}

/// Released journal labels, also used by the writer. Not the wire enum spelling.
pub fn execution_order_label(order: ExecutionOrder) -> &'static str {
    match order {
        ExecutionOrder::Parallel
        | ExecutionOrder::BoundParallel
        | ExecutionOrder::ParallelGroupTail
        | ExecutionOrder::ParallelGroupCapture
        | ExecutionOrder::ParallelGroupDataflow
        | ExecutionOrder::ParallelGroupConditional => "parallel",
        ExecutionOrder::Sequential
        | ExecutionOrder::BoundSequential
        | ExecutionOrder::GroupTail
        | ExecutionOrder::GroupCapture
        | ExecutionOrder::ResultChain
        | ExecutionOrder::Dataflow
        | ExecutionOrder::GroupDataflow
        | ExecutionOrder::GroupConditional => "sequential",
    }
}

/// Original numeric observation only. Opaque legacy owners remain valid and
/// contribute zero without reservations; this does not allocate or repair IDs.
pub fn merge_reserved_sequence(token: &ContinuationToken, plan: &MergePlan) -> u64 {
    plan.result_binding
        .iter()
        .flat_map(|binding| binding.reserved_sequences())
        .fold(
            canonical_merge_sequence(token)
                .or_else(|| canonical_continuation_sequence(token))
                .unwrap_or(0),
            u64::max,
        )
}

fn fault(message: &str) -> Fault {
    Fault {
        code: "LSV4007".into(),
        message: message.into(),
    }
}
