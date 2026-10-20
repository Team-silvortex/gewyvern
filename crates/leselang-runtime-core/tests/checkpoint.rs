use std::{
    cell::{Cell, RefCell},
    cmp::Ordering,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};

use leselang_runtime_core::{
    CheckpointCommitError, CheckpointError, CheckpointRecovery, RecoveredCheckpoint,
    RecoveryPayload, RecoveryRecord,
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

mod finalization {
    use super::*;
    use leselang_runtime_core::{CheckpointFinalizationError as Error, CheckpointFinalizer};

    struct Host {
        owner: Native,
        events: Rc<RefCell<Vec<usize>>>,
        committed: Rc<Cell<Option<u64>>>,
        proposed: u64,
        graph: u64,
        fail: usize,
        unwind: usize,
        poison_metadata: bool,
        unwind_after_commit: bool,
    }
    impl Host {
        fn new(drops: &Rc<RefCell<Vec<usize>>>) -> Self {
            Self {
                owner: native(3, drops),
                events: Rc::default(),
                committed: Rc::new(Cell::new(None)),
                proposed: 0,
                graph: 7,
                fail: 0,
                unwind: 0,
                poison_metadata: false,
                unwind_after_commit: false,
            }
        }
        fn check(&mut self, stage: usize) -> Result<(), Native> {
            self.events.borrow_mut().push(stage);
            assert_ne!(self.unwind, stage, "native finalization unwind");
            if self.fail == stage {
                Err(native(99, &self.owner.drops))
            } else {
                Ok(())
            }
        }
    }
    impl CheckpointFinalizer for Host {
        type Commit = Native;
        type Error = Native;
        fn validate_dispatches(&mut self) -> Result<(), Native> {
            self.check(1)
        }
        fn validate_graphs(&mut self) -> Result<u64, Native> {
            self.check(2)?;
            Ok(self.graph)
        }
        fn validate_audits(&mut self) -> Result<(), Native> {
            self.check(3)
        }
        fn advance_watermark(&mut self, highest: u64) -> Result<Native, Native> {
            self.check(4)?;
            self.proposed = highest;
            let mut metadata = native(4, &self.owner.drops);
            metadata.poison = self.poison_metadata;
            Ok(metadata)
        }
        fn commit(&mut self) -> Result<(), Native> {
            self.check(5)?;
            self.committed.set(Some(self.proposed));
            assert!(!self.unwind_after_commit, "commit already happened");
            Ok(())
        }
    }
    fn recovery(drops: &Rc<RefCell<Vec<usize>>>) -> CheckpointRecovery<usize, Native, Native> {
        let mut recovery = CheckpointRecovery::new(2, 100);
        recovery.push(pending(1, 1, native(1, drops))).unwrap();
        recovery
            .push(RecoveryRecord {
                key: 2,
                sequence: 2,
                payload: RecoveryPayload::Completed(native(2, drops)),
            })
            .unwrap();
        recovery
    }
    fn sorted(drops: &Rc<RefCell<Vec<usize>>>) -> Vec<usize> {
        let mut result = drops.borrow().clone();
        result.sort();
        result
    }
    fn stage(error: &Error<Native>) -> usize {
        match error {
            Error::Dispatch(_) => 1,
            Error::Graph(_) => 2,
            Error::Audit(_) => 3,
            Error::Watermark(_) => 4,
            Error::Commit(_) => 5,
        }
    }

    #[test]
    fn ordered_finalization_joins_original_maxima_without_clamping_or_early_handoff() {
        for row in [0, 1, 7, u64::MAX - 1] {
            for graph in [0, 1, 7, u64::MAX - 1, u64::MAX] {
                let drops = Rc::default();
                let mut recovery =
                    CheckpointRecovery::<usize, Native, Native>::new(1, u64::MAX - 1);
                if row != 0 {
                    recovery.push(pending(1, row, native(1, &drops))).unwrap();
                }
                let mut host = Host::new(&drops);
                host.graph = graph;
                let events = host.events.clone();
                let committed = host.committed.clone();
                let (snapshot, metadata) = recovery.finish_recovery(host).unwrap();
                assert_eq!(&*events.borrow(), &[1, 2, 3, 4, 5]);
                assert_eq!(&*drops.borrow(), &[3]);
                assert_eq!(snapshot.highest_sequence(), row);
                assert_eq!(committed.get(), Some(row.max(graph)));
                assert_eq!(metadata.id, 4);
                drop((snapshot, metadata));
                assert_eq!(
                    sorted(&drops),
                    if row == 0 { vec![3, 4] } else { vec![1, 3, 4] }
                );
            }
        }
    }

    #[test]
    fn ordered_finalization_keeps_native_error_ownership_and_exact_refusal_prefix() {
        for fail in 1..=5 {
            let drops = Rc::default();
            let recovery = recovery(&drops);
            let mut host = Host::new(&drops);
            host.fail = fail;
            let events = host.events.clone();
            let committed = host.committed.clone();
            let Err(CheckpointCommitError::Host(error)) = recovery.finish_recovery(host) else {
                panic!()
            };
            assert_eq!(stage(&error), fail);
            assert_eq!(*events.borrow(), (1..=fail).collect::<Vec<_>>());
            assert_eq!(committed.get(), None);
            assert_eq!(format!("{error:?}"), error.to_string());
            assert!(std::error::Error::source(&error).is_none());
            assert_eq!(
                sorted(&drops),
                if fail == 5 {
                    vec![1, 2, 3, 4]
                } else {
                    vec![1, 2, 3]
                }
            );
            let original = match error {
                Error::Dispatch(e)
                | Error::Graph(e)
                | Error::Audit(e)
                | Error::Watermark(e)
                | Error::Commit(e) => e,
            };
            assert_eq!(original.id, 99);
            drop(original);
            assert_eq!(
                sorted(&drops),
                if fail == 5 {
                    vec![1, 2, 3, 4, 99]
                } else {
                    vec![1, 2, 3, 99]
                }
            );
        }
    }

    #[test]
    fn ordered_finalization_each_hook_unwind_discards_staging_without_retry() {
        for unwind in 1..=5 {
            let drops = Rc::default();
            let recovery = recovery(&drops);
            let mut host = Host::new(&drops);
            host.unwind = unwind;
            let events = host.events.clone();
            let committed = host.committed.clone();
            assert!(catch_unwind(AssertUnwindSafe(|| recovery.finish_recovery(host))).is_err());
            assert_eq!(*events.borrow(), (1..=unwind).collect::<Vec<_>>());
            assert_eq!(committed.get(), None);
            assert_eq!(
                sorted(&drops),
                if unwind == 5 {
                    vec![1, 2, 3, 4]
                } else {
                    vec![1, 2, 3]
                }
            );
        }
    }

    #[test]
    fn ordered_finalization_cleanup_unwind_keeps_snapshot_metadata_and_error_local() {
        for mode in 0..4 {
            let drops = Rc::default();
            let mut recovery = CheckpointRecovery::<usize, Native, Native>::new(1, 10);
            let mut value = native(1, &drops);
            value.poison = mode == 3;
            recovery.push(pending(1, 1, value)).unwrap();
            let mut host = Host::new(&drops);
            host.owner.poison = mode < 2;
            host.fail = match mode {
                1 | 3 => 4,
                2 => 5,
                _ => 0,
            };
            host.poison_metadata = mode == 2;
            let committed = host.committed.clone();
            assert!(catch_unwind(AssertUnwindSafe(|| recovery.finish_recovery(host))).is_err());
            assert_eq!(committed.get().is_some(), mode == 0);
            assert_eq!(
                sorted(&drops),
                match mode {
                    0 => vec![1, 3, 4],
                    2 => vec![1, 3, 4, 99],
                    _ => vec![1, 3, 99],
                }
            );
        }
    }

    #[test]
    fn ordered_finalization_commit_then_unwind_is_uncertain_not_replay_permission() {
        let drops = Rc::default();
        let recovery = recovery(&drops);
        let mut host = Host::new(&drops);
        host.unwind_after_commit = true;
        let committed = host.committed.clone();
        let events = host.events.clone();
        assert!(catch_unwind(AssertUnwindSafe(|| recovery.finish_recovery(host))).is_err());
        assert_eq!(committed.get(), Some(7));
        assert_eq!(&*events.borrow(), &[1, 2, 3, 4, 5]);
        assert_eq!(sorted(&drops), [1, 2, 3, 4]);
    }

    #[test]
    fn ordered_finalization_of_closed_staging_releases_host_without_any_hook() {
        let drops = Rc::default();
        let mut recovery = recovery(&drops);
        assert_eq!(
            recovery.push(pending(3, 3, native(5, &drops))),
            Err(CheckpointError::RecordLimit)
        );
        let host = Host::new(&drops);
        let events = host.events.clone();
        assert!(matches!(
            recovery.finish_recovery(host),
            Err(CheckpointCommitError::Closed)
        ));
        assert!(events.borrow().is_empty());
        assert_eq!(sorted(&drops), [1, 2, 3, 5]);
    }
}

mod record_pipeline {
    use super::*;
    use leselang_runtime_core::{
        RecordRecoveryError as Error, RecordRecoveryHost, StoredRecordState as State,
        inspect_record_recovery,
    };

    struct Image {
        key: Native,
        payload: Native,
    }
    struct Host {
        image: Option<Image>,
        receipt: Option<Native>,
        state: State,
        events: Vec<&'static str>,
        fail: Option<&'static str>,
        panic_at: usize,
        drops: Rc<RefCell<Vec<usize>>>,
    }
    impl Host {
        fn new(state: State, drops: &Rc<RefCell<Vec<usize>>>) -> Self {
            Self {
                image: Some(Image {
                    key: native(1, drops),
                    payload: native(2, drops),
                }),
                receipt: Some(native(3, drops)),
                state,
                events: vec![],
                fail: None,
                panic_at: 0,
                drops: drops.clone(),
            }
        }
        fn observe(&mut self, event: &'static str) {
            self.events.push(event);
            assert_ne!(self.events.len(), self.panic_at, "native record unwind");
        }
        fn check(&mut self, event: &'static str) -> Result<(), Native> {
            self.observe(event);
            if self.fail == Some(event) {
                Err(native(99, &self.drops))
            } else {
                Ok(())
            }
        }
    }
    impl RecordRecoveryHost for Host {
        type Key = Native;
        type Image = Image;
        type Pending = Native;
        type Completed = Native;
        type Error = Native;
        fn admit_image(&mut self) -> Result<Image, Native> {
            self.check("image")?;
            Ok(self.image.take().unwrap())
        }
        fn sequence(&mut self, _: &Image) -> Result<u64, Native> {
            self.check("sequence")?;
            Ok(7)
        }
        fn validate_identity(&mut self, image: &Image) -> Result<(), Native> {
            assert_eq!(image.key.id, 1);
            self.check("identity")
        }
        fn validate_deadline(&mut self, _: &Image) -> Result<(), Native> {
            self.check("deadline")
        }
        fn validate_owner(&mut self, _: &Image) -> Result<(), Native> {
            self.check("owner")
        }
        fn stored_state(&mut self) -> Result<State, Native> {
            self.check("state")?;
            Ok(self.state)
        }
        fn admit_completed(&mut self) -> Result<Native, Native> {
            self.check("receipt")?;
            Ok(self.receipt.take().unwrap())
        }
        fn validate_successor(
            &mut self,
            _: &Image,
            receipt: Option<&Native>,
        ) -> Result<(), Native> {
            assert_eq!(receipt.is_some(), self.state == State::Completed);
            self.check("successor")
        }
        fn pending_payload(&mut self, image: Image) -> (Native, Native) {
            self.observe("pending");
            (image.key, image.payload)
        }
        fn completed_payload(&mut self, image: Image, receipt: Native) -> (Native, Native) {
            self.observe("completed");
            let Image { key, payload } = image;
            drop(payload);
            (key, receipt)
        }
    }
    fn events(state: State) -> Vec<&'static str> {
        let mut steps = vec![
            "image", "sequence", "identity", "deadline", "owner", "state",
        ];
        if state == State::Completed {
            steps.push("receipt");
        }
        steps.extend([
            "successor",
            if state == State::Pending {
                "pending"
            } else {
                "completed"
            },
        ]);
        steps
    }
    fn sorted(drops: &Rc<RefCell<Vec<usize>>>) -> Vec<usize> {
        let mut result = drops.borrow().clone();
        result.sort();
        result
    }
    #[test]
    fn stored_record_moves_original_noncomparable_local_owners_and_defers_pending_receipts() {
        for state in [State::Pending, State::Completed] {
            let drops = Rc::new(RefCell::new(vec![]));
            let mut host = Host::new(state, &drops);
            // A pending row must not invoke a rejecting unused receipt decoder.
            host.fail = Some("receipt");
            if state == State::Completed {
                host.fail = None;
            }
            let erased: &mut dyn RecordRecoveryHost<
                Key = Native,
                Image = Image,
                Pending = Native,
                Completed = Native,
                Error = Native,
            > = &mut host;
            let record = inspect_record_recovery(erased).unwrap();
            assert_eq!(record.sequence, 7);
            assert_eq!(record.key.id, 1);
            assert_eq!(host.events, events(state));
            match record.payload {
                RecoveryPayload::Pending(value) => {
                    assert_eq!(value.id, 2);
                    assert!(drops.borrow().is_empty());
                }
                RecoveryPayload::Completed(value) => {
                    assert_eq!(value.id, 3);
                    assert_eq!(*drops.borrow(), [2]);
                }
            }
            drop(record.key);
            drop(host);
            assert_eq!(sorted(&drops), [1, 2, 3]);
        }
    }
    #[test]
    fn stored_record_errors_keep_original_ownership_and_exact_native_stage() {
        for state in [State::Pending, State::Completed] {
            let steps = events(state);
            for (index, stage) in steps[..steps.len() - 1].iter().enumerate() {
                let drops = Rc::new(RefCell::new(vec![]));
                let mut host = Host::new(state, &drops);
                host.fail = Some(stage);
                let error = match inspect_record_recovery(&mut host) {
                    Err(error) => error,
                    Ok(_) => panic!("missing refusal"),
                };
                assert!(std::error::Error::source(&error).is_none());
                assert!(format!("{error:?} {error}").contains("stored effect"));
                let native = match error {
                    Error::Image(e) if *stage == "image" => e,
                    Error::Sequence(e) if *stage == "sequence" => e,
                    Error::Identity(e) if *stage == "identity" => e,
                    Error::Deadline(e) if *stage == "deadline" => e,
                    Error::Owner(e) if *stage == "owner" => e,
                    Error::State(e) if *stage == "state" => e,
                    Error::Receipt(e) if *stage == "receipt" => e,
                    Error::Successor(e) if *stage == "successor" => e,
                    _ => panic!("reordered native refusal"),
                };
                assert_eq!(host.events, steps[..=index]);
                assert_eq!(native.id, 99);
                assert!(Rc::ptr_eq(&native.drops, &drops));
                drop((host, native));
                assert_eq!(sorted(&drops), [1, 2, 3, 99]);
            }
        }
    }
    #[test]
    fn stored_record_every_native_unwind_and_late_cleanup_panic_prevent_payload_handoff() {
        for state in [State::Pending, State::Completed] {
            for panic_at in 1..=events(state).len() {
                let drops = Rc::new(RefCell::new(vec![]));
                let mut host = Host::new(state, &drops);
                host.panic_at = panic_at;
                assert!(
                    catch_unwind(AssertUnwindSafe(|| inspect_record_recovery(&mut host))).is_err()
                );
                assert_eq!(host.events, events(state)[..panic_at]);
                drop(host);
                assert_eq!(sorted(&drops), [1, 2, 3]);
            }
        }
        let drops = Rc::new(RefCell::new(vec![]));
        let mut host = Host::new(State::Completed, &drops);
        host.image.as_mut().unwrap().payload.poison = true;
        assert!(catch_unwind(AssertUnwindSafe(|| inspect_record_recovery(&mut host))).is_err());
        drop(host);
        assert_eq!(sorted(&drops), [1, 2, 3]);
    }
    #[test]
    fn stored_record_late_refusal_or_commit_failure_never_publishes_checkpoint_prefix() {
        for state in [State::Pending, State::Completed] {
            for failure in [None, Some("successor")] {
                for fail_commit in [false, true] {
                    let drops = Rc::new(RefCell::new(vec![]));
                    let mut host = Host::new(state, &drops);
                    host.fail = failure;
                    let mut recovery =
                        CheckpointRecovery::<usize, (Native, Native), (Native, Native)>::new(
                            2, 100,
                        );
                    recovery
                        .push(RecoveryRecord {
                            key: 4,
                            sequence: 1,
                            payload: RecoveryPayload::Pending((
                                native(4, &drops),
                                native(5, &drops),
                            )),
                        })
                        .unwrap();
                    let committed = Cell::new(false);
                    let result = (|| {
                        let record = inspect_record_recovery(&mut host)?;
                        let key = record.key;
                        let payload = match record.payload {
                            RecoveryPayload::Pending(value) => {
                                RecoveryPayload::Pending((key, value))
                            }
                            RecoveryPayload::Completed(value) => {
                                RecoveryPayload::Completed((key, value))
                            }
                        };
                        recovery
                            .push(RecoveryRecord {
                                key: 1,
                                sequence: record.sequence,
                                payload,
                            })
                            .unwrap();
                        recovery
                            .finish((), |_, _| {
                                if fail_commit {
                                    return Err(native(99, &drops));
                                }
                                committed.set(true);
                                Ok(())
                            })
                            .map_err(|error| match error {
                                CheckpointCommitError::Host(error) => Error::Successor(error),
                                _ => panic!("unexpected closed checkpoint"),
                            })
                    })();
                    assert_eq!(result.is_ok(), failure.is_none() && !fail_commit);
                    assert_eq!(committed.get(), result.is_ok());
                    drop((result, host));
                    let expected = if failure.is_some() || fail_commit {
                        vec![1, 2, 3, 4, 5, 99]
                    } else {
                        vec![1, 2, 3, 4, 5]
                    };
                    assert_eq!(sorted(&drops), expected);
                }
            }
        }
    }
}

struct Native {
    id: usize,
    poison: bool,
    drops: Rc<RefCell<Vec<usize>>>,
}
impl Drop for Native {
    fn drop(&mut self) {
        self.drops.borrow_mut().push(self.id);
        assert!(!self.poison, "native cleanup unwind");
    }
}
fn native(id: usize, drops: &Rc<RefCell<Vec<usize>>>) -> Native {
    Native {
        id,
        poison: false,
        drops: drops.clone(),
    }
}
fn poisoned(id: usize, drops: &Rc<RefCell<Vec<usize>>>) -> Native {
    let mut value = native(id, drops);
    value.poison = true;
    value
}
fn pending<Key>(key: Key, sequence: u64, value: Native) -> RecoveryRecord<Key, Native, Native> {
    RecoveryRecord {
        key,
        sequence,
        payload: RecoveryPayload::Pending(value),
    }
}

#[test]
fn native_snapshot_moves_original_thread_local_owners_after_host_cleanup() {
    let drops = Rc::new(RefCell::new(vec![]));
    let mut recovery = CheckpointRecovery::new(3, 100);
    assert!(recovery.push(pending("z", 7, native(1, &drops))).is_ok());
    assert!(
        recovery
            .push(RecoveryRecord {
                key: "a",
                sequence: 2,
                payload: RecoveryPayload::Completed(native(2, &drops)),
            })
            .is_ok()
    );
    let (snapshot, commit) = recovery
        .finish(native(3, &drops), |host, snapshot| {
            assert_eq!(host.id, 3);
            assert_eq!(snapshot.pending()["z"].id, 1);
            assert_eq!(snapshot.completed()["a"].id, 2);
            assert_eq!(snapshot.highest_sequence(), 7);
            Ok::<_, Native>(native(4, &drops))
        })
        .unwrap_or_else(|_| panic!("unexpected commit error"));
    assert_eq!(*drops.borrow(), [3]);
    let (pending, completed) = snapshot.into_parts();
    assert_eq!(pending["z"].id, 1);
    assert_eq!(completed["a"].id, 2);
    drop((pending, completed, commit));
    assert_eq!(*drops.borrow(), [3, 1, 2, 4]);
}

#[test]
fn duplicate_keys_in_either_partition_poison_the_entire_pass_without_overwrite() {
    for completed in [false, true] {
        let drops = Rc::new(RefCell::new(vec![]));
        let mut recovery = CheckpointRecovery::new(4, 100);
        assert!(recovery.push(pending("same", 1, native(1, &drops))).is_ok());
        let payload = if completed {
            RecoveryPayload::Completed(native(2, &drops))
        } else {
            RecoveryPayload::Pending(native(2, &drops))
        };
        assert_eq!(
            recovery.push(RecoveryRecord {
                key: "same",
                sequence: 2,
                payload,
            }),
            Err(CheckpointError::DuplicateKey)
        );
        assert_eq!(*drops.borrow(), [2, 1]);
        assert_eq!(
            recovery.push(pending("fresh", 3, native(3, &drops))),
            Err(CheckpointError::Closed)
        );
        let called = Cell::new(false);
        let output = recovery.finish((), |_, _| {
            called.set(true);
            Ok::<_, ()>(())
        });
        assert!(matches!(output, Err(CheckpointCommitError::Closed)));
        assert!(!called.get());
        assert_eq!(*drops.borrow(), [2, 1, 3]);
    }
}

#[test]
fn inclusive_limits_and_sequence_ceilings_are_not_repaired_or_wrapped() {
    for (limit, ceiling, sequence, expected) in [
        (0, 0, 0, CheckpointError::RecordLimit),
        (1, 0, 1, CheckpointError::InvalidSequence),
        (1, 9, 0, CheckpointError::InvalidSequence),
        (1, 9, 10, CheckpointError::InvalidSequence),
        (1, u64::MAX, u64::MAX, CheckpointError::InvalidSequence),
    ] {
        let drops = Rc::new(RefCell::new(vec![]));
        let mut recovery = CheckpointRecovery::new(limit, ceiling);
        assert_eq!(
            recovery.push(pending("a", sequence, native(1, &drops))),
            Err(expected)
        );
        assert_eq!(*drops.borrow(), [1]);
    }
    let mut empty = CheckpointRecovery::<(), (), ()>::new(0, 0);
    let (snapshot, ()) = empty
        .finish((), |_, snapshot| {
            assert!(snapshot.pending().is_empty());
            assert_eq!(snapshot.highest_sequence(), 0);
            Ok::<_, ()>(())
        })
        .unwrap_or_else(|_| panic!());
    assert!(snapshot.completed().is_empty());
    empty = CheckpointRecovery::new(1, u64::MAX);
    assert!(
        empty
            .push(RecoveryRecord {
                key: (),
                sequence: u64::MAX - 1,
                payload: RecoveryPayload::Pending(())
            })
            .is_ok()
    );
    assert!(empty.finish((), |_, _| Ok::<_, ()>(())).is_ok());
}

#[test]
fn final_host_refusal_preserves_opaque_error_after_discarding_staged_data() {
    let drops = Rc::new(RefCell::new(vec![]));
    let mut recovery = CheckpointRecovery::new(1, 1);
    assert!(recovery.push(pending("a", 1, native(1, &drops))).is_ok());
    let output = recovery.finish(native(2, &drops), |_, _| {
        Err::<Native, _>(native(3, &drops))
    });
    assert_eq!(*drops.borrow(), [2, 1]);
    let Err(CheckpointCommitError::Host(error)) = output else {
        panic!()
    };
    assert_eq!(error.id, 3);
    drop(error);
    assert_eq!(*drops.borrow(), [2, 1, 3]);
}

#[test]
fn host_cleanup_unwind_drops_unpublished_snapshot_and_commit_or_error() {
    for success in [false, true] {
        let drops = Rc::new(RefCell::new(vec![]));
        let mut recovery = CheckpointRecovery::new(1, 1);
        assert!(recovery.push(pending("a", 1, native(1, &drops))).is_ok());
        let output = catch_unwind(AssertUnwindSafe(|| {
            recovery.finish(poisoned(2, &drops), |_, _| {
                if success {
                    Ok(native(3, &drops))
                } else {
                    Err(native(3, &drops))
                }
            })
        }));
        assert!(output.is_err());
        let mut observed = drops.borrow().clone();
        observed.sort();
        assert_eq!(observed, [1, 2, 3]);
    }
}

#[test]
fn staged_cleanup_unwind_does_not_leak_the_original_host_error() {
    let drops = Rc::new(RefCell::new(vec![]));
    let mut recovery = CheckpointRecovery::new(1, 1);
    assert!(recovery.push(pending("a", 1, poisoned(1, &drops))).is_ok());
    let output = catch_unwind(AssertUnwindSafe(|| {
        recovery.finish(native(2, &drops), |_, _| Err::<(), _>(native(3, &drops)))
    }));
    assert!(output.is_err());
    assert_eq!(*drops.borrow(), [2, 1, 3]);
}

#[test]
fn final_callback_unwind_releases_staging_and_host_without_handoff() {
    let drops = Rc::new(RefCell::new(vec![]));
    let mut recovery = CheckpointRecovery::new(1, 1);
    assert!(recovery.push(pending("a", 1, native(1, &drops))).is_ok());
    let output = catch_unwind(AssertUnwindSafe(|| {
        recovery.finish(native(2, &drops), |_, _| -> Result<(), ()> {
            panic!("host commit uncertainty")
        })
    }));
    assert!(output.is_err());
    let mut observed = drops.borrow().clone();
    observed.sort();
    assert_eq!(observed, [1, 2]);
}

#[test]
fn closed_pass_releases_uncalled_callback_captures_and_never_calls_host() {
    let drops = Rc::new(RefCell::new(vec![]));
    let mut recovery = CheckpointRecovery::new(0, 0);
    assert_eq!(
        recovery.push(pending("a", 1, native(1, &drops))),
        Err(CheckpointError::RecordLimit)
    );
    let capture = poisoned(2, &drops);
    let called = Cell::new(false);
    let output = catch_unwind(AssertUnwindSafe(|| {
        recovery.finish(native(3, &drops), |_, _| {
            called.set(true);
            drop(capture);
            Ok::<_, ()>(())
        })
    }));
    assert!(output.is_err());
    assert!(!called.get());
    let mut observed = drops.borrow().clone();
    observed.sort();
    assert_eq!(observed, [1, 2, 3]);
}

struct Key {
    value: u32,
    compare_poison: Rc<Cell<bool>>,
}
impl PartialEq for Key {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}
impl Eq for Key {}
impl PartialOrd for Key {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Key {
    fn cmp(&self, other: &Self) -> Ordering {
        assert!(!self.compare_poison.get(), "native key comparison unwind");
        self.value.cmp(&other.value)
    }
}

#[test]
fn native_key_unwind_seals_the_pass_but_cold_limits_run_before_comparison() {
    for limit in [1, 2] {
        let drops = Rc::new(RefCell::new(vec![]));
        let poison = Rc::new(Cell::new(false));
        let mut recovery = CheckpointRecovery::new(limit, 10);
        assert!(
            recovery
                .push(pending(
                    Key {
                        value: 1,
                        compare_poison: poison.clone()
                    },
                    1,
                    native(1, &drops)
                ))
                .is_ok()
        );
        poison.set(true);
        let output = catch_unwind(AssertUnwindSafe(|| {
            recovery.push(pending(
                Key {
                    value: 2,
                    compare_poison: poison.clone(),
                },
                2,
                native(2, &drops),
            ))
        }));
        if limit == 1 {
            assert_eq!(output.unwrap(), Err(CheckpointError::RecordLimit));
        } else {
            assert!(output.is_err());
        }
        assert!(matches!(
            recovery.finish((), |_, _| Ok::<_, ()>(())),
            Err(CheckpointCommitError::Closed)
        ));
        let mut observed = drops.borrow().clone();
        observed.sort();
        assert_eq!(observed, [1, 2]);
    }
}

#[test]
fn error_debug_never_invokes_native_formatters() {
    let drops = Rc::new(RefCell::new(vec![]));
    let error = CheckpointCommitError::Host(native(1, &drops));
    assert_eq!(format!("{error:?}"), "Host");
    drop(error);
    assert_eq!(*drops.borrow(), [1]);
}

#[test]
fn rejected_row_or_staged_destructor_unwind_keeps_the_pass_closed() {
    for poison_incoming in [false, true] {
        let drops = Rc::new(RefCell::new(vec![]));
        let mut recovery = CheckpointRecovery::new(2, 10);
        let first = if poison_incoming {
            native(1, &drops)
        } else {
            poisoned(1, &drops)
        };
        assert!(recovery.push(pending("a", 1, first)).is_ok());
        let second = if poison_incoming {
            poisoned(2, &drops)
        } else {
            native(2, &drops)
        };
        assert!(catch_unwind(AssertUnwindSafe(|| recovery.push(pending("b", 0, second)))).is_err());
        assert!(matches!(
            recovery.finish((), |_, _| Ok::<_, ()>(())),
            Err(CheckpointCommitError::Closed)
        ));
        let mut observed = drops.borrow().clone();
        observed.sort();
        assert_eq!(observed, [1, 2]);
    }
}

#[derive(Deserialize, Serialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
enum StoredRecord<Key, Pending, Completed> {
    Pending {
        key: Key,
        sequence: u64,
        frame: Pending,
    },
    Completed {
        key: Key,
        sequence: u64,
        receipt: Completed,
    },
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct StoredCheckpoint<Key, Pending, Completed> {
    version: u32,
    records: Vec<StoredRecord<Key, Pending, Completed>>,
}
struct Store {
    bytes: Vec<u8>,
    watermark: u64,
    commits: usize,
    reject_commit: bool,
}

// A consumer-owned byte format and commit policy, not a core storage backend.
fn recover_store<
    Key: Ord + DeserializeOwned,
    Pending: DeserializeOwned,
    Completed: DeserializeOwned,
>(
    store: &mut Store,
) -> Result<RecoveredCheckpoint<Key, Pending, Completed>, &'static str> {
    if store.bytes.len() > 4096 {
        return Err("bytes");
    }
    let stored: StoredCheckpoint<Key, Pending, Completed> =
        serde_json::from_slice(&store.bytes).map_err(|_| "decode")?;
    if stored.version != 1 {
        return Err("version");
    }
    let mut recovery = CheckpointRecovery::new(8, 1000);
    for record in stored.records {
        let record = match record {
            StoredRecord::Pending {
                key,
                sequence,
                frame,
            } => RecoveryRecord {
                key,
                sequence,
                payload: RecoveryPayload::Pending(frame),
            },
            StoredRecord::Completed {
                key,
                sequence,
                receipt,
            } => RecoveryRecord {
                key,
                sequence,
                payload: RecoveryPayload::Completed(receipt),
            },
        };
        recovery.push(record).map_err(|_| "row")?;
    }
    let (snapshot, ()) = recovery
        .finish(store, |store, snapshot| {
            if store.reject_commit {
                return Err("commit");
            }
            store.watermark = store.watermark.max(snapshot.highest_sequence() + 1);
            store.commits += 1;
            Ok(())
        })
        .map_err(|error| match error {
            CheckpointCommitError::Closed => "closed",
            CheckpointCommitError::Host(error) => error,
        })?;
    Ok(snapshot)
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct CanvasFrame {
    target: String,
    original_fuel: u64,
    revision: u64,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct CanvasReceipt {
    focused: String,
    remaining_fuel: u64,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct InstrumentFrame {
    channel: u8,
    remaining_samples: u32,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct InstrumentReceipt {
    reading: i64,
}

#[test]
fn unrelated_persisted_schemas_restore_original_budgets_and_never_dispatch_completed_rows() {
    let canvas = StoredCheckpoint {
        version: 1,
        records: vec![
            StoredRecord::Completed {
                key: "focus-a",
                sequence: 9,
                receipt: CanvasReceipt {
                    focused: "first".into(),
                    remaining_fuel: 37,
                },
            },
            StoredRecord::Pending {
                key: "focus-b",
                sequence: 10,
                frame: CanvasFrame {
                    target: "second".into(),
                    original_fuel: 36,
                    revision: 7,
                },
            },
        ],
    };
    let mut store = Store {
        bytes: serde_json::to_vec(&canvas).unwrap(),
        watermark: 1,
        commits: 0,
        reject_commit: false,
    };
    let original = store.bytes.clone();
    let mut dispatches = vec![];
    for _ in 0..2 {
        let recovered = recover_store::<String, CanvasFrame, CanvasReceipt>(&mut store)
            .unwrap_or_else(|_| panic!());
        assert_eq!(recovered.pending()["focus-b"].original_fuel, 36);
        assert_eq!(recovered.pending()["focus-b"].revision, 7);
        assert_eq!(recovered.completed()["focus-a"].focused, "first");
        assert_eq!(recovered.completed()["focus-a"].remaining_fuel, 37);
        // Live authority is host policy; stale recovery data cannot bypass it.
        let live_revision = 8;
        for frame in recovered.pending().values() {
            if frame.revision == live_revision {
                dispatches.push(frame.target.clone());
            }
        }
        assert!(dispatches.is_empty());
    }
    assert_eq!(store.watermark, 11);
    assert_eq!(store.bytes, original);

    let instrument = StoredCheckpoint {
        version: 1,
        records: vec![
            StoredRecord::Pending {
                key: 42u16,
                sequence: 12,
                frame: InstrumentFrame {
                    channel: 3,
                    remaining_samples: 2,
                },
            },
            StoredRecord::Completed {
                key: 41u16,
                sequence: 11,
                receipt: InstrumentReceipt { reading: -7 },
            },
        ],
    };
    let mut store = Store {
        bytes: serde_json::to_vec(&instrument).unwrap(),
        watermark: 90,
        commits: 0,
        reject_commit: false,
    };
    for _ in 0..2 {
        let recovered = recover_store::<u16, InstrumentFrame, InstrumentReceipt>(&mut store)
            .unwrap_or_else(|_| panic!());
        assert_eq!(recovered.pending()[&42].channel, 3);
        assert_eq!(recovered.pending()[&42].remaining_samples, 2);
        assert_eq!(recovered.completed()[&41].reading, -7);
        assert_eq!(
            recovered.pending().keys().copied().collect::<Vec<_>>(),
            [42]
        );
        assert!(!recovered.pending().contains_key(&41));
    }
    assert_eq!(store.watermark, 90);
    assert_eq!(store.commits, 2);
}

#[test]
fn consumer_decode_version_duplicate_and_commit_failures_publish_no_partial_snapshot() {
    for (bytes, reject, expected) in [
        (br#"{"version":2,"records":[]}"#.to_vec(), false, "version"),
        (br#"{"version":1,"records":[],"unknown":true}"#.to_vec(), false, "decode"),
        (br#"{"version":1,"records":[{"state":"pending","key":1,"sequence":1,"frame":null},{"state":"completed","key":1,"sequence":2,"receipt":null}]}"#.to_vec(), false, "row"),
        (br#"{"version":1,"records":[]}"#.to_vec(), true, "commit"),
    ] {
        let original = bytes.clone();
        let mut store = Store { bytes, watermark: 90, commits: 0, reject_commit: reject };
        assert!(matches!(recover_store::<u16, (), ()>(&mut store), Err(error) if error == expected));
        assert_eq!(store.watermark, 90);
        assert_eq!(store.commits, 0);
        assert_eq!(store.bytes, original);
    }
}
