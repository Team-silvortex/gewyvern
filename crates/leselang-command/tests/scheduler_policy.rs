use leselang_command::retention_policy::MAX_JOURNAL_RECORDS;
use leselang_command::scheduler_policy::{
    InvalidSchedulerLimits, SchedulerLimits, SchedulerPressure, check_scheduler_admission,
    scheduler_lease_capacity_exhausted, validate_scheduler_limits,
};
use leselang_runtime_core::DispatchAdmissionError;

#[test]
fn defaults_and_policy_bounds_share_the_product_record_ceiling() {
    assert_eq!(MAX_JOURNAL_RECORDS, 10_000);
    assert_eq!(
        SchedulerLimits::default(),
        SchedulerLimits {
            max_pending_dispatches: MAX_JOURNAL_RECORDS,
            max_active_leases: MAX_JOURNAL_RECORDS,
        }
    );
    for pending in [0, 1, 9_999, 10_000, 10_001, usize::MAX] {
        for active in [0, 1, 9_999, 10_000, 10_001, usize::MAX] {
            let limits = SchedulerLimits {
                max_pending_dispatches: pending,
                max_active_leases: active,
            };
            assert_eq!(
                validate_scheduler_limits(limits).is_ok(),
                (1..=10_000).contains(&pending) && (1..=10_000).contains(&active)
            );
        }
    }
    assert_eq!(
        InvalidSchedulerLimits.to_string(),
        "scheduler limits must be between 1 and the journal record limit"
    );
}

#[test]
fn batch_size_precedes_pressure_and_full_width_addition_cannot_wrap() {
    let limits = SchedulerLimits {
        max_pending_dispatches: 2,
        max_active_leases: 1,
    };
    assert_eq!(
        check_scheduler_admission(limits, usize::MAX, 3),
        Err(DispatchAdmissionError::BatchTooLarge)
    );
    assert_eq!(
        check_scheduler_admission(limits, 2, 1),
        Err(DispatchAdmissionError::Backpressured)
    );
    assert_eq!(check_scheduler_admission(limits, 1, 1), Ok(()));
    assert_eq!(check_scheduler_admission(limits, 2, 0), Ok(()));
    let full = SchedulerLimits {
        max_pending_dispatches: usize::MAX,
        max_active_leases: 1,
    };
    assert_eq!(check_scheduler_admission(full, usize::MAX, 0), Ok(()));
    assert_eq!(
        check_scheduler_admission(full, usize::MAX, 1),
        Err(DispatchAdmissionError::Backpressured)
    );
    assert_eq!(validate_scheduler_limits(full), Err(InvalidSchedulerLimits));
}

#[test]
fn pressure_observation_does_not_grant_a_lease_or_revoke_existing_ones() {
    for pending in [0, 1, 10_000, usize::MAX] {
        for active in [0, 1, 2, 10_000, usize::MAX] {
            for limit in [0, 1, 2, 10_000, usize::MAX] {
                let pressure = SchedulerPressure {
                    pending_dispatches: pending,
                    active_leases: active,
                    limits: SchedulerLimits {
                        max_pending_dispatches: 10_000,
                        max_active_leases: limit,
                    },
                };
                let before = serde_json::to_vec(&pressure).unwrap();
                assert_eq!(
                    scheduler_lease_capacity_exhausted(pressure),
                    active >= limit
                );
                assert_eq!(serde_json::to_vec(&pressure).unwrap(), before);
            }
        }
    }
}

#[test]
fn closed_wire_is_not_semantic_policy_admission_or_current_storage_evidence() {
    let limits: SchedulerLimits =
        serde_json::from_str(r#"{"max_pending_dispatches":0,"max_active_leases":0}"#).unwrap();
    assert_eq!(
        validate_scheduler_limits(limits),
        Err(InvalidSchedulerLimits)
    );
    assert_eq!(check_scheduler_admission(limits, 0, 0), Ok(()));
    assert!(
        serde_json::from_str::<SchedulerLimits>(
            r#"{"max_pending_dispatches":1,"max_active_leases":1,"extra":true}"#
        )
        .is_err()
    );
    let pressure = SchedulerPressure {
        pending_dispatches: 0,
        active_leases: 0,
        limits: SchedulerLimits::default(),
    };
    let mut wire = serde_json::to_value(pressure).unwrap();
    wire["limits"]["extra"] = serde_json::json!(true);
    assert!(serde_json::from_value::<SchedulerPressure>(wire).is_err());
    assert_eq!(
        serde_json::from_slice::<SchedulerPressure>(&serde_json::to_vec(&pressure).unwrap())
            .unwrap(),
        pressure
    );
}
