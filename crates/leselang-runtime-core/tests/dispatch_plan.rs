use leselang_runtime_core::{
    ClockError, DispatchAttemptError, DispatchLeaseError, DispatchLeaseObservation,
    DispatchLeasePlanError, DispatchLeaseStamp, MAX_CLOCK_MS, next_dispatch_attempt,
    plan_dispatch_lease, validate_dispatch_lease,
};

mod recovery {
    use leselang_runtime_core::{
        CheckpointCommitError, CheckpointRecovery, DispatchRecoveryError, DispatchRecoveryHost,
        DispatchRecoveryLimits, RecoveryPayload, RecoveryRecord, StoredDispatchMetadata,
        inspect_dispatch_recovery,
    };
    use std::{cell::RefCell, panic::AssertUnwindSafe, rc::Rc};

    struct Numbers(Option<StoredDispatchMetadata<()>>);
    impl DispatchRecoveryHost for Numbers {
        type Request = ();
        type Retry = ();
        type Error = ();
        fn admit_request(&mut self) -> Result<(), ()> {
            Ok(())
        }
        fn validate_identity(&mut self, _: &()) -> Result<(), ()> {
            Ok(())
        }
        fn validate_image(&mut self, _: &()) -> Result<(), ()> {
            Ok(())
        }
        fn read_metadata(&mut self) -> Result<StoredDispatchMetadata<()>, ()> {
            Ok(self.0.take().unwrap())
        }
        fn validate_retry_error(&mut self, _: ()) -> Result<(), ()> {
            Ok(())
        }
    }

    fn stage<E>(error: &DispatchRecoveryError<E>) -> u8 {
        match error {
            DispatchRecoveryError::Request(_) => 1,
            DispatchRecoveryError::Identity(_) => 2,
            DispatchRecoveryError::Image(_) => 3,
            DispatchRecoveryError::Metadata(_) => 4,
            DispatchRecoveryError::Attempt => 6,
            DispatchRecoveryError::RetryState => 7,
            DispatchRecoveryError::RetryHistory => 8,
            DispatchRecoveryError::RetryError(_) => 5,
            DispatchRecoveryError::CompletionState => 9,
        }
    }

    #[test]
    fn dispatch_recovery_numeric_and_history_order_matches_boundary_oracle() {
        let numbers = [
            i64::MIN,
            -1,
            0,
            1,
            u32::MAX as i64,
            u32::MAX as i64 + 1,
            i64::MAX,
        ];
        let mut cases = 0;
        for max_attempts in [0, 1, u32::MAX] {
            for max_retries in [0, 1, u32::MAX] {
                for attempt in numbers {
                    for ready_at_ms in [-1, 0, i64::MAX] {
                        for retry_count in numbers {
                            for present in [false, true] {
                                for acknowledged in [false, true] {
                                    for effect_completed in [false, true] {
                                        let expected = if attempt < 0
                                            || attempt as i128 > max_attempts as i128
                                        {
                                            Err(6)
                                        } else if ready_at_ms < 0
                                            || retry_count < 0
                                            || retry_count as i128 > max_retries as i128
                                        {
                                            Err(7)
                                        } else if (retry_count == 0) == present {
                                            Err(8)
                                        } else if acknowledged != effect_completed {
                                            Err(9)
                                        } else {
                                            Ok(())
                                        };
                                        let mut host = Numbers(Some(StoredDispatchMetadata {
                                            attempt,
                                            ready_at_ms,
                                            retry_count,
                                            last_error: present.then_some(()),
                                            acknowledged,
                                            effect_completed,
                                        }));
                                        assert_eq!(
                                            inspect_dispatch_recovery(
                                                &mut host,
                                                DispatchRecoveryLimits {
                                                    max_attempts,
                                                    max_retries
                                                }
                                            )
                                            .map_err(|e| stage(&e)),
                                            expected
                                        );
                                        cases += 1;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        assert_eq!(cases, 10584);
    }

    type Events = Rc<RefCell<Vec<u8>>>;
    struct Owner {
        identity: Box<u8>,
        events: Events,
        panic_drop: bool,
    }
    impl Owner {
        fn new(identity: u8, events: &Events) -> Self {
            Self {
                identity: Box::new(identity),
                events: Rc::clone(events),
                panic_drop: false,
            }
        }
    }
    impl Drop for Owner {
        fn drop(&mut self) {
            self.events.borrow_mut().push(100 + *self.identity);
            assert!(!self.panic_drop, "native owner cleanup");
        }
    }
    struct Host {
        events: Events,
        fail: u8,
        unwind: bool,
        retry: bool,
        completed: bool,
        panic_cleanup: bool,
    }
    impl Host {
        fn new(retry: bool) -> Self {
            Self {
                events: Rc::default(),
                fail: 0,
                unwind: false,
                retry,
                completed: false,
                panic_cleanup: false,
            }
        }
        fn visit(&mut self, at: u8) -> Result<(), Owner> {
            self.events.borrow_mut().push(at);
            if self.fail == at {
                assert!(!self.unwind, "native hook unwind");
                Err(Owner::new(9, &self.events))
            } else {
                Ok(())
            }
        }
    }
    impl DispatchRecoveryHost for Host {
        type Request = Owner;
        type Retry = Owner;
        type Error = Owner;
        fn admit_request(&mut self) -> Result<Owner, Owner> {
            self.visit(1)?;
            let mut owner = Owner::new(1, &self.events);
            owner.panic_drop = self.panic_cleanup;
            Ok(owner)
        }
        fn validate_identity(&mut self, original: &Owner) -> Result<(), Owner> {
            assert_eq!(*original.identity, 1);
            self.visit(2)
        }
        fn validate_image(&mut self, original: &Owner) -> Result<(), Owner> {
            assert_eq!(*original.identity, 1);
            self.visit(3)
        }
        fn read_metadata(&mut self) -> Result<StoredDispatchMetadata<Owner>, Owner> {
            self.visit(4)?;
            Ok(StoredDispatchMetadata {
                attempt: 1,
                ready_at_ms: i64::MAX,
                retry_count: i64::from(self.retry),
                last_error: self.retry.then(|| Owner::new(2, &self.events)),
                acknowledged: false,
                effect_completed: self.completed,
            })
        }
        fn validate_retry_error(&mut self, original: Owner) -> Result<(), Owner> {
            assert_eq!(*original.identity, 2);
            self.visit(5)
        }
    }
    fn limits() -> DispatchRecoveryLimits {
        DispatchRecoveryLimits {
            max_attempts: 1,
            max_retries: 1,
        }
    }
    fn calls(events: &Events) -> Vec<u8> {
        events
            .borrow()
            .iter()
            .copied()
            .filter(|v| *v < 100)
            .collect()
    }

    #[test]
    fn dispatch_recovery_preserves_move_only_local_owners_and_lazy_error_decode() {
        for retry in [false, true] {
            let mut host = Host::new(retry);
            let borrowed: &mut dyn DispatchRecoveryHost<Request = Owner, Retry = Owner, Error = Owner> =
                &mut host;
            assert!(inspect_dispatch_recovery(borrowed, limits()).is_ok());
            assert_eq!(
                calls(&host.events),
                if retry {
                    vec![1, 2, 3, 4, 5]
                } else {
                    vec![1, 2, 3, 4]
                }
            );
            assert_eq!(
                host.events.borrow().iter().filter(|&&v| v == 101).count(),
                1
            );
            assert_eq!(
                host.events.borrow().iter().filter(|&&v| v == 102).count(),
                usize::from(retry)
            );
        }
    }

    #[test]
    fn dispatch_recovery_native_errors_are_owned_redacted_and_stop_at_first_fault() {
        for retry in [false, true] {
            for fail in 1..=if retry { 5 } else { 4 } {
                let mut host = Host::new(retry);
                host.fail = fail;
                let error = inspect_dispatch_recovery(&mut host, limits()).unwrap_err();
                assert_eq!(stage(&error), fail);
                assert_eq!(calls(&host.events), (1..=fail).collect::<Vec<_>>());
                assert!(!host.events.borrow().contains(&109));
                assert_eq!(format!("{error:?}"), error.to_string());
                assert!(std::error::Error::source(&error).is_none());
                let original = match error {
                    DispatchRecoveryError::Request(e)
                    | DispatchRecoveryError::Identity(e)
                    | DispatchRecoveryError::Image(e)
                    | DispatchRecoveryError::Metadata(e)
                    | DispatchRecoveryError::RetryError(e) => e,
                    _ => panic!("not native"),
                };
                assert_eq!(*original.identity, 9);
                drop(original);
                assert_eq!(
                    host.events.borrow().iter().filter(|&&v| v == 109).count(),
                    1
                );
                assert_eq!(
                    host.events.borrow().iter().filter(|&&v| v == 101).count(),
                    usize::from(fail > 1)
                );
                assert_eq!(
                    host.events.borrow().iter().filter(|&&v| v == 102).count(),
                    usize::from(fail == 5)
                );
            }
        }
    }

    #[test]
    fn dispatch_recovery_every_hook_unwind_and_final_cleanup_stop_without_retry() {
        for retry in [false, true] {
            for fail in 1..=if retry { 5 } else { 4 } {
                let mut host = Host::new(retry);
                host.fail = fail;
                host.unwind = true;
                assert!(
                    std::panic::catch_unwind(AssertUnwindSafe(|| inspect_dispatch_recovery(
                        &mut host,
                        limits()
                    )))
                    .is_err()
                );
                assert_eq!(calls(&host.events), (1..=fail).collect::<Vec<_>>());
                assert_eq!(
                    host.events.borrow().iter().filter(|&&v| v == 101).count(),
                    usize::from(fail > 1)
                );
                assert_eq!(
                    host.events.borrow().iter().filter(|&&v| v == 102).count(),
                    usize::from(fail == 5)
                );
            }
        }
        let mut host = Host::new(true);
        host.panic_cleanup = true;
        assert!(
            std::panic::catch_unwind(AssertUnwindSafe(|| inspect_dispatch_recovery(
                &mut host,
                limits()
            )))
            .is_err()
        );
        assert_eq!(&*host.events.borrow(), &[1, 2, 3, 4, 5, 102, 101]);
    }

    #[test]
    fn dispatch_recovery_refusal_or_commit_failure_never_publishes_staged_checkpoint() {
        for mode in 0..4 {
            let mut host = Host::new(true);
            host.completed = mode == 0;
            host.fail = if mode == 1 { 5 } else { 0 };
            let events = Rc::clone(&host.events);
            let mut checkpoint = CheckpointRecovery::new(2, 10);
            checkpoint
                .push(RecoveryRecord {
                    key: 1,
                    sequence: 1,
                    payload: RecoveryPayload::Pending(Owner::new(3, &events)),
                })
                .unwrap();
            checkpoint
                .push(RecoveryRecord {
                    key: 2,
                    sequence: 2,
                    payload: RecoveryPayload::Completed(Owner::new(4, &events)),
                })
                .unwrap();
            let result = checkpoint.finish(host, |host, snapshot| {
                assert_eq!(snapshot.highest_sequence(), 2);
                inspect_dispatch_recovery(host, limits()).map_err(|e| stage(&e))?;
                host.events.borrow_mut().push(6);
                if mode == 2 {
                    return Err(10);
                }
                Ok(())
            });
            if mode == 3 {
                let (snapshot, ()) = result.unwrap();
                assert!(!events.borrow().contains(&103));
                drop(snapshot);
            } else {
                assert!(matches!(result, Err(CheckpointCommitError::Host(_))));
            }
            assert_eq!(events.borrow().iter().filter(|&&v| v == 103).count(), 1);
            assert_eq!(events.borrow().iter().filter(|&&v| v == 104).count(), 1);
            assert_eq!(events.borrow().contains(&6), mode >= 2);
        }
    }
}

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
