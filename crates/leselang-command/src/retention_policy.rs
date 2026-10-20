//! Product journal retention policy, without storage, eviction or replay authority.

use serde::{Deserialize, Serialize};

pub const DEFAULT_COMPLETED_RETENTION: usize = 5_000;
pub const DEFAULT_COMPACTION_BATCH: usize = 500;
pub const MAX_COMPACTION_BATCH: usize = 1_000;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RetentionPolicy {
    pub max_completed_records: usize,
    pub max_delete_per_run: usize,
}

impl Default for RetentionPolicy {
    fn default() -> Self {
        Self {
            max_completed_records: DEFAULT_COMPLETED_RETENTION,
            max_delete_per_run: DEFAULT_COMPACTION_BATCH,
        }
    }
}

impl RetentionPolicy {
    /// Pure batch sizing, not policy admission or permission to delete records.
    /// The host must validate policy and count only eligible logical units,
    /// protecting live groups and selecting/committing under its storage boundary.
    pub fn removal_count(&self, completed_units: usize) -> usize {
        completed_units
            .saturating_sub(self.max_completed_records)
            .min(self.max_delete_per_run)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CompactionReport {
    pub removed_records: usize,
    pub remaining_completed: usize,
    pub reclaimed_logical_bytes: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InvalidRetentionPolicy;

impl std::fmt::Display for InvalidRetentionPolicy {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("journal retention policy exceeds runtime bounds")
    }
}

impl std::error::Error for InvalidRetentionPolicy {}

/// Validate product policy against the owning journal's record ceiling.
/// Successful admission neither inspects storage nor reserves a deletion batch.
pub fn validate_retention_policy(
    policy: &RetentionPolicy,
    max_journal_records: usize,
) -> Result<(), InvalidRetentionPolicy> {
    if policy.max_completed_records == 0
        || policy.max_completed_records > max_journal_records
        || policy.max_delete_per_run == 0
        || policy.max_delete_per_run > MAX_COMPACTION_BATCH
    {
        return Err(InvalidRetentionPolicy);
    }
    Ok(())
}
