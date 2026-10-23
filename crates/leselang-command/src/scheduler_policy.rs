//! Product scheduler policy and observations, not a queue, reservation or lease.
//!
//! Decoding these DTOs does not validate policy, authenticate an observation or
//! grant dispatch authority. The owner must count eligible work and check/commit
//! under the same journal boundary. Limits must be consistent across workers.

use leselang_runtime_core::{
    DispatchAdmissionError, check_dispatch_admission, dispatch_lease_capacity_exhausted,
};
use serde::{Deserialize, Serialize};

use crate::retention_policy::MAX_JOURNAL_RECORDS;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SchedulerLimits {
    pub max_pending_dispatches: usize,
    pub max_active_leases: usize,
}

impl Default for SchedulerLimits {
    fn default() -> Self {
        Self {
            max_pending_dispatches: MAX_JOURNAL_RECORDS,
            max_active_leases: MAX_JOURNAL_RECORDS,
        }
    }
}

/// Caller-supplied snapshot data, not proof of current journal state.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SchedulerPressure {
    pub pending_dispatches: usize,
    pub active_leases: usize,
    pub limits: SchedulerLimits,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InvalidSchedulerLimits;

impl std::fmt::Display for InvalidSchedulerLimits {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("scheduler limits must be between 1 and the journal record limit")
    }
}

impl std::error::Error for InvalidSchedulerLimits {}

pub fn validate_scheduler_limits(limits: SchedulerLimits) -> Result<(), InvalidSchedulerLimits> {
    if limits.max_pending_dispatches == 0
        || limits.max_pending_dispatches > MAX_JOURNAL_RECORDS
        || limits.max_active_leases == 0
        || limits.max_active_leases > MAX_JOURNAL_RECORDS
    {
        return Err(InvalidSchedulerLimits);
    }
    Ok(())
}

/// Uses the shared overflow-safe arithmetic; an oversized batch wins over
/// transient pressure. Validate policy separately before using this observation.
pub fn check_scheduler_admission(
    limits: SchedulerLimits,
    pending: usize,
    additional: usize,
) -> Result<(), DispatchAdmissionError> {
    check_dispatch_admission(pending, additional, limits.max_pending_dispatches)
}

/// Counts occupied slots only. With no runnable candidate the owner must return
/// idle, even when this is true. Lowered limits do not revoke existing leases.
pub fn scheduler_lease_capacity_exhausted(pressure: SchedulerPressure) -> bool {
    dispatch_lease_capacity_exhausted(pressure.active_leases, pressure.limits.max_active_leases)
}
