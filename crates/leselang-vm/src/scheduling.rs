use serde::{Deserialize, Serialize};

use crate::{DispatchLease, Fault, MAX_JOURNAL_RECORDS};

/// Trusted host policy, applied consistently by every worker of one journal namespace.
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

impl SchedulerLimits {
    pub(crate) fn validate(self) -> Result<(), Fault> {
        if self.max_pending_dispatches == 0
            || self.max_pending_dispatches > MAX_JOURNAL_RECORDS
            || self.max_active_leases == 0
            || self.max_active_leases > MAX_JOURNAL_RECORDS
        {
            return Err(Fault {
                code: "LSV2500".into(),
                message: "scheduler limits must be between 1 and the journal record limit".into(),
            });
        }
        Ok(())
    }

    pub(crate) fn check_admission(self, pending: usize, additional: usize) -> Result<(), Fault> {
        if additional > self.max_pending_dispatches {
            return Err(Fault {
                code: "LSV2503".into(),
                message: format!(
                    "root needs {additional} dispatches, exceeding the configured limit {}",
                    self.max_pending_dispatches
                ),
            });
        }
        if pending.saturating_add(additional) > self.max_pending_dispatches {
            return Err(Fault {
                code: "LSV2501".into(),
                message: format!(
                    "dispatch admission is backpressured: {pending} pending, {additional} requested, limit {}",
                    self.max_pending_dispatches
                ),
            });
        }
        Ok(())
    }
}

/// A read-only authoritative outbox snapshot, not a VM-local continuation count.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SchedulerPressure {
    pub pending_dispatches: usize,
    pub active_leases: usize,
    pub limits: SchedulerLimits,
}

impl SchedulerPressure {
    pub(crate) fn lease_capacity_exhausted(self) -> bool {
        self.active_leases >= self.limits.max_active_leases
    }

    pub(crate) fn lease_fault(self) -> Fault {
        Fault {
            code: "LSV2502".into(),
            message: format!(
                "dispatch leasing is backpressured: {} active leases, limit {}",
                self.active_leases, self.limits.max_active_leases
            ),
        }
    }
}

/// Backpressure never grants a lease or increments a delivery attempt.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(
    tag = "kind",
    content = "payload",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum DispatchClaim {
    Leased(Box<DispatchLease>),
    Idle,
    Backpressured(SchedulerPressure),
}
