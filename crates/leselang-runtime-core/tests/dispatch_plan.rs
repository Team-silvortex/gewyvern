use leselang_runtime_core::{
    ClockError, DispatchAttemptError, DispatchLeaseError, DispatchLeaseObservation,
    DispatchLeasePlanError, DispatchLeaseStamp, MAX_CLOCK_MS, next_dispatch_attempt,
    plan_dispatch_lease, validate_dispatch_lease,
};

#[test]
fn lease_timing_matches_a_wide_oracle_with_explicit_fault_precedence() {
    let clocks = [
        0,
        1,
        MAX_CLOCK_MS - 10,
        MAX_CLOCK_MS - 1,
        MAX_CLOCK_MS,
        MAX_CLOCK_MS + 1,
        u64::MAX,
    ];
    let durations = [0, 1, 2, 10, MAX_CLOCK_MS, MAX_CLOCK_MS + 1, u64::MAX];
    let limits = [0, 1, 2, 10, MAX_CLOCK_MS, u64::MAX];
    let mut cases = 0;
    for now in clocks {
        for duration in durations {
            for limit in limits {
                let expected = if now > MAX_CLOCK_MS {
                    Err(DispatchLeasePlanError::Clock(ClockError::OutOfRange))
                } else if duration == 0 || duration > limit {
                    Err(DispatchLeasePlanError::Duration)
                } else if now as u128 + duration as u128 > MAX_CLOCK_MS as u128 {
                    Err(DispatchLeasePlanError::Clock(ClockError::Overflow))
                } else {
                    Ok(now + duration)
                };
                assert_eq!(plan_dispatch_lease(now, duration, limit), expected);
                cases += 1;
            }
        }
    }
    assert_eq!(cases, 294);
}

#[test]
fn delivery_counter_matches_a_wide_oracle_without_reset_or_saturation() {
    let counts = (0..=128)
        .chain([u32::MAX - 1, u32::MAX])
        .collect::<Vec<_>>();
    let mut cases = 0;
    for &current in &counts {
        for &limit in &counts {
            let next = u64::from(current) + 1;
            let expected = if next > u64::from(u32::MAX) {
                Err(DispatchAttemptError::CounterExhausted)
            } else if next > u64::from(limit) {
                Err(DispatchAttemptError::LimitReached)
            } else {
                Ok(next as u32)
            };
            assert_eq!(next_dispatch_attempt(current, limit), expected);
            cases += 1;
        }
    }
    assert_eq!(cases, 17161);
}

#[test]
fn first_and_last_granted_attempts_are_exact_and_zero_grants_no_delivery() {
    assert_eq!(
        next_dispatch_attempt(0, 0),
        Err(DispatchAttemptError::LimitReached)
    );
    assert_eq!(next_dispatch_attempt(0, 1), Ok(1));
    assert_eq!(next_dispatch_attempt(99, 100), Ok(100));
    assert_eq!(
        next_dispatch_attempt(100, 100),
        Err(DispatchAttemptError::LimitReached)
    );
    assert_eq!(next_dispatch_attempt(u32::MAX - 1, u32::MAX), Ok(u32::MAX));
    assert_eq!(
        next_dispatch_attempt(u32::MAX, 0),
        Err(DispatchAttemptError::CounterExhausted)
    );
}

fn observation(stamp: DispatchLeaseStamp) -> DispatchLeaseObservation {
    DispatchLeaseObservation {
        same_request: true,
        active: true,
        attempt: stamp.attempt,
        retry_count: stamp.retry_count,
        ready_at_ms: 0,
        expires_at_ms: Some(stamp.expires_at_ms),
    }
}

#[test]
fn last_portable_clock_allows_current_completion_but_not_a_new_lease() {
    let stamp = DispatchLeaseStamp {
        attempt: next_dispatch_attempt(0, 1).unwrap(),
        retry_count: 0,
        expires_at_ms: plan_dispatch_lease(MAX_CLOCK_MS - 1, 1, 1).unwrap(),
    };
    assert_eq!(
        validate_dispatch_lease(stamp, observation(stamp), MAX_CLOCK_MS),
        Ok(())
    );
    assert_eq!(
        plan_dispatch_lease(MAX_CLOCK_MS, 1, 1),
        Err(DispatchLeasePlanError::Clock(ClockError::Overflow))
    );
    assert_eq!(
        next_dispatch_attempt(stamp.attempt, 1),
        Err(DispatchAttemptError::LimitReached)
    );
    // Planning failures neither invalidate nor settle the existing lease.
    assert_eq!(
        validate_dispatch_lease(stamp, observation(stamp), MAX_CLOCK_MS),
        Ok(())
    );
}

#[test]
fn numeric_reissuance_preserves_semantic_retry_metadata_and_old_attempt_fencing() {
    let first = DispatchLeaseStamp {
        attempt: next_dispatch_attempt(4, 10).unwrap(),
        retry_count: 3,
        expires_at_ms: plan_dispatch_lease(100, 10, 20).unwrap(),
    };
    assert_eq!(
        validate_dispatch_lease(first, observation(first), 110),
        Ok(())
    );
    let second = DispatchLeaseStamp {
        attempt: next_dispatch_attempt(first.attempt, 10).unwrap(),
        retry_count: first.retry_count,
        expires_at_ms: plan_dispatch_lease(110, 10, 20).unwrap(),
    };
    assert_eq!(second.attempt, 6);
    assert_eq!(second.retry_count, 3);
    assert_eq!(
        validate_dispatch_lease(first, observation(second), 110),
        Err(DispatchLeaseError::Superseded)
    );
    assert_eq!(
        validate_dispatch_lease(second, observation(second), 120),
        Ok(())
    );
}

#[test]
fn repeated_numeric_plans_are_not_reservations_or_execution_authority() {
    for _ in 0..10 {
        assert_eq!(plan_dispatch_lease(10, 5, 5), Ok(15));
        assert_eq!(next_dispatch_attempt(0, 1), Ok(1));
    }
    // A host that already committed attempt 1 must re-read, not reuse old input.
    assert_eq!(
        next_dispatch_attempt(1, 1),
        Err(DispatchAttemptError::LimitReached)
    );
    assert_eq!(plan_dispatch_lease(15, 5, 5), Ok(20));
}
