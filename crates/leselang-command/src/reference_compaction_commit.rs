//! Reference retention batches over native journal ownership, not storage admission.

use std::collections::BTreeSet;

use crate::continuation_identity::continuation_age_order;
use crate::reference_recovery::MAX_JOURNAL_ENTRY_BYTES;
use crate::reference_settlement::maintenance::JournalCompaction;
use crate::{
    ContinuationToken, EffectError, EffectRequest, MAX_CONTINUATION_BYTES, MergePlan,
    RetentionPolicy, Step,
};
use leselang_runtime_core::Fault;
use serde::Serialize;

pub struct MemoryCompactionGroup<'a> {
    pub plan: &'a MergePlan,
    pub branches: &'a [ContinuationToken],
    pub terminal: Option<&'a Step>,
}

pub struct MemoryCompactionDispatch<'a> {
    pub request: &'a EffectRequest,
    pub last_error: Option<&'a EffectError>,
}

/// Views borrow the original maps in native order. Measurement retains native
/// bounds/errors; removal hooks are infallible map operations, not transactions.
pub trait MemoryCompactionJournal {
    fn groups(&self) -> impl Iterator<Item = (&ContinuationToken, MemoryCompactionGroup<'_>)>;
    fn group(&self, token: &ContinuationToken) -> Option<MemoryCompactionGroup<'_>>;
    fn dispatch(&self, token: &ContinuationToken) -> Option<MemoryCompactionDispatch<'_>>;
    fn measure<T: Serialize>(&self, value: &T, limit: usize, label: &str) -> Result<usize, Fault>;
    fn remove_group(&mut self, token: &ContinuationToken);
    fn remove_dispatch(&mut self, token: &ContinuationToken);
    fn retain_live_audits(&mut self);
}

enum MemoryUnit {
    Effect(ContinuationToken),
    Merge {
        group_token: ContinuationToken,
        age_token: ContinuationToken,
    },
}
impl MemoryUnit {
    fn age_token(&self) -> &ContinuationToken {
        match self {
            Self::Effect(token) => token,
            Self::Merge { age_token, .. } => age_token,
        }
    }
}

/// Protect all group members; completed groups count as one unit. Preflight the
/// entire selected batch before removing anything, then cascade audits once.
/// Unwinds from native mutation hooks are not caught, retried or compensated.
pub fn compact_memory<Host: MemoryCompactionJournal + ?Sized>(
    host: &mut Host,
    local_completed: &[(ContinuationToken, Step)],
    policy: &RetentionPolicy,
) -> Result<JournalCompaction, Fault> {
    let protected = host
        .groups()
        .flat_map(|(_, group)| group.branches.iter())
        .collect::<BTreeSet<_>>();
    let mut units = local_completed
        .iter()
        .filter(|(token, _)| !protected.contains(token))
        .map(|(token, _)| MemoryUnit::Effect(token.clone()))
        .collect::<Vec<_>>();
    for (token, group) in host.groups() {
        if group.terminal.is_some() {
            let age_token = group
                .branches
                .first()
                .ok_or_else(|| fault("LSV4007", "completed merge group has no branches"))?;
            units.push(MemoryUnit::Merge {
                group_token: token.clone(),
                age_token: age_token.clone(),
            });
        }
    }
    units.sort_by(|left, right| continuation_age_order(left.age_token(), right.age_token()));
    let completed_count = units.len();
    let remove_count = policy.removal_count(completed_count);
    let mut reclaimed_logical_bytes = 0usize;
    let mut removed_tokens = Vec::with_capacity(remove_count);
    for unit in units.iter().take(remove_count) {
        let tokens = match unit {
            MemoryUnit::Effect(token) => std::slice::from_ref(token),
            MemoryUnit::Merge { group_token, .. } => {
                let group = host
                    .group(group_token)
                    .ok_or_else(|| fault("LSV4033", "selected merge group disappeared"))?;
                let terminal = group
                    .terminal
                    .ok_or_else(|| fault("LSV4007", "selected merge group is not completed"))?;
                reclaimed_logical_bytes = reclaimed_logical_bytes
                    .saturating_add(host.measure(
                        group.plan,
                        MAX_CONTINUATION_BYTES,
                        "merge plan",
                    )?)
                    .saturating_add(host.measure(
                        terminal,
                        MAX_JOURNAL_ENTRY_BYTES,
                        "merge result",
                    )?);
                group.branches
            }
        };
        for token in tokens {
            if let Some((_, step)) = local_completed.iter().find(|(local, _)| local == token) {
                reclaimed_logical_bytes = reclaimed_logical_bytes.saturating_add(host.measure(
                    step,
                    MAX_JOURNAL_ENTRY_BYTES,
                    "completed step",
                )?);
            }
            if let Some(dispatch) = host.dispatch(token) {
                reclaimed_logical_bytes = reclaimed_logical_bytes.saturating_add(host.measure(
                    dispatch.request,
                    MAX_JOURNAL_ENTRY_BYTES,
                    "effect request",
                )?);
                if let Some(error) = dispatch.last_error {
                    reclaimed_logical_bytes = reclaimed_logical_bytes.saturating_add(
                        host.measure(error, MAX_JOURNAL_ENTRY_BYTES, "effect error")?,
                    );
                }
            }
            removed_tokens.push(token.clone());
        }
    }
    for unit in units.into_iter().take(remove_count) {
        if let MemoryUnit::Merge { group_token, .. } = unit {
            host.remove_group(&group_token);
        }
    }
    for token in &removed_tokens {
        host.remove_dispatch(token);
    }
    if !removed_tokens.is_empty() {
        host.retain_live_audits();
    }
    Ok(JournalCompaction {
        removed_records: remove_count,
        pruned_tokens: removed_tokens,
        remaining_completed: completed_count.saturating_sub(remove_count),
        reclaimed_logical_bytes,
    })
}

/// One native immediate transaction. Candidate rows are collected before writes;
/// group validation/deletion and local-cache reconciliation stay on this owner.
/// The original no-op close and mutating commit retain different diagnostics.
pub trait DurableCompactionTransaction {
    fn completed_count(&mut self) -> Result<i64, Fault>;
    fn candidates(&mut self, limit: i64) -> Result<Vec<(i64, String)>, Fault>;
    fn effect_bytes(&mut self, token: &str) -> Result<usize, Fault>;
    fn delete_effect(&mut self, token: &str) -> Result<usize, Fault>;
    fn delete_group(&mut self, token: &str) -> Result<usize, Fault>;
    fn stale_local(
        &mut self,
        local: &[(ContinuationToken, Step)],
    ) -> Result<Vec<ContinuationToken>, Fault>;
    fn commit(self, no_removal: bool) -> Result<(), Fault>;
}

/// Commit before exposing any pruned tokens. Errors/unwinds consume the native
/// owner and rely on rollback; neither the cache nor the watermark is changed here.
pub fn compact_durable<Host: DurableCompactionTransaction>(
    mut host: Host,
    local_completed: &[(ContinuationToken, Step)],
    policy: &RetentionPolicy,
) -> Result<JournalCompaction, Fault> {
    let completed_count = usize::try_from(host.completed_count()?)
        .map_err(|_| fault("LSV4007", "completed journal count is invalid"))?;
    let remove_count = policy.removal_count(completed_count);
    if remove_count == 0 {
        let pruned_tokens = host.stale_local(local_completed)?;
        host.commit(true)?;
        return Ok(JournalCompaction {
            removed_records: 0,
            pruned_tokens,
            remaining_completed: completed_count,
            reclaimed_logical_bytes: 0,
        });
    }
    let limit = i64::try_from(remove_count)
        .map_err(|_| fault("LSV4030", "journal compaction batch is invalid"))?;
    let candidates = host.candidates(limit)?;
    let mut removed_units = 0usize;
    let mut reclaimed_logical_bytes = 0usize;
    for (kind, token) in candidates {
        match kind {
            0 => {
                let logical_bytes = host.effect_bytes(&token)?;
                if host.delete_effect(&token)? != 1 {
                    return Err(fault(
                        "LSV4030",
                        "completed journal record changed during compaction",
                    ));
                }
                reclaimed_logical_bytes = reclaimed_logical_bytes.saturating_add(logical_bytes);
            }
            1 => {
                reclaimed_logical_bytes =
                    reclaimed_logical_bytes.saturating_add(host.delete_group(&token)?)
            }
            _ => {
                return Err(fault(
                    "LSV4007",
                    "journal compaction candidate kind is invalid",
                ));
            }
        }
        removed_units += 1;
    }
    let pruned_tokens = host.stale_local(local_completed)?;
    host.commit(false)?;
    Ok(JournalCompaction {
        remaining_completed: completed_count.saturating_sub(removed_units),
        removed_records: removed_units,
        pruned_tokens,
        reclaimed_logical_bytes,
    })
}

fn fault(code: &str, message: &str) -> Fault {
    Fault {
        code: code.into(),
        message: message.into(),
    }
}
