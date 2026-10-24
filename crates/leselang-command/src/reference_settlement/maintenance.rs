//! Product retention/expiry coordination, not a storage or transaction owner.
//!
//! Native hooks retain bounded reads, group protection, audit cascades, byte
//! accounting, deadline settlement and backend-specific failure semantics. No
//! hook is retried or compensated; a native failure can follow native effects.

use crate::continuation_identity::continuation_age_order;
use crate::retention_policy::{MAX_JOURNAL_RECORDS, validate_retention_policy};
use crate::{CompactionReport, ContinuationImage, ContinuationToken, RetentionPolicy, Step};
use leselang_runtime_core::{Fault, validate_clock};

/// Native compaction observation, not a wire receipt or permission to delete.
/// Logical-unit counts are independent of the number of pruned branch tokens.
pub struct JournalCompaction {
    pub removed_records: usize,
    pub pruned_tokens: Vec<ContinuationToken>,
    pub remaining_completed: usize,
    pub reclaimed_logical_bytes: usize,
}

pub trait ReferenceRetentionHost {
    /// Copy the original local cache in its native iteration order, after admission.
    fn completed_snapshot(&self) -> Vec<(ContinuationToken, Step)>;
    fn compact_completed(
        &mut self,
        local_completed: &[(ContinuationToken, Step)],
        policy: &RetentionPolicy,
    ) -> Result<JournalCompaction, Fault>;
    fn remove_completed(&mut self, token: &ContinuationToken);
}

/// Validate before snapshotting; stably sort by released age semantics, commit,
/// then evict exactly the native token list, even if no logical units were removed.
pub fn compact_journal<Host: ReferenceRetentionHost + ?Sized>(
    policy: &RetentionPolicy,
    host: &mut Host,
) -> Result<CompactionReport, Fault> {
    validate_retention_policy(policy, MAX_JOURNAL_RECORDS).map_err(|error| Fault {
        code: "LSV2301".to_string(),
        message: error.to_string(),
    })?;
    let mut local_completed = host.completed_snapshot();
    local_completed.sort_by(|(left, _), (right, _)| continuation_age_order(left, right));
    let compacted = host.compact_completed(&local_completed, policy)?;
    for token in &compacted.pruned_tokens {
        host.remove_completed(token);
    }
    Ok(CompactionReport {
        removed_records: compacted.removed_records,
        remaining_completed: compacted.remaining_completed,
        reclaimed_logical_bytes: compacted.reclaimed_logical_bytes,
    })
}

pub trait ReferenceExpirationHost {
    fn expire_due(&mut self, now_ms: u64) -> Result<Vec<(ContinuationImage, Step)>, Fault>;
    fn remove_pending(&mut self, token: &ContinuationToken);
    /// Move the original token/result into the cache, replacing an existing entry.
    fn insert_expired(&mut self, token: ContinuationToken, step: Step);
}

/// Preserve native batch order and commit-before-cache publication. Do not run
/// visible-group selection, reconstruct cancellation records or deduplicate rows.
/// A hook unwind after a mutation leaves that mutation intact, with no retry.
pub fn expire_due<Host: ReferenceExpirationHost + ?Sized>(
    now_ms: u64,
    host: &mut Host,
) -> Result<(), Fault> {
    validate_clock(now_ms)?;
    let expired = host.expire_due(now_ms)?;
    for (image, step) in expired {
        host.remove_pending(&image.token);
        host.insert_expired(image.token, step);
    }
    Ok(())
}
