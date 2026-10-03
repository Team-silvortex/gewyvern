use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use leselang_runtime_core::{
    Admission, AdmissionAdapter, AdmissionAttempt, AdmissionPolicy, AdmissionPoll, AdmissionWait,
    Fault, MAX_ADMISSION_ATTEMPTS, MAX_ADMISSION_DELAY_MS, MAX_EXECUTION_TIMEOUT_MS,
    validate_clock, validate_execution_deadline,
};
use serde_json::json;

// Deliberately neither Clone nor serializable nor a language/product DTO.
#[derive(Debug)]
struct CounterRequest {
    value: u64,
    drops: Arc<AtomicUsize>,
}

impl Drop for CounterRequest {
    fn drop(&mut self) {
        self.drops.fetch_add(1, Ordering::SeqCst);
    }
}

#[derive(Default)]
struct CounterHost {
    outcomes: VecDeque<AdmissionAttempt<u64>>,
    calls: Vec<(u64, u64, u64)>,
}

impl AdmissionAdapter<CounterRequest> for CounterHost {
    type Started = u64;

    fn try_admit(
        &mut self,
        input: &CounterRequest,
        now_ms: u64,
        deadline_at_ms: u64,
    ) -> AdmissionAttempt<u64> {
        self.calls.push((input.value, now_ms, deadline_at_ms));
        self.outcomes
            .pop_front()
            .unwrap_or_else(|| AdmissionAttempt::Backpressured(fault("COUNTER_BUSY")))
    }
}

fn fault(code: &str) -> Fault {
    Fault {
        code: code.into(),
        message: format!("host reported {code}"),
    }
}

fn policy() -> AdmissionPolicy {
    AdmissionPolicy {
        max_attempts: 8,
        base_delay_ms: 10,
        max_delay_ms: 25,
    }
}

fn admission(timeout_ms: u64) -> (Admission<CounterRequest>, Arc<AtomicUsize>) {
    let drops = Arc::new(AtomicUsize::new(0));
    let input = CounterRequest {
        value: 42,
        drops: drops.clone(),
    };
    (
        Admission::new(input, 100, timeout_ms, policy()).unwrap(),
        drops,
    )
}

fn wait(outcome: AdmissionPoll<u64>) -> AdmissionWait {
    match outcome {
        AdmissionPoll::Waiting(wait) => wait,
        other => panic!("expected waiting, got {other:?}"),
    }
}

#[test]
fn construction_does_not_call_host_or_drop_input() {
    let (handle, drops) = admission(1_000);
    assert_eq!(handle.attempts(), 0);
    assert!(!handle.is_finished());
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    drop(handle);
    assert_eq!(drops.load(Ordering::SeqCst), 1);
}

#[test]
fn a_dynamically_selected_adapter_needs_no_runtime_or_product_wrapper() {
    let (mut handle, _) = admission(1_000);
    let mut concrete = CounterHost {
        outcomes: [AdmissionAttempt::Started(3)].into(),
        ..CounterHost::default()
    };
    let host: &mut dyn AdmissionAdapter<CounterRequest, Started = u64> = &mut concrete;
    assert_eq!(handle.poll(host, 100).unwrap(), AdmissionPoll::Started(3));
}

#[test]
fn callback_unwind_consumes_input_without_replaying_unknown_host_side_effects() {
    struct PanickingHost {
        publications: usize,
    }
    impl AdmissionAdapter<CounterRequest> for PanickingHost {
        type Started = u64;
        fn try_admit(&mut self, _: &CounterRequest, _: u64, _: u64) -> AdmissionAttempt<u64> {
            self.publications += 1;
            panic!("host failed after publication");
        }
    }
    let (mut handle, drops) = admission(1_000);
    let mut host = PanickingHost { publications: 0 };
    let result =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| handle.poll(&mut host, 100)));
    assert!(result.is_err());
    assert!(handle.is_finished());
    assert_eq!(handle.attempts(), 1);
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    assert_eq!(
        handle.poll(&mut host, 110).unwrap(),
        AdmissionPoll::Finished
    );
    assert!(!handle.cancel());
    assert_eq!(host.publications, 1);
}

#[test]
fn not_before_polling_spends_no_attempt_or_host_call() {
    let (mut handle, _) = admission(1_000);
    let mut host = CounterHost::default();
    let first = wait(handle.poll(&mut host, 100).unwrap());
    assert_eq!(first.retry_at_ms, 110);
    for now_ms in 100..110 {
        assert_eq!(wait(handle.poll(&mut host, now_ms).unwrap()), first);
    }
    assert_eq!(handle.attempts(), 1);
    assert_eq!(host.calls.len(), 1);
}

#[test]
fn backoff_is_exponential_capped_and_preserves_input_and_deadline() {
    let (mut handle, _) = admission(1_000);
    let mut host = CounterHost::default();
    for (now_ms, retry_at_ms, attempts) in
        [(100, 110, 1), (110, 130, 2), (130, 155, 3), (155, 180, 4)]
    {
        assert_eq!(
            wait(handle.poll(&mut host, now_ms).unwrap()),
            AdmissionWait {
                attempts,
                retry_at_ms,
                deadline_at_ms: 1_100,
            }
        );
    }
    assert_eq!(
        host.calls,
        vec![
            (42, 100, 1_100),
            (42, 110, 1_100),
            (42, 130, 1_100),
            (42, 155, 1_100)
        ]
    );
}

#[test]
fn host_classification_not_error_code_controls_retry() {
    let (mut handle, _) = admission(1_000);
    let mut host = CounterHost {
        outcomes: [
            AdmissionAttempt::Backpressured(fault("UNRELATED_PRESSURE")),
            AdmissionAttempt::Rejected(fault("LSV2501")),
        ]
        .into(),
        ..CounterHost::default()
    };
    assert_eq!(wait(handle.poll(&mut host, 100).unwrap()).attempts, 1);
    assert_eq!(
        handle.poll(&mut host, 110).unwrap(),
        AdmissionPoll::Rejected(fault("LSV2501"))
    );
    assert!(handle.is_finished());
    assert_eq!(host.calls.len(), 2);
}

#[test]
fn successful_start_transfers_output_exactly_once_and_releases_input() {
    let (mut handle, drops) = admission(1_000);
    let mut host = CounterHost {
        outcomes: [AdmissionAttempt::Started(99)].into(),
        ..CounterHost::default()
    };
    assert_eq!(
        handle.poll(&mut host, 100).unwrap(),
        AdmissionPoll::Started(99)
    );
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    assert!(handle.is_finished());
    assert!(!handle.cancel());
    for now_ms in [0, 101, u64::MAX] {
        assert_eq!(
            handle.poll(&mut host, now_ms).unwrap(),
            AdmissionPoll::Finished
        );
    }
    assert_eq!(host.calls.len(), 1);
}

#[test]
fn rejection_releases_input_and_is_not_retried() {
    let (mut handle, drops) = admission(1_000);
    let mut host = CounterHost {
        outcomes: [AdmissionAttempt::Rejected(fault("DENIED"))].into(),
        ..CounterHost::default()
    };
    assert_eq!(
        handle.poll(&mut host, 100).unwrap(),
        AdmissionPoll::Rejected(fault("DENIED"))
    );
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    assert_eq!(
        handle.poll(&mut host, 110).unwrap(),
        AdmissionPoll::Finished
    );
    assert_eq!(host.calls.len(), 1);
}

#[test]
fn cancel_before_first_attempt_or_while_waiting_is_local_and_single_use() {
    for waiting in [false, true] {
        let (mut handle, drops) = admission(1_000);
        let mut host = CounterHost::default();
        if waiting {
            wait(handle.poll(&mut host, 100).unwrap());
        }
        let calls = host.calls.len();
        assert!(handle.cancel());
        assert!(!handle.cancel());
        assert_eq!(drops.load(Ordering::SeqCst), 1);
        assert_eq!(
            handle.poll(&mut host, u64::MAX).unwrap(),
            AdmissionPoll::Finished
        );
        assert_eq!(host.calls.len(), calls);
    }
}

#[test]
fn deadline_wins_before_first_attempt_without_calling_host() {
    let (mut handle, drops) = admission(20);
    let mut host = CounterHost::default();
    assert!(
        matches!(handle.poll(&mut host, 120).unwrap(), AdmissionPoll::Rejected(error) if error.code == "LSV2513")
    );
    assert_eq!(handle.attempts(), 0);
    assert!(host.calls.is_empty());
    assert_eq!(drops.load(Ordering::SeqCst), 1);
}

#[test]
fn wakeup_is_clamped_to_deadline_and_expiry_never_calls_host_again() {
    let (mut handle, drops) = admission(5);
    let mut host = CounterHost::default();
    let waiting = wait(handle.poll(&mut host, 100).unwrap());
    assert_eq!(waiting.retry_at_ms, 105);
    assert_eq!(waiting.deadline_at_ms, 105);
    assert!(
        matches!(handle.poll(&mut host, 105).unwrap(), AdmissionPoll::Rejected(error) if error.code == "LSV2513")
    );
    assert_eq!(host.calls.len(), 1);
    assert_eq!(drops.load(Ordering::SeqCst), 1);
}

#[test]
fn admission_after_wait_receives_original_absolute_deadline() {
    let (mut handle, _) = admission(1_000);
    let mut host = CounterHost {
        outcomes: [
            AdmissionAttempt::Backpressured(fault("BUSY")),
            AdmissionAttempt::Started(7),
        ]
        .into(),
        ..CounterHost::default()
    };
    wait(handle.poll(&mut host, 100).unwrap());
    assert_eq!(
        handle.poll(&mut host, 500).unwrap(),
        AdmissionPoll::Started(7)
    );
    assert_eq!(host.calls, [(42, 100, 1_100), (42, 500, 1_100)]);
}

#[test]
fn invalid_or_regressing_clock_leaves_retry_state_unchanged() {
    let (mut handle, _) = admission(1_000);
    let mut host = CounterHost::default();
    let expected = wait(handle.poll(&mut host, 100).unwrap());
    for (now_ms, code) in [(99, "LSV2511"), (u64::MAX, "LSV2011")] {
        assert_eq!(handle.poll(&mut host, now_ms).unwrap_err().code, code);
        assert_eq!(handle.attempts(), 1);
        assert!(!handle.is_finished());
        assert_eq!(wait(handle.poll(&mut host, 100).unwrap()), expected);
    }
    assert_eq!(host.calls.len(), 1);
    wait(handle.poll(&mut host, 109).unwrap());
    assert_eq!(handle.poll(&mut host, 108).unwrap_err().code, "LSV2511");
    assert_eq!(handle.attempts(), 1);
}

#[test]
fn maximum_attempts_are_total_including_first_and_release_input() {
    let drops = Arc::new(AtomicUsize::new(0));
    let mut handle = Admission::new(
        CounterRequest {
            value: 1,
            drops: drops.clone(),
        },
        0,
        1_000,
        AdmissionPolicy {
            max_attempts: MAX_ADMISSION_ATTEMPTS,
            base_delay_ms: 1,
            max_delay_ms: 3,
        },
    )
    .unwrap();
    let mut host = CounterHost::default();
    let mut now_ms = 0;
    for attempts in 1..MAX_ADMISSION_ATTEMPTS {
        let waiting = wait(handle.poll(&mut host, now_ms).unwrap());
        assert_eq!(waiting.attempts, attempts);
        now_ms = waiting.retry_at_ms;
    }
    assert!(
        matches!(handle.poll(&mut host, now_ms).unwrap(), AdmissionPoll::Rejected(error) if error.code == "LSV2512" && error.message.contains("32 attempts"))
    );
    assert_eq!(handle.attempts(), MAX_ADMISSION_ATTEMPTS);
    assert_eq!(host.calls.len(), MAX_ADMISSION_ATTEMPTS as usize);
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    assert_eq!(
        handle.poll(&mut host, now_ms).unwrap(),
        AdmissionPoll::Finished
    );
}

#[test]
fn one_attempt_policy_rejects_first_pressure_immediately() {
    let mut handle = Admission::new(
        (),
        0,
        1_000,
        AdmissionPolicy {
            max_attempts: 1,
            ..policy()
        },
    )
    .unwrap();
    struct Busy;
    impl AdmissionAdapter<()> for Busy {
        type Started = ();
        fn try_admit(&mut self, _: &(), _: u64, _: u64) -> AdmissionAttempt<()> {
            AdmissionAttempt::Backpressured(fault("BUSY"))
        }
    }
    assert!(
        matches!(handle.poll(&mut Busy, 0).unwrap(), AdmissionPoll::Rejected(error) if error.code == "LSV2512")
    );
    assert_eq!(handle.attempts(), 1);
    assert!(handle.is_finished());
}

#[test]
fn invalid_policies_are_rejected_before_admission() {
    for invalid in [
        AdmissionPolicy {
            max_attempts: 0,
            ..policy()
        },
        AdmissionPolicy {
            max_attempts: MAX_ADMISSION_ATTEMPTS + 1,
            ..policy()
        },
        AdmissionPolicy {
            base_delay_ms: 0,
            ..policy()
        },
        AdmissionPolicy {
            base_delay_ms: 26,
            ..policy()
        },
        AdmissionPolicy {
            max_delay_ms: MAX_ADMISSION_DELAY_MS + 1,
            ..policy()
        },
        AdmissionPolicy {
            base_delay_ms: MAX_ADMISSION_DELAY_MS + 1,
            max_delay_ms: MAX_ADMISSION_DELAY_MS + 1,
            ..policy()
        },
    ] {
        assert_eq!(
            Admission::new((), 0, 1, invalid).unwrap_err().code,
            "LSV2510"
        );
    }
    assert!(
        Admission::new(
            (),
            0,
            1,
            AdmissionPolicy {
                max_attempts: MAX_ADMISSION_ATTEMPTS,
                base_delay_ms: MAX_ADMISSION_DELAY_MS,
                max_delay_ms: MAX_ADMISSION_DELAY_MS
            }
        )
        .is_ok()
    );
}

#[test]
fn clock_and_timeout_boundaries_are_checked_without_overflow() {
    assert_eq!(validate_clock(i64::MAX as u64), Ok(()));
    assert_eq!(
        validate_clock(i64::MAX as u64 + 1).unwrap_err().code,
        "LSV2011"
    );
    assert_eq!(
        validate_execution_deadline(0, MAX_EXECUTION_TIMEOUT_MS),
        Ok(MAX_EXECUTION_TIMEOUT_MS)
    );
    assert_eq!(
        validate_execution_deadline(i64::MAX as u64 - 1, 1),
        Ok(i64::MAX as u64)
    );
    for timeout_ms in [0, MAX_EXECUTION_TIMEOUT_MS + 1] {
        assert_eq!(
            Admission::new((), 0, timeout_ms, policy())
                .unwrap_err()
                .code,
            "LSV2012"
        );
    }
    for now_ms in [i64::MAX as u64, u64::MAX] {
        assert_eq!(
            Admission::new((), now_ms, 1, policy()).unwrap_err().code,
            "LSV2011"
        );
    }
}

#[test]
fn unrelated_counter_and_text_hosts_use_distinct_opaque_inputs_and_outputs() {
    struct TextRequest {
        text: String,
    }
    struct TextReceipt {
        bytes: usize,
    }
    struct TextHost;
    impl AdmissionAdapter<TextRequest> for TextHost {
        type Started = TextReceipt;
        fn try_admit(
            &mut self,
            input: &TextRequest,
            _: u64,
            _: u64,
        ) -> AdmissionAttempt<TextReceipt> {
            AdmissionAttempt::Started(TextReceipt {
                bytes: input.text.len(),
            })
        }
    }
    let mut text = Admission::new(
        TextRequest {
            text: "host-neutral".into(),
        },
        0,
        100,
        policy(),
    )
    .unwrap();
    let AdmissionPoll::Started(receipt) = text.poll(&mut TextHost, 0).unwrap() else {
        panic!("text host should start");
    };
    assert_eq!(receipt.bytes, 12);
    let (mut counter, _) = admission(1_000);
    let mut host = CounterHost {
        outcomes: [AdmissionAttempt::Started(42)].into(),
        ..CounterHost::default()
    };
    assert_eq!(
        counter.poll(&mut host, 100).unwrap(),
        AdmissionPoll::Started(42)
    );
}

#[test]
fn independent_instances_can_be_owned_and_polled_by_separate_threads() {
    let workers: Vec<_> = (0..8)
        .map(|value| {
            let (mut handle, _) = admission(1_000);
            std::thread::spawn(move || {
                let mut host = CounterHost {
                    outcomes: [AdmissionAttempt::Started(value)].into(),
                    ..CounterHost::default()
                };
                assert_eq!(
                    handle.poll(&mut host, 100).unwrap(),
                    AdmissionPoll::Started(value)
                );
                assert_eq!(host.calls.len(), 1);
                assert_eq!(handle.attempts(), 1);
            })
        })
        .collect();
    for worker in workers {
        worker.join().unwrap();
    }
}

#[test]
fn faults_and_observation_json_keep_reference_wire_shapes() {
    let error = fault("DENIED");
    let bytes = serde_json::to_vec(&error).unwrap();
    assert_eq!(serde_json::from_slice::<Fault>(&bytes).unwrap(), error);
    assert_eq!(
        serde_json::to_value(AdmissionPoll::<u64>::Rejected(error)).unwrap(),
        json!({"kind":"rejected","payload":{"code":"DENIED","message":"host reported DENIED"}})
    );
    assert_eq!(
        serde_json::to_value(AdmissionPoll::Started(7_u64)).unwrap(),
        json!({"kind":"started","payload":7})
    );
    assert_eq!(
        serde_json::to_value(AdmissionPoll::<u64>::Finished).unwrap(),
        json!({"kind":"finished"})
    );
    assert_eq!(
        serde_json::to_value(AdmissionPoll::<u64>::Waiting(AdmissionWait {
            attempts: 1,
            retry_at_ms: 10,
            deadline_at_ms: 100
        }))
        .unwrap(),
        json!({"kind":"waiting","payload":{"attempts":1,"retry_at_ms":10,"deadline_at_ms":100}})
    );
    assert!(
        serde_json::from_value::<AdmissionPolicy>(
            json!({"max_attempts":8,"base_delay_ms":250,"max_delay_ms":30000,"unexpected":true})
        )
        .is_err()
    );
}
