use leselang_command::reference_dispatch::{
    DEFAULT_DISPATCH_LEASE_MS, MAX_DISPATCH_ATTEMPTS, MAX_DISPATCH_LEASE_MS, lease_expiration,
    next_attempt, validate_dispatch_clock, validate_lease_clock, validate_observed_lease,
    validate_proposed_retry,
};
use leselang_command::{
    ContinuationImage, ContinuationToken, DispatchLease, EffectError, EffectErrorClass,
    EffectOperation, EffectRequest, PresentationEnvelope, PresentationOperation, RetrySchedule,
};
use leselang_hir::{Effect, Type};
use leselang_host_contract::{CapabilitySet, Principal};
use leselang_runtime_core::{DispatchLeaseObservation, Fault};

fn lease() -> DispatchLease {
    let continuation = ContinuationImage {
        schema_version: 1,
        token: ContinuationToken::from_wire("continuation-7".into()),
        program_counter: 1,
        expected_revision: None,
        result_type: Type::UiFocus,
        pending_effect: Effect::UiFocus {
            node_id: "a".into(),
        },
        fuel_remaining: 99,
        deadline_ms: 30_000,
        deadline_at_ms: Some(40_000),
        max_output_items: 10_000,
        result_binding: None,
        group_result: None,
    };
    DispatchLease {
        request: EffectRequest {
            effect_id: "effect-7".into(),
            required_capability: "ui.presentation".into(),
            operation: EffectOperation::Presentation(PresentationEnvelope {
                schema_version: 1,
                principal: Principal::new("operator").unwrap(),
                capabilities: CapabilitySet::new(["ui.presentation"]),
                operation: PresentationOperation::Focus {
                    node_id: "a".into(),
                },
            }),
            budget: continuation.resource_budget(),
            continuation,
        },
        attempt: 1,
        retry_count: 0,
        lease_expires_at_ms: 100,
    }
}

// Frozen product decisions deliberately do not call adapter or shared kernels.
mod frozen {
    use super::{DispatchLease, DispatchLeaseObservation, Fault, RetrySchedule};

    pub fn fault(code: &str, message: &str) -> Fault {
        Fault {
            code: code.into(),
            message: message.into(),
        }
    }

    pub fn expiry(now: u64, duration: u64) -> Result<u64, Fault> {
        if now > i64::MAX as u64 {
            return Err(fault("LSV4015", "dispatch clock is out of range"));
        }
        if duration == 0 || duration > 300_000 {
            return Err(fault(
                "LSV4015",
                "dispatch lease must be between 1 and 300000 ms",
            ));
        }
        match now.checked_add(duration) {
            Some(end) if end <= i64::MAX as u64 => Ok(end),
            _ => Err(fault(
                "LSV4015",
                "dispatch lease expiration is out of range",
            )),
        }
    }

    pub fn attempt(current: u32) -> Result<u32, Fault> {
        if current == u32::MAX {
            Err(fault("LSV4017", "dispatch attempt counter is exhausted"))
        } else if current >= 100 {
            Err(fault("LSV4017", "dispatch attempt limit 100 reached"))
        } else {
            Ok(current + 1)
        }
    }

    pub fn observed(
        lease: &DispatchLease,
        now: u64,
        current: DispatchLeaseObservation,
    ) -> Result<(), Fault> {
        if !current.same_request
            || !current.active
            || current.attempt != lease.attempt
            || current.retry_count != lease.retry_count
            || current.ready_at_ms > now
            || current.expires_at_ms != Some(lease.lease_expires_at_ms)
        {
            Err(fault("LSV4022", "dispatch lease has been superseded"))
        } else if lease.lease_expires_at_ms < now {
            Err(fault("LSV4023", "dispatch lease has expired"))
        } else {
            Ok(())
        }
    }

    pub fn retry(current: u32, schedule: &RetrySchedule, now: u64) -> Result<(), Fault> {
        let error = &schedule.error;
        if current.checked_add(1) != Some(schedule.retry_count)
            || schedule.retry_count > 16
            || schedule.ready_at_ms <= now
            || schedule.ready_at_ms > i64::MAX as u64
            || error.code.is_empty()
            || error.code.len() > 64
            || !error
                .code
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_'))
            || error.message.is_empty()
            || error.message.len() > 1024
        {
            Err(fault("LSV4026", "retry schedule is invalid"))
        } else {
            Ok(())
        }
    }
}

#[test]
fn issuance_and_attempt_policy_match_frozen_limits_and_error_precedence() {
    assert_eq!(
        (
            DEFAULT_DISPATCH_LEASE_MS,
            MAX_DISPATCH_LEASE_MS,
            MAX_DISPATCH_ATTEMPTS
        ),
        (30_000, 300_000, 100)
    );
    let max = i64::MAX as u64;
    let mut count = 0;
    for now in [0, 1, max - 300_000, max - 1, max, max + 1, u64::MAX] {
        let expected = if now <= max {
            Ok(())
        } else {
            Err(frozen::fault("LSV4015", "dispatch clock is out of range"))
        };
        assert_eq!(validate_dispatch_clock(now), expected);
        for duration in [0, 1, 30_000, 300_000, 300_001, max, u64::MAX] {
            let expected = frozen::expiry(now, duration);
            assert_eq!(lease_expiration(now, duration), expected);
            assert_eq!(validate_lease_clock(now, duration), expected.map(|_| ()));
            count += 1;
        }
    }
    assert_eq!(count, 49);
    for current in (0..=102).chain([u32::MAX - 1, u32::MAX]) {
        assert_eq!(next_attempt(current), frozen::attempt(current));
    }
    assert_eq!(validate_dispatch_clock(max), Ok(()));
    assert!(lease_expiration(max, 1).is_err());
}

#[test]
fn observed_lease_matrix_preserves_currentness_before_expiry_without_authenticating_data() {
    let mut count = 0;
    let mut lease = lease();
    for (attempt, retry_count) in [(0, 0), (1, 0), (100, 16), (u32::MAX, u32::MAX)] {
        lease.attempt = attempt;
        lease.retry_count = retry_count;
        let bytes = serde_json::to_vec(&lease).unwrap();
        for now in [99, 100, 101] {
            for mask in 0..64 {
                let observation = DispatchLeaseObservation {
                    same_request: mask & 1 == 0,
                    active: mask & 2 == 0,
                    attempt: if mask & 4 == 0 { attempt } else { attempt ^ 1 },
                    retry_count: if mask & 8 == 0 {
                        retry_count
                    } else {
                        retry_count ^ 1
                    },
                    ready_at_ms: if mask & 16 == 0 { now } else { now + 1 },
                    expires_at_ms: if mask & 32 == 0 { Some(100) } else { None },
                };
                assert_eq!(
                    validate_observed_lease(&lease, now, observation),
                    frozen::observed(&lease, now, observation)
                );
                count += 1;
            }
        }
        assert_eq!(serde_json::to_vec(&lease).unwrap(), bytes);
    }
    assert_eq!(count, 768);
}

#[test]
fn retry_admission_matches_frozen_counts_clock_metadata_and_bytes() {
    let max = i64::MAX as u64;
    let metadata = [
        ("ok".into(), "retry".into()),
        ("a".repeat(64), "x".repeat(1024)),
        ("a".repeat(65), "retry".into()),
        ("ok".into(), "x".repeat(1025)),
        (String::new(), "retry".into()),
        ("ok".into(), String::new()),
        ("bad/code".into(), "retry".into()),
        ("\u{e9}".into(), "retry".into()),
    ];
    let mut count = 0;
    for current in [0, 1, 15, 16, u32::MAX] {
        for retry_count in [0, 1, 2, 16, 17, u32::MAX] {
            for ready_at_ms in [0, 1, 2, max, max + 1] {
                for now in [0, 1, max, u64::MAX] {
                    for (code, message) in &metadata {
                        for class in [EffectErrorClass::Transient, EffectErrorClass::Permanent] {
                            let schedule = RetrySchedule {
                                retry_count,
                                ready_at_ms,
                                error: EffectError {
                                    class,
                                    code: code.clone(),
                                    message: message.clone(),
                                },
                            };
                            let bytes = serde_json::to_vec(&schedule).unwrap();
                            assert_eq!(
                                validate_proposed_retry(current, &schedule, now),
                                frozen::retry(current, &schedule, now)
                            );
                            assert_eq!(serde_json::to_vec(&schedule).unwrap(), bytes);
                            count += 1;
                        }
                    }
                }
            }
        }
    }
    assert_eq!(count, 9600);
}

#[test]
fn static_observations_do_not_validate_request_identity_or_classify_errors() {
    let mut lease = lease();
    lease.request.effect_id.clear();
    let observation = DispatchLeaseObservation {
        same_request: true,
        active: true,
        attempt: 1,
        retry_count: 0,
        ready_at_ms: 100,
        expires_at_ms: Some(100),
    };
    assert_eq!(validate_observed_lease(&lease, 100, observation), Ok(()));
    let schedule = RetrySchedule {
        retry_count: 1,
        ready_at_ms: 101,
        error: EffectError {
            class: EffectErrorClass::Permanent,
            code: "denied".into(),
            message: "not retryable".into(),
        },
    };
    assert_eq!(validate_proposed_retry(0, &schedule, 100), Ok(()));
    // Classification belongs to the existing planner, not numeric proposal admission.
    assert_eq!(
        leselang_command::retry_policy::plan_effect_retry(
            0,
            100,
            &schedule.error,
            &Default::default()
        ),
        Ok(None)
    );
}
