use leselang_runtime_core::{
    ClockError, MAX_CLOCK_MS, RetryPlanError, RetryScheduleError, RetrySchedulePoint, RetryTiming,
    plan_retry_schedule, validate_retry_schedule,
};

fn timing() -> RetryTiming {
    RetryTiming {
        max_retries: 16,
        base_delay_ms: 7,
        max_delay_ms: 25,
    }
}

#[test]
fn first_retry_uses_base_then_exact_cap_without_advancing_any_owner() {
    for (current, delay) in [(0, 7), (1, 14), (2, 25), (15, 25)] {
        let expected = RetrySchedulePoint {
            retry_count: current + 1,
            ready_at_ms: 100 + delay,
        };
        assert_eq!(
            plan_retry_schedule(current, 100, timing()),
            Ok(Some(expected))
        );
        assert_eq!(
            plan_retry_schedule(current, 100, timing()),
            Ok(Some(expected))
        );
        validate_retry_schedule(current, expected, 100, 16).unwrap();
    }
}

#[test]
fn bounded_plan_matrix_matches_wide_integer_arithmetic() {
    let mut cases = 0;
    for current in 0..80 {
        for base in 1..=7 {
            for cap in base..=40 {
                let policy = RetryTiming {
                    max_retries: 80,
                    base_delay_ms: base,
                    max_delay_ms: cap,
                };
                let point = plan_retry_schedule(current, 13, policy).unwrap().unwrap();
                let delay = ((base as u128) * (1u128 << current)).min(cap as u128) as u64;
                assert_eq!(
                    point,
                    RetrySchedulePoint {
                        retry_count: current + 1,
                        ready_at_ms: 13 + delay
                    }
                );
                validate_retry_schedule(current, point, 13, 80).unwrap();
                cases += 1;
            }
        }
    }
    assert_eq!(cases, 20720);
}

#[test]
fn exhausted_and_full_width_counters_do_not_wrap_reset_or_compute_a_wakeup() {
    for (current, ceiling) in [(0, 0), (16, 16), (17, 16), (u32::MAX, u32::MAX)] {
        let policy = RetryTiming {
            max_retries: ceiling,
            base_delay_ms: u64::MAX,
            max_delay_ms: u64::MAX,
        };
        assert_eq!(plan_retry_schedule(current, MAX_CLOCK_MS, policy), Ok(None));
    }
    let policy = RetryTiming {
        max_retries: u32::MAX,
        base_delay_ms: 1,
        max_delay_ms: 1,
    };
    assert_eq!(
        plan_retry_schedule(u32::MAX - 1, 0, policy),
        Ok(Some(RetrySchedulePoint {
            retry_count: u32::MAX,
            ready_at_ms: 1
        }))
    );
}

#[test]
fn clock_then_timing_validation_precedes_budget_exhaustion() {
    let invalid = RetryTiming {
        max_retries: 0,
        base_delay_ms: 0,
        max_delay_ms: 0,
    };
    assert_eq!(
        plan_retry_schedule(0, u64::MAX, invalid),
        Err(RetryPlanError::Clock(ClockError::OutOfRange))
    );
    assert_eq!(
        plan_retry_schedule(0, 0, invalid),
        Err(RetryPlanError::Timing)
    );
    assert_eq!(
        plan_retry_schedule(
            0,
            0,
            RetryTiming {
                base_delay_ms: 2,
                max_delay_ms: 1,
                ..invalid
            }
        ),
        Err(RetryPlanError::Timing)
    );
}

#[test]
fn portable_clock_overflow_is_not_clamped_to_a_deadline_or_infinite_time() {
    let policy = RetryTiming {
        max_retries: 1,
        base_delay_ms: 1,
        max_delay_ms: 1,
    };
    assert_eq!(
        plan_retry_schedule(0, MAX_CLOCK_MS - 1, policy),
        Ok(Some(RetrySchedulePoint {
            retry_count: 1,
            ready_at_ms: MAX_CLOCK_MS
        }))
    );
    assert_eq!(
        plan_retry_schedule(0, MAX_CLOCK_MS, policy),
        Err(RetryPlanError::Clock(ClockError::Overflow))
    );
    assert_eq!(
        plan_retry_schedule(
            0,
            0,
            RetryTiming {
                base_delay_ms: u64::MAX,
                max_delay_ms: u64::MAX,
                ..policy
            }
        ),
        Err(RetryPlanError::Clock(ClockError::Overflow))
    );
}

#[test]
fn commit_check_requires_exact_next_count_before_a_future_portable_time() {
    for current in 0..=5 {
        for ceiling in 0..=5 {
            for next in 0..=6 {
                for at in [0, 100, 101, MAX_CLOCK_MS, MAX_CLOCK_MS + 1, u64::MAX] {
                    let expected = if next != current + 1 || next > ceiling {
                        Err(RetryScheduleError::Counter)
                    } else if at <= 100 || at > MAX_CLOCK_MS {
                        Err(RetryScheduleError::ReadyTime)
                    } else {
                        Ok(())
                    };
                    assert_eq!(
                        validate_retry_schedule(
                            current,
                            RetrySchedulePoint {
                                retry_count: next,
                                ready_at_ms: at
                            },
                            100,
                            ceiling
                        ),
                        expected
                    );
                }
            }
        }
    }
}

#[test]
fn forged_counter_overflow_and_clock_domains_never_pass_the_commit_check() {
    for next in [0, 1, u32::MAX] {
        assert_eq!(
            validate_retry_schedule(
                u32::MAX,
                RetrySchedulePoint {
                    retry_count: next,
                    ready_at_ms: 1
                },
                0,
                u32::MAX
            ),
            Err(RetryScheduleError::Counter)
        );
    }
    assert_eq!(
        validate_retry_schedule(
            0,
            RetrySchedulePoint {
                retry_count: 1,
                ready_at_ms: u64::MAX
            },
            u64::MAX - 1,
            1
        ),
        Err(RetryScheduleError::ReadyTime)
    );
}

#[test]
fn commit_observation_does_not_authenticate_backoff_or_remember_settlement() {
    let point = plan_retry_schedule(0, 100, timing()).unwrap().unwrap();
    let changed = RetrySchedulePoint {
        ready_at_ms: point.ready_at_ms + 1,
        ..point
    };
    assert_eq!(validate_retry_schedule(0, changed, 100, 16), Ok(()));
    assert_eq!(
        validate_retry_schedule(1, point, 100, 16),
        Err(RetryScheduleError::Counter)
    );
    assert_eq!(validate_retry_schedule(0, point, 100, 16), Ok(()));
    // Exact policy/lease provenance and a fresh count are mandatory host checks.
}
