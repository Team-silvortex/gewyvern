use leselang_runtime_core::{
    Admission, AdmissionAdapter, AdmissionAttempt, AdmissionEnd, AdmissionPolicy, AdmissionPoll,
    AdmissionStatus, AdmissionWait, Fault,
};
use serde_json::json;

const PRIVATE_DATA: &str = "PRIVATE_HOST_INPUT_MARKER";

#[derive(Debug)]
struct PrivateInput {
    data: String,
}

struct Host;
impl AdmissionAdapter<PrivateInput> for Host {
    type Started = usize;
    fn try_admit(&mut self, input: &PrivateInput, _: u64, _: u64) -> AdmissionAttempt<usize> {
        AdmissionAttempt::Started(input.data.len())
    }
}

#[test]
fn debug_output_does_not_disclose_host_input() {
    let mut handle = Admission::new(
        PrivateInput {
            data: PRIVATE_DATA.into(),
        },
        100,
        1_000,
        AdmissionPolicy::default(),
    )
    .unwrap();
    for debug in [format!("{handle:?}"), format!("{handle:#?}")] {
        assert!(
            !debug.contains(PRIVATE_DATA),
            "input leaked in admission debug output"
        );
        assert!(debug.contains("Pending"));
    }
    assert_eq!(
        handle.poll(&mut Host, 100).unwrap(),
        AdmissionPoll::Started(PRIVATE_DATA.len())
    );
    assert!(format!("{handle:?}").contains("Finished"));
}

#[test]
fn debug_requires_no_input_debug_and_never_calls_an_input_formatter() {
    struct Opaque;
    let opaque = Admission::new(Opaque, 100, 1_000, AdmissionPolicy::default()).unwrap();
    assert!(format!("{opaque:?}").contains("Pending"));

    struct Trap;
    impl std::fmt::Debug for Trap {
        fn fmt(&self, _: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            panic!("host-owned input formatter must not run");
        }
    }
    let trap = Admission::new(Trap, 100, 1_000, AdmissionPolicy::default()).unwrap();
    assert!(format!("{trap:#?}").contains("Pending"));
}

#[derive(Default)]
struct BusyHost {
    calls: usize,
}

impl AdmissionAdapter<PrivateInput> for BusyHost {
    type Started = usize;
    fn try_admit(&mut self, _: &PrivateInput, _: u64, _: u64) -> AdmissionAttempt<usize> {
        self.calls += 1;
        AdmissionAttempt::Backpressured(Fault {
            code: "HOST_BUSY".into(),
            message: "busy".into(),
        })
    }
}

fn private_admission() -> Admission<PrivateInput> {
    Admission::new(
        PrivateInput {
            data: PRIVATE_DATA.into(),
        },
        100,
        1_000,
        AdmissionPolicy::default(),
    )
    .unwrap()
}

#[test]
fn repeated_status_observation_spends_no_attempt_or_clock_progress() {
    let mut handle = private_admission();
    let mut host = BusyHost::default();
    let initial = AdmissionStatus::Pending(AdmissionWait {
        attempts: 0,
        retry_at_ms: 100,
        deadline_at_ms: 1_100,
    });
    for _ in 0..100 {
        assert_eq!(handle.status(), initial);
    }
    assert_eq!(host.calls, 0);
    assert_eq!(handle.attempts(), 0);
    let AdmissionPoll::Waiting(wait) = handle.poll(&mut host, 100).unwrap() else {
        panic!("expected pressure");
    };
    let expected = AdmissionStatus::Pending(wait);
    for _ in 0..100 {
        assert_eq!(handle.status(), expected);
        let debug = format!("{handle:?}");
        assert!(!debug.contains(PRIVATE_DATA));
        assert!(!debug.contains("HOST_BUSY"));
    }
    assert_eq!(host.calls, 1);
    assert_eq!(handle.poll(&mut host, 99).unwrap_err().code, "LSV2511");
    assert_eq!(
        handle.poll(&mut host, u64::MAX).unwrap_err().code,
        "LSV2011"
    );
    assert_eq!(handle.status(), expected);
    assert_eq!(
        handle.poll(&mut host, 100).unwrap(),
        AdmissionPoll::Waiting(wait)
    );
    assert_eq!(host.calls, 1);
}

#[test]
fn finished_status_records_attempts_but_cannot_repeat_start_or_cancellation() {
    let mut handle = private_admission();
    assert!(handle.cancel());
    assert_eq!(handle.status(), AdmissionStatus::Finished { attempts: 0 });
    assert!(!handle.cancel());

    let mut handle = private_admission();
    let mut host = Host;
    assert_eq!(
        handle.poll(&mut host, 100).unwrap(),
        AdmissionPoll::Started(PRIVATE_DATA.len())
    );
    assert_eq!(handle.status(), AdmissionStatus::Finished { attempts: 1 });
    assert_eq!(
        handle.poll(&mut host, u64::MAX).unwrap(),
        AdmissionPoll::Finished
    );
    assert_eq!(handle.status(), AdmissionStatus::Finished { attempts: 1 });
}

#[test]
fn status_observation_does_not_passively_expire_or_schedule_work() {
    let mut handle = private_admission();
    let mut host = Host;
    assert!(matches!(handle.status(), AdmissionStatus::Pending(_)));
    assert!(
        matches!(handle.poll(&mut host, 1_100).unwrap(), AdmissionPoll::Rejected(error) if error.code == "LSV2513")
    );
    assert_eq!(handle.status(), AdmissionStatus::Finished { attempts: 0 });
}

#[test]
fn status_json_is_metadata_only_and_needs_no_input_serialization() {
    let handle = private_admission();
    assert_eq!(
        serde_json::to_value(handle.status()).unwrap(),
        json!({"kind":"pending","payload":{"attempts":0,"retry_at_ms":100,"deadline_at_ms":1100}})
    );
    let encoded = serde_json::to_string(&handle.status()).unwrap();
    assert!(!encoded.contains(PRIVATE_DATA));
    assert!(!encoded.contains("data"));
    let mut handle = handle;
    assert!(handle.cancel());
    assert_eq!(
        serde_json::to_value(handle.status()).unwrap(),
        json!({"kind":"finished","payload":{"attempts":0}})
    );
}

#[test]
fn terminal_input_cleanup_unwind_cannot_rearm_any_lifecycle_path() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct Input(Arc<AtomicUsize>);
    impl Drop for Input {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
            panic!("host-owned cleanup failed");
        }
    }
    struct CleanupHost {
        outcome: u8,
        calls: usize,
    }
    impl AdmissionAdapter<Input> for CleanupHost {
        type Started = ();
        fn try_admit(&mut self, _: &Input, _: u64, _: u64) -> AdmissionAttempt<()> {
            self.calls += 1;
            match self.outcome {
                2 => AdmissionAttempt::Started(()),
                3 => AdmissionAttempt::Rejected(Fault {
                    code: "DENIED".into(),
                    message: "denied".into(),
                }),
                _ => AdmissionAttempt::Backpressured(Fault {
                    code: "BUSY".into(),
                    message: "busy".into(),
                }),
            }
        }
    }
    // Cancellation, expiry, success, rejection and exhausted pressure all consume input.
    for outcome in 0..5 {
        let drops = Arc::new(AtomicUsize::new(0));
        let mut handle = Admission::new(
            Input(drops.clone()),
            100,
            1_000,
            AdmissionPolicy {
                max_attempts: 1,
                ..AdmissionPolicy::default()
            },
        )
        .unwrap();
        let mut host = CleanupHost { outcome, calls: 0 };
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            if outcome == 0 {
                handle.cancel();
            } else {
                let _ = handle.poll(&mut host, if outcome == 1 { 1_100 } else { 100 });
            }
        }));
        assert!(result.is_err());
        let attempts = u32::from(outcome >= 2);
        assert_eq!(handle.status(), AdmissionStatus::Finished { attempts });
        assert_eq!(
            handle.terminal_reason(),
            Some(match outcome {
                0 => AdmissionEnd::Cancelled,
                1 => AdmissionEnd::Expired,
                2 => AdmissionEnd::Started,
                3 => AdmissionEnd::Rejected,
                _ => AdmissionEnd::Exhausted,
            })
        );
        assert_eq!(drops.load(Ordering::SeqCst), 1);
        assert_eq!(host.calls, attempts as usize);
        assert!(!handle.cancel());
        assert_eq!(
            handle.poll(&mut host, u64::MAX).unwrap(),
            AdmissionPoll::Finished
        );
        drop(handle);
        assert_eq!(drops.load(Ordering::SeqCst), 1);
    }
}
