use leselang_runtime_core::{
    Admission, AdmissionAdapter, AdmissionAttempt, AdmissionPolicy, AdmissionPoll, ClockError,
    Fault, MAX_CLOCK_MS, MAX_EXECUTION_TIMEOUT_MS, checked_clock_add, validate_clock,
    validate_execution_deadline,
};

#[test]
fn exact_addition_and_zero_offset_match_small_integer_arithmetic() {
    for now_ms in 0..=64 {
        for offset_ms in 0..=64 {
            assert_eq!(checked_clock_add(now_ms, offset_ms), Ok(now_ms + offset_ms));
        }
    }
    assert_eq!(checked_clock_add(MAX_CLOCK_MS, 0), Ok(MAX_CLOCK_MS));
}

#[test]
fn input_range_has_priority_over_offset_overflow() {
    for now_ms in [MAX_CLOCK_MS + 1, u64::MAX] {
        for offset_ms in [0, 1, u64::MAX] {
            assert_eq!(
                checked_clock_add(now_ms, offset_ms),
                Err(ClockError::OutOfRange)
            );
        }
    }
}

#[test]
fn result_range_and_full_width_addition_never_wrap_or_silently_clamp() {
    assert_eq!(checked_clock_add(MAX_CLOCK_MS - 1, 1), Ok(MAX_CLOCK_MS));
    for (now_ms, offset_ms) in [
        (MAX_CLOCK_MS, 1),
        (MAX_CLOCK_MS - 1, 2),
        (0, MAX_CLOCK_MS + 1),
        (0, u64::MAX),
        (1, u64::MAX),
        (MAX_CLOCK_MS, u64::MAX),
    ] {
        assert_eq!(
            checked_clock_add(now_ms, offset_ms),
            Err(ClockError::Overflow)
        );
    }
}

#[test]
fn large_clocks_retain_integer_precision_without_a_floating_point_round_trip() {
    let now_ms = (1_u64 << 53) + 1;
    assert_eq!(checked_clock_add(now_ms, 1), Ok(now_ms + 1));
    assert_eq!(
        checked_clock_add(MAX_CLOCK_MS - 17, 16),
        Ok(MAX_CLOCK_MS - 1)
    );
}

#[test]
fn helper_has_no_implicit_clock_domain_or_monotonic_watermark() {
    for (now_ms, expected) in [(100, 107), (50, 57), (200, 207), (50, 57)] {
        assert_eq!(checked_clock_add(now_ms, 7), Ok(expected));
    }
    // Independent calls are pure; the admission handle separately owns monotonicity.
    assert_eq!(checked_clock_add(u64::MAX, 0), Err(ClockError::OutOfRange));
    assert_eq!(checked_clock_add(50, 7), Ok(57));
}

#[test]
fn execution_adapter_preserves_clock_timeout_and_overflow_fault_priority() {
    for (now_ms, timeout_ms, code, message) in [
        (u64::MAX, 0, "LSV2011", "scheduler clock is out of range"),
        (
            MAX_CLOCK_MS,
            0,
            "LSV2012",
            "effect timeout must be between 1 and 86400000 ms",
        ),
        (
            0,
            MAX_EXECUTION_TIMEOUT_MS + 1,
            "LSV2012",
            "effect timeout must be between 1 and 86400000 ms",
        ),
        (
            MAX_CLOCK_MS,
            1,
            "LSV2011",
            "effect absolute deadline is out of range",
        ),
    ] {
        let error = validate_execution_deadline(now_ms, timeout_ms).unwrap_err();
        assert_eq!(error.code, code);
        assert_eq!(error.message, message);
        assert_eq!(
            serde_json::to_value(&error).unwrap(),
            serde_json::json!({"code":code,"message":message})
        );
    }
    assert_eq!(validate_clock(MAX_CLOCK_MS), Ok(()));
    assert_eq!(
        validate_execution_deadline(0, MAX_EXECUTION_TIMEOUT_MS),
        Ok(MAX_EXECUTION_TIMEOUT_MS)
    );
    assert_eq!(
        validate_execution_deadline(MAX_CLOCK_MS - 1, 1),
        Ok(MAX_CLOCK_MS)
    );
}

#[test]
fn admission_overflowing_backoff_clamps_only_to_its_pinned_deadline() {
    struct Busy(usize);
    impl AdmissionAdapter<()> for Busy {
        type Started = ();
        fn try_admit(&mut self, _: &(), _: u64, _: u64) -> AdmissionAttempt<()> {
            self.0 += 1;
            AdmissionAttempt::Backpressured(Fault {
                code: "HOST_BUSY".into(),
                message: "busy".into(),
            })
        }
    }
    let mut host = Busy(0);
    let mut admission = Admission::new(
        (),
        MAX_CLOCK_MS - 2,
        2,
        AdmissionPolicy {
            max_attempts: 3,
            base_delay_ms: 1,
            max_delay_ms: 2,
        },
    )
    .unwrap();
    for (now_ms, retry_at_ms, attempts) in [
        (MAX_CLOCK_MS - 2, MAX_CLOCK_MS - 1, 1),
        (MAX_CLOCK_MS - 1, MAX_CLOCK_MS, 2),
    ] {
        let AdmissionPoll::Waiting(wait) = admission.poll(&mut host, now_ms).unwrap() else {
            panic!("bounded backoff should wait for its pinned deadline");
        };
        assert_eq!(wait.retry_at_ms, retry_at_ms);
        assert_eq!(wait.deadline_at_ms, MAX_CLOCK_MS);
        assert_eq!(wait.attempts, attempts);
    }
    assert!(
        matches!(admission.poll(&mut host, MAX_CLOCK_MS).unwrap(), AdmissionPoll::Rejected(error) if error.code == "LSV2513")
    );
    assert_eq!(host.0, 2);
    assert_eq!(
        admission.poll(&mut host, u64::MAX).unwrap(),
        AdmissionPoll::Finished
    );
}

#[test]
fn arithmetic_errors_are_fixed_metadata_without_host_values_or_causes() {
    for (error, message) in [
        (ClockError::OutOfRange, "scheduler clock is out of range"),
        (
            ClockError::Overflow,
            "scheduler clock addition exceeds the portable range",
        ),
    ] {
        let diagnostic: &dyn std::error::Error = &error;
        assert_eq!(diagnostic.to_string(), message);
        assert!(diagnostic.source().is_none());
    }
    assert!(std::mem::size_of::<ClockError>() <= std::mem::size_of::<u64>());
}
