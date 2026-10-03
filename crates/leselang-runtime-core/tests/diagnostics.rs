use std::collections::VecDeque;

use leselang_runtime_core::{
    Admission, AdmissionAdapter, AdmissionAttempt, AdmissionPolicy, AdmissionPoll, Fault,
    MAX_ADMISSION_ATTEMPTS, MAX_ADMISSION_DELAY_MS,
};

#[derive(Default)]
struct Host {
    outcomes: VecDeque<AdmissionAttempt<()>>,
    calls: usize,
}

impl AdmissionAdapter<()> for Host {
    type Started = ();
    fn try_admit(&mut self, _: &(), _: u64, _: u64) -> AdmissionAttempt<()> {
        self.calls += 1;
        self.outcomes.pop_front().unwrap()
    }
}

fn policy(max_attempts: u32) -> AdmissionPolicy {
    AdmissionPolicy {
        max_attempts,
        base_delay_ms: 1,
        max_delay_ms: 1,
    }
}

fn rejected(outcome: AdmissionPoll<()>) -> Fault {
    match outcome {
        AdmissionPoll::Rejected(fault) => fault,
        other => panic!("expected rejection, got {other:?}"),
    }
}

#[test]
fn exhaustion_does_not_echo_private_host_codes_or_messages() {
    let mut admission = Admission::new((), 100, 1_000, policy(1)).unwrap();
    let mut host = Host {
        outcomes: [AdmissionAttempt::Backpressured(Fault {
            code: "PRIVATE_HOST_CODE_MARKER".into(),
            message: "PRIVATE_HOST_MESSAGE_MARKER\nwith control characters\0".into(),
        })]
        .into(),
        ..Host::default()
    };
    let fault = rejected(admission.poll(&mut host, 100).unwrap());
    assert_eq!(fault.code, "LSV2512");
    assert_eq!(fault.message, "admission exhausted after 1 attempt");
    let json = serde_json::to_string(&fault).unwrap();
    assert!(!json.contains("PRIVATE_HOST"));
    assert!(admission.is_finished());
    assert_eq!(host.calls, 1);
}

#[test]
fn exhaustion_output_is_bounded_independently_of_host_error_size() {
    let messages = [String::new(), "PRIVATE_LONG_MESSAGE".repeat(100_000)];
    let mut sizes = Vec::new();
    for message in messages {
        let mut admission = Admission::new((), 0, 1_000, policy(1)).unwrap();
        let mut host = Host {
            outcomes: [AdmissionAttempt::Backpressured(Fault {
                code: "PRIVATE_LONG_CODE".repeat(10_000),
                message,
            })]
            .into(),
            ..Host::default()
        };
        let fault = rejected(admission.poll(&mut host, 0).unwrap());
        assert_eq!(fault.message, "admission exhausted after 1 attempt");
        let bytes = serde_json::to_vec(&fault).unwrap();
        assert!(bytes.len() < 128);
        sizes.push(bytes.len());
    }
    assert_eq!(sizes[0], sizes[1]);
}

#[test]
fn maximum_attempts_keep_a_short_diagnostic_and_never_replay_the_failure() {
    let mut admission = Admission::new((), 0, 1_000, policy(MAX_ADMISSION_ATTEMPTS)).unwrap();
    let mut host = Host {
        outcomes: (0..MAX_ADMISSION_ATTEMPTS)
            .map(|_| {
                AdmissionAttempt::Backpressured(Fault {
                    code: "HOST_PRIVATE_CODE".into(),
                    message: "HOST_PRIVATE_MESSAGE".into(),
                })
            })
            .collect(),
        ..Host::default()
    };
    for now_ms in 0..u64::from(MAX_ADMISSION_ATTEMPTS - 1) {
        assert!(matches!(
            admission.poll(&mut host, now_ms).unwrap(),
            AdmissionPoll::Waiting(_)
        ));
    }
    let fault = rejected(
        admission
            .poll(&mut host, u64::from(MAX_ADMISSION_ATTEMPTS - 1))
            .unwrap(),
    );
    assert_eq!(fault.message, "admission exhausted after 32 attempts");
    assert_eq!(host.calls, MAX_ADMISSION_ATTEMPTS as usize);
    assert_eq!(
        admission.poll(&mut host, u64::MAX).unwrap(),
        AdmissionPoll::Finished
    );
    assert_eq!(host.calls, MAX_ADMISSION_ATTEMPTS as usize);
}

#[test]
fn permanent_host_rejections_remain_verbatim_even_with_admission_like_codes() {
    for code in ["DENIED", "LSV2501", "LSV2512"] {
        let expected = Fault {
            code: code.into(),
            message: "HOST_OWNED_DETAIL\n\0".into(),
        };
        let mut admission = Admission::new((), 0, 1_000, policy(8)).unwrap();
        let mut host = Host {
            outcomes: [AdmissionAttempt::Rejected(expected.clone())].into(),
            ..Host::default()
        };
        assert_eq!(rejected(admission.poll(&mut host, 0).unwrap()), expected);
        assert_eq!(
            admission.poll(&mut host, 1).unwrap(),
            AdmissionPoll::Finished
        );
        assert_eq!(host.calls, 1);
    }
}

#[test]
fn policy_preflight_names_each_constraint_without_echoing_submitted_values() {
    for (invalid, expected) in [
        (
            AdmissionPolicy {
                max_attempts: 0,
                ..policy(1)
            },
            "admission max_attempts must be between 1 and 32",
        ),
        (
            AdmissionPolicy {
                max_attempts: u32::MAX,
                ..policy(1)
            },
            "admission max_attempts must be between 1 and 32",
        ),
        (
            AdmissionPolicy {
                base_delay_ms: 0,
                ..policy(1)
            },
            "admission base_delay_ms must be between 1 and 3600000 ms",
        ),
        (
            AdmissionPolicy {
                base_delay_ms: u64::MAX,
                ..policy(1)
            },
            "admission base_delay_ms must be between 1 and 3600000 ms",
        ),
        (
            AdmissionPolicy {
                max_delay_ms: u64::MAX,
                ..policy(1)
            },
            "admission max_delay_ms must not exceed 3600000 ms",
        ),
        (
            AdmissionPolicy {
                max_delay_ms: 0,
                ..policy(1)
            },
            "admission max_delay_ms must be at least base_delay_ms",
        ),
        (
            AdmissionPolicy {
                base_delay_ms: 10,
                max_delay_ms: 9,
                ..policy(1)
            },
            "admission max_delay_ms must be at least base_delay_ms",
        ),
    ] {
        let fault = invalid.validate().unwrap_err();
        assert_eq!(fault.code, "LSV2510");
        assert_eq!(fault.message, expected);
        assert!(fault.message.len() < 128);
        assert!(!fault.message.contains(&u64::MAX.to_string()));
    }
}

#[test]
fn multiple_policy_violations_have_deterministic_preflight_priority() {
    let mut invalid = AdmissionPolicy {
        max_attempts: 0,
        base_delay_ms: 0,
        max_delay_ms: u64::MAX,
    };
    assert!(
        invalid
            .validate()
            .unwrap_err()
            .message
            .starts_with("admission max_attempts ")
    );
    invalid.max_attempts = 1;
    assert!(
        invalid
            .validate()
            .unwrap_err()
            .message
            .starts_with("admission base_delay_ms ")
    );
    invalid.base_delay_ms = 1;
    assert!(
        invalid
            .validate()
            .unwrap_err()
            .message
            .starts_with("admission max_delay_ms ")
    );
}

#[test]
fn pure_preflight_is_nonmutating_and_matches_constructor_validation() {
    for invalid in [
        AdmissionPolicy {
            max_attempts: 0,
            ..policy(1)
        },
        AdmissionPolicy {
            base_delay_ms: 0,
            ..policy(1)
        },
        AdmissionPolicy {
            max_delay_ms: u64::MAX,
            ..policy(1)
        },
        AdmissionPolicy {
            base_delay_ms: 2,
            ..policy(1)
        },
    ] {
        let original = invalid;
        let error = invalid.validate().unwrap_err();
        assert_eq!(invalid, original);
        assert_eq!(Admission::new((), 0, 1_000, invalid).unwrap_err(), error);
        // Policy validation precedes even invalid clock/deadline input, as before.
        assert_eq!(Admission::new((), u64::MAX, 0, invalid).unwrap_err(), error);
    }
}

#[test]
fn valid_policy_preflight_accepts_defaults_and_inclusive_edges() {
    for valid in [
        AdmissionPolicy::default(),
        policy(1),
        AdmissionPolicy {
            max_attempts: MAX_ADMISSION_ATTEMPTS,
            base_delay_ms: MAX_ADMISSION_DELAY_MS,
            max_delay_ms: MAX_ADMISSION_DELAY_MS,
        },
    ] {
        assert_eq!(valid.validate(), Ok(()));
        assert!(Admission::new((), 0, 1_000, valid).is_ok());
    }
}
