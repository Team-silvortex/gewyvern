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
