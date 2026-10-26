//! Reference cleanup of already-selected durable records, not recovery admission.

use std::collections::BTreeSet;

use crate::reference_merge::validate_merge_plan;
use crate::reference_recovery::decode_journal_value;
use crate::{ContinuationToken, MAX_CONTINUATION_BYTES, MergePlan, Step};
use leselang_runtime_core::Fault;

/// Raw aggregate and complete-token observations from the caller's snapshot.
pub trait CompactionRecordSource {
    fn effect_bytes(&mut self, token: &str) -> Result<i64, Fault>;
    fn completed_tokens(&mut self) -> Result<BTreeSet<String>, Fault>;
}

pub fn completed_effect_bytes<Host: CompactionRecordSource + ?Sized>(
    host: &mut Host,
    token: &str,
) -> Result<usize, Fault> {
    usize::try_from(host.effect_bytes(token)?)
        .map_err(|_| fault("LSV4007", "journal compaction size is invalid"))
}

/// Keep local order and duplicates. No payload decoding, token admission, cache
/// mutation or retry is introduced; an absent persisted token is stale locally.
pub fn stale_local_completed<Host: CompactionRecordSource + ?Sized>(
    host: &mut Host,
    local: &[(ContinuationToken, Step)],
) -> Result<Vec<ContinuationToken>, Fault> {
    let persisted = host.completed_tokens()?;
    Ok(local
        .iter()
        .filter(|(token, _)| !persisted.contains(token.as_str()))
        .map(|(token, _)| token.clone())
        .collect())
}

pub struct StoredCompactionGroup {
    pub bytes: i64,
    pub plan: Vec<u8>,
}
pub struct StoredCompactionBranch {
    pub token: String,
    pub bytes: i64,
    pub state: String,
}

/// Borrow the original compaction transaction. Branch rows must be collected
/// eagerly in native position order before returning. No commit/rollback hook
/// exists: a late refusal can follow deletes and the outer owner must roll back.
pub trait GroupCompactionOwner {
    fn group(&mut self, token: &str) -> Result<StoredCompactionGroup, Fault>;
    fn branches(&mut self, token: &str) -> Result<Vec<StoredCompactionBranch>, Fault>;
    fn delete_group(&mut self, token: &str) -> Result<usize, Fault>;
    fn delete_branch(&mut self, token: &str) -> Result<usize, Fault>;
}

/// Read all rows before plan admission, then check completeness before byte
/// conversions. Delete the group first, followed by branches in original order.
/// This keeps the released cleanup checks, not a complete graph recovery proof.
pub fn delete_completed_group<Host: GroupCompactionOwner + ?Sized>(
    host: &mut Host,
    token: &str,
) -> Result<usize, Fault> {
    let group = host.group(token)?;
    let branches = host.branches(token)?;
    let plan: MergePlan = decode_journal_value(&group.plan, MAX_CONTINUATION_BYTES)?;
    validate_merge_plan(&plan)?;
    if branches.len() != plan.branches.len() || branches.iter().any(|b| b.state != "completed") {
        return Err(fault(
            "LSV4030",
            "completed merge group has an incomplete branch set",
        ));
    }
    let mut reclaimed = usize::try_from(group.bytes)
        .map_err(|_| fault("LSV4007", "merge group size is invalid"))?;
    for branch in &branches {
        reclaimed = reclaimed.saturating_add(
            usize::try_from(branch.bytes)
                .map_err(|_| fault("LSV4007", "merge branch size is invalid"))?,
        );
    }
    if host.delete_group(token)? != 1 {
        return Err(fault(
            "LSV4030",
            "completed merge group changed during compaction",
        ));
    }
    for branch in branches {
        if host.delete_branch(&branch.token)? != 1 {
            return Err(fault("LSV4030", "merge branch changed during compaction"));
        }
    }
    Ok(reclaimed)
}

fn fault(code: &str, message: &str) -> Fault {
    Fault {
        code: code.into(),
        message: message.into(),
    }
}
