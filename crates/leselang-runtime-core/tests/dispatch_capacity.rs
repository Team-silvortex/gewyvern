use leselang_runtime_core::{
    Admission, AdmissionAdapter, AdmissionAttempt, AdmissionEnd, AdmissionPolicy, AdmissionPoll,
    DispatchAdmissionError, Fault, check_dispatch_admission, dispatch_lease_capacity_exhausted,
};

#[test]
fn admission_matches_a_wide_oracle_including_overflow_and_zero_limits() {
    let counts = [
        0,
        1,
        2,
        3,
        10,
        100,
        usize::MAX / 2,
        usize::MAX - 1,
        usize::MAX,
    ];
    let mut cases = 0;
    for pending in counts {
        for additional in counts {
            for limit in counts {
                let expected = if additional > limit {
                    Err(DispatchAdmissionError::BatchTooLarge)
                } else if pending as u128 + additional as u128 > limit as u128 {
                    Err(DispatchAdmissionError::Backpressured)
                } else {
                    Ok(())
                };
                assert_eq!(
                    check_dispatch_admission(pending, additional, limit),
                    expected
                );
                cases += 1;
            }
        }
    }
    assert_eq!(cases, 729);
}

#[test]
fn oversized_batches_are_permanent_before_queue_pressure() {
    for pending in [0, 5, usize::MAX] {
        assert_eq!(
            check_dispatch_admission(pending, 6, 5),
            Err(DispatchAdmissionError::BatchTooLarge)
        );
    }
    assert_eq!(check_dispatch_admission(4, 1, 5), Ok(()));
    assert_eq!(
        check_dispatch_admission(5, 1, 5),
        Err(DispatchAdmissionError::Backpressured)
    );
    assert_eq!(check_dispatch_admission(usize::MAX, 0, usize::MAX), Ok(()));
    assert_eq!(
        check_dispatch_admission(usize::MAX, 1, usize::MAX),
        Err(DispatchAdmissionError::Backpressured)
    );
}

#[test]
fn lowering_limits_does_not_assume_occupancy_is_within_the_new_limit() {
    assert_eq!(
        check_dispatch_admission(3, 0, 2),
        Err(DispatchAdmissionError::Backpressured)
    );
    let counts = [0, 1, 2, 3, 10, usize::MAX - 1, usize::MAX];
    for active in counts {
        for limit in counts {
            assert_eq!(
                dispatch_lease_capacity_exhausted(active, limit),
                active >= limit
            );
        }
    }
}

#[test]
fn capacity_observations_neither_reserve_nor_release_slots() {
    let pending = 2;
    for _ in 0..10 {
        assert_eq!(check_dispatch_admission(pending, 1, 3), Ok(()));
        assert!(!dispatch_lease_capacity_exhausted(0, 1));
    }
    // Once a host has admitted another batch it must re-read, not reuse success.
    assert_eq!(
        check_dispatch_admission(pending + 1, 1, 3),
        Err(DispatchAdmissionError::Backpressured)
    );
    assert!(dispatch_lease_capacity_exhausted(1, 1));
}

#[derive(Default)]
struct Host {
    pending: usize,
    limit: usize,
    calls: usize,
    published: Vec<usize>,
}

impl AdmissionAdapter<usize> for Host {
    type Started = usize;

    fn try_admit(&mut self, batch: &usize, _: u64, _: u64) -> AdmissionAttempt<usize> {
        self.calls += 1;
        // This synchronous owner performs check and publication without yielding.
        match check_dispatch_admission(self.pending, *batch, self.limit) {
            Ok(()) => {
                self.pending += batch;
                self.published.push(*batch);
                AdmissionAttempt::Started(*batch)
            }
            Err(DispatchAdmissionError::BatchTooLarge) => AdmissionAttempt::Rejected(Fault {
                code: "host.batch-too-large".into(),
                message: "batch exceeds host queue policy".into(),
            }),
            Err(DispatchAdmissionError::Backpressured) => AdmissionAttempt::Backpressured(Fault {
                code: "host.queue-busy".into(),
                message: "host queue has no room".into(),
            }),
        }
    }
}

fn admission(batch: usize) -> Admission<usize> {
    Admission::new(
        batch,
        100,
        1000,
        AdmissionPolicy {
            max_attempts: 3,
            base_delay_ms: 10,
            max_delay_ms: 20,
        },
    )
    .unwrap()
}

#[test]
fn original_admission_lifecycle_retries_only_transient_pressure_without_publication() {
    let mut host = Host {
        pending: 2,
        limit: 2,
        ..Host::default()
    };
    let mut queued = admission(1);
    let AdmissionPoll::Waiting(wait) = queued.poll(&mut host, 100).unwrap() else {
        panic!("expected pressure");
    };
    assert_eq!(wait.retry_at_ms, 110);
    assert_eq!(wait.deadline_at_ms, 1100);
    assert_eq!(wait.attempts, 1);
    assert!(host.published.is_empty());
    assert_eq!(host.pending, 2);
    assert!(matches!(
        queued.poll(&mut host, 109).unwrap(),
        AdmissionPoll::Waiting(_)
    ));
    assert_eq!(host.calls, 1);
    host.pending -= 1;
    assert_eq!(
        queued.poll(&mut host, 110).unwrap(),
        AdmissionPoll::Started(1)
    );
    assert_eq!(host.pending, 2);
    assert_eq!(host.published, [1]);
    assert_eq!(queued.terminal_reason(), Some(AdmissionEnd::Started));
    assert_eq!(
        queued.poll(&mut host, 111).unwrap(),
        AdmissionPoll::Finished
    );
    assert_eq!(host.calls, 2);
}

#[test]
fn oversized_or_cancelled_waits_never_publish_or_retry_after_terminal() {
    let mut host = Host {
        pending: 2,
        limit: 2,
        ..Host::default()
    };
    let mut oversized = admission(3);
    assert!(
        matches!(oversized.poll(&mut host, 100).unwrap(), AdmissionPoll::Rejected(fault)
        if fault.code == "host.batch-too-large")
    );
    assert_eq!(oversized.terminal_reason(), Some(AdmissionEnd::Rejected));
    host.limit = 10;
    assert_eq!(
        oversized.poll(&mut host, 110).unwrap(),
        AdmissionPoll::Finished
    );
    assert_eq!(host.calls, 1);
    host.limit = 2;
    let mut cancelled = admission(1);
    assert!(matches!(
        cancelled.poll(&mut host, 100).unwrap(),
        AdmissionPoll::Waiting(_)
    ));
    assert!(cancelled.cancel());
    host.pending = 0;
    assert_eq!(
        cancelled.poll(&mut host, 110).unwrap(),
        AdmissionPoll::Finished
    );
    assert_eq!(host.calls, 2);
    assert!(host.published.is_empty());
}
