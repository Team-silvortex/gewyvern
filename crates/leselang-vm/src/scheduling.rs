use leselang_command::scheduler_policy::{
    check_scheduler_admission, scheduler_lease_capacity_exhausted,
};
use leselang_runtime_core::DispatchAdmissionError;

use crate::Fault;

pub use leselang_command::{DispatchClaim, SchedulerLimits, SchedulerPressure};

pub(crate) trait ReferenceSchedulerLimits {
    fn validate(self) -> Result<(), Fault>;
    fn check_admission(self, pending: usize, additional: usize) -> Result<(), Fault>;
}

impl ReferenceSchedulerLimits for SchedulerLimits {
    fn validate(self) -> Result<(), Fault> {
        leselang_command::reference_startup::validate_limits(self)
    }

    fn check_admission(self, pending: usize, additional: usize) -> Result<(), Fault> {
        check_scheduler_admission(self, pending, additional).map_err(|error| match error {
            DispatchAdmissionError::BatchTooLarge => Fault {
                code: "LSV2503".into(),
                message: format!(
                    "root needs {additional} dispatches, exceeding the configured limit {}",
                    self.max_pending_dispatches
                ),
            },
            DispatchAdmissionError::Backpressured => Fault {
                code: "LSV2501".into(),
                message: format!(
                    "dispatch admission is backpressured: {pending} pending, {additional} requested, limit {}",
                    self.max_pending_dispatches
                ),
            },
        })
    }
}

pub(crate) trait ReferenceSchedulerPressure {
    fn lease_capacity_exhausted(self) -> bool;
    #[cfg(test)]
    fn lease_fault(self) -> Fault;
}

impl ReferenceSchedulerPressure for SchedulerPressure {
    fn lease_capacity_exhausted(self) -> bool {
        scheduler_lease_capacity_exhausted(self)
    }

    #[cfg(test)]
    fn lease_fault(self) -> Fault {
        leselang_command::reference_claim::lease_capacity_fault(self)
    }
}
