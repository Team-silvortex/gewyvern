//! Reference merge-graph registration over native journal ownership.

use crate::continuation_identity::canonical_continuation_sequence;
use crate::reference_continuation::validate_continuation_encoding_size;
use crate::reference_group_batch::merge_reserved_sequence;
use crate::retention_policy::MAX_JOURNAL_RECORDS;
use crate::{ContinuationToken, EffectRequest, MergePlan, SchedulerLimits};
use leselang_runtime_core::Fault;

pub trait MemoryMergeGraphJournal {
    fn contains_group(&self, token: &ContinuationToken) -> bool;
    fn contains_dispatch(&self, token: &ContinuationToken) -> bool;
    fn check_capacity(&self, additional: usize, limits: SchedulerLimits) -> Result<(), Fault>;
    fn insert_dispatch(&mut self, request: &EffectRequest);
    fn insert_group(
        &mut self,
        token: &ContinuationToken,
        plan: &MergePlan,
        branches: Vec<ContinuationToken>,
    );
}

/// The native public entry admits the graph first. Keep bounded-image checks
/// before conflict observations, then capacity before any mutation. This is not
/// idempotent replay: even an identical registered graph is a conflict. Infallible
/// memory writes retain their original prefix if a trusted owner unwinds.
pub fn register_memory<Host: MemoryMergeGraphJournal + ?Sized>(
    host: &mut Host,
    token: &ContinuationToken,
    plan: &MergePlan,
    branches: &[(&str, &EffectRequest)],
    limits: SchedulerLimits,
) -> Result<(), Fault> {
    for (_, request) in branches {
        validate_continuation_encoding_size(&request.continuation)?;
    }
    if host.contains_group(token)
        || host.contains_dispatch(token)
        || branches.iter().any(|(_, request)| {
            host.contains_dispatch(&request.continuation.token)
                || host.contains_group(&request.continuation.token)
        })
    {
        return Err(fault(
            "LSV4011",
            "merge graph token conflicts with pending state",
        ));
    }
    host.check_capacity(branches.len(), limits)?;
    let tokens = branches
        .iter()
        .map(|(_, request)| request.continuation.token.clone())
        .collect();
    for (_, request) in branches {
        host.insert_dispatch(request);
    }
    host.insert_group(token, plan, tokens);
    Ok(())
}

/// Native bounded encodings in the exact same order as the admitted requests.
pub type EncodedMergeBranch<'a> = (&'a str, Vec<u8>, Vec<u8>);

pub struct MergeGraphEncoding<'a> {
    pub plan: &'a [u8],
    pub branches: &'a [EncodedMergeBranch<'a>],
    pub additional_bytes: usize,
}

/// One immediate transaction opened after native graph admission and encoding.
/// Native owners retain counts/quota, conflict reads, signed positions/deadlines,
/// SQL diagnostics, watermark reconciliation and atomic commit/rollback.
pub trait DurableMergeGraphTransaction {
    fn record_count(&mut self) -> Result<usize, Fault>;
    fn check_capacity(&mut self, additional: usize, limits: SchedulerLimits) -> Result<(), Fault>;
    fn ensure_growth(&mut self, additional: usize) -> Result<(), Fault>;
    fn group_conflicts(&mut self, token: &ContinuationToken) -> Result<bool, Fault>;
    fn insert_group(
        &mut self,
        token: &ContinuationToken,
        plan: &MergePlan,
        bytes: &[u8],
    ) -> Result<(), Fault>;
    fn insert_branch(
        &mut self,
        group: &ContinuationToken,
        request: &EffectRequest,
        position: usize,
        encoded: &EncodedMergeBranch<'_>,
    ) -> Result<(), Fault>;
    fn advance_watermark(&mut self, sequence: u64) -> Result<(), Fault>;
    fn commit(self) -> Result<(), Fault>;
}

/// Keep record/capacity/quota checks before conflict reads. Branch storage keeps
/// native effect -> dispatch -> link ordering; conflicts are never repaired.
/// Encodings are trusted matching native buffers, not a new recovery authority.
/// Commit consumes the owner and is never retried after an uncertain outcome.
pub fn register_durable<Host: DurableMergeGraphTransaction>(
    mut host: Host,
    token: &ContinuationToken,
    plan: &MergePlan,
    branches: &[(&str, &EffectRequest)],
    limits: SchedulerLimits,
    encoding: MergeGraphEncoding<'_>,
) -> Result<(), Fault> {
    let current_count = host.record_count()?;
    let added_records = branches.len().saturating_add(1);
    if current_count.saturating_add(added_records) > MAX_JOURNAL_RECORDS {
        return Err(fault(
            "LSV4006",
            format!("journal record limit {MAX_JOURNAL_RECORDS} reached"),
        ));
    }
    host.check_capacity(branches.len(), limits)?;
    host.ensure_growth(encoding.additional_bytes)?;
    if host.group_conflicts(token)? {
        return Err(fault(
            "LSV4011",
            "merge graph token conflicts with durable state",
        ));
    }
    host.insert_group(token, plan, encoding.plan)?;
    for (position, ((_, request), encoded)) in branches.iter().zip(encoding.branches).enumerate() {
        host.insert_branch(token, request, position, encoded)?;
    }
    let highest_sequence = branches
        .iter()
        .filter_map(|(_, request)| canonical_continuation_sequence(&request.continuation.token))
        .fold(merge_reserved_sequence(token, plan), u64::max);
    host.advance_watermark(highest_sequence)?;
    host.commit()
}

fn fault(code: &str, message: impl Into<String>) -> Fault {
    Fault {
        code: code.into(),
        message: message.into(),
    }
}
