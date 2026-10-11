use std::{
    cell::{Cell, RefCell},
    convert::Infallible,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};

use leselang_runtime_core::{
    CheckpointCommitError, CheckpointRecovery, GroupReceiptError, RecoveryPayload, RecoveryRecord,
    ReservationError, ReservationObservation, ReservationProofError, SequenceReservations,
    StoredGroupState, validate_group_receipt, validate_reservation_proof,
};

#[test]
fn complete_cross_group_index_preserves_gaps_full_width_and_inclusive_limits() {
    let mut reservations = SequenceReservations::new(3, u64::MAX);
    for sequence in [19, 2, u64::MAX - 1] {
        reservations.insert(sequence).unwrap();
    }
    let summary = reservations.finish().unwrap();
    assert_eq!(summary.count, 3);
    assert_eq!(summary.highest_sequence, u64::MAX - 1);
    let empty = SequenceReservations::new(0, 0).finish().unwrap();
    assert_eq!(empty.count, 0);
    assert_eq!(empty.highest_sequence, 0);
}

#[test]
fn duplicate_reservation_closes_the_entire_pass_without_overwrite_or_partial_summary() {
    let mut reservations = SequenceReservations::new(3, 100);
    reservations.insert(7).unwrap();
    reservations.insert(9).unwrap();
    assert_eq!(reservations.insert(7), Err(ReservationError::Duplicate));
    assert_eq!(reservations.insert(10), Err(ReservationError::Closed));
    assert_eq!(reservations.finish(), Err(ReservationError::Closed));
}

#[test]
fn count_sequence_and_duplicate_precedence_never_refill_repair_or_wrap() {
    for (count, ceiling, sequence, expected) in [
        (0, 0, 0, ReservationError::Limit),
        (1, 0, 1, ReservationError::Sequence),
        (1, 9, 10, ReservationError::Sequence),
        (1, 100, 0, ReservationError::Sequence),
        (1, u64::MAX, u64::MAX, ReservationError::Sequence),
    ] {
        let mut reservations = SequenceReservations::new(count, ceiling);
        assert_eq!(reservations.insert(sequence), Err(expected));
        assert_eq!(reservations.insert(1), Err(ReservationError::Closed));
        assert_eq!(reservations.finish(), Err(ReservationError::Closed));
    }
    let mut full = SequenceReservations::new(1, 10);
    full.insert(1).unwrap();
    assert_eq!(full.insert(1), Err(ReservationError::Limit));
    assert_eq!(full.finish(), Err(ReservationError::Closed));
}

#[test]
fn original_watermark_presence_and_alias_match_the_frozen_ordered_truth_table() {
    for watermark in [None, Some(0), Some(1), Some(2), Some(100), Some(u64::MAX)] {
        for sequence in [1, 2, 99, u64::MAX - 1] {
            for expected_record in [false, true] {
                for record_exists in [false, true] {
                    for aliases_owner in [false, true] {
                        let original = if watermark.is_none_or(|w| w == 0 || sequence >= w) {
                            Err(ReservationProofError::Watermark)
                        } else if record_exists != expected_record {
                            Err(ReservationProofError::Presence)
                        } else if aliases_owner {
                            Err(ReservationProofError::Alias)
                        } else {
                            Ok(())
                        };
                        assert_eq!(
                            validate_reservation_proof(
                                sequence,
                                expected_record,
                                ReservationObservation {
                                    allocation_watermark: watermark,
                                    record_exists,
                                    aliases_owner
                                }
                            ),
                            original
                        );
                    }
                }
            }
        }
    }
    assert_eq!(
        validate_reservation_proof(
            0,
            true,
            ReservationObservation {
                allocation_watermark: None,
                record_exists: false,
                aliases_owner: true,
            }
        ),
        Err(ReservationProofError::Sequence)
    );
}

#[test]
fn imported_high_identity_cannot_launder_a_cold_reservation_or_repair_original_watermark() {
    let metadata = Cell::new(40);
    let mut recovery = CheckpointRecovery::<u64, (), ()>::new(1, 1000);
    recovery
        .push(RecoveryRecord {
            key: 100,
            sequence: 100,
            payload: RecoveryPayload::Pending(()),
        })
        .unwrap();
    let result = recovery.finish((), |_, snapshot| {
        validate_reservation_proof(
            50,
            false,
            ReservationObservation {
                allocation_watermark: Some(metadata.get()),
                record_exists: false,
                aliases_owner: false,
            },
        )?;
        metadata.set(snapshot.highest_sequence() + 1);
        Ok(())
    });
    assert!(matches!(
        result,
        Err(CheckpointCommitError::Host(
            ReservationProofError::Watermark
        ))
    ));
    assert_eq!(metadata.get(), 40);
}

struct Native {
    id: usize,
    original: Box<[u8]>,
    drops: Rc<RefCell<Vec<usize>>>,
}
impl Drop for Native {
    fn drop(&mut self) {
        self.drops.borrow_mut().push(self.id);
    }
}
fn native(id: usize, drops: &Rc<RefCell<Vec<usize>>>) -> Native {
    Native {
        id,
        original: b"private-host-receipt".to_vec().into_boxed_slice(),
        drops: drops.clone(),
    }
}

#[test]
fn complete_member_state_and_receipt_presence_gate_the_once_only_native_hook() {
    for state in [StoredGroupState::Pending, StoredGroupState::Completed] {
        for all_completed in [false, true] {
            for present in [false, true] {
                let drops = Rc::new(RefCell::new(Vec::new()));
                let input = present.then(|| native(1, &drops));
                let called = Cell::new(0);
                let result = validate_group_receipt(state, all_completed, input, |input| {
                    called.set(called.get() + 1);
                    Ok::<_, Infallible>(input)
                });
                match (state, all_completed, present) {
                    (StoredGroupState::Pending, false, false) => {
                        assert!(matches!(result, Ok(None)));
                        assert_eq!(called.get(), 0);
                    }
                    (StoredGroupState::Completed, true, true) => {
                        assert!(matches!(&result, Ok(Some(receipt)) if receipt.id == 1));
                        assert_eq!(called.get(), 1);
                        assert!(drops.borrow().is_empty());
                    }
                    _ => {
                        assert!(matches!(result, Err(GroupReceiptError::State)));
                        assert_eq!(called.get(), 0);
                        assert_eq!(drops.borrow().len(), usize::from(present));
                    }
                }
                drop(result);
                assert_eq!(drops.borrow().len(), usize::from(present));
            }
        }
    }
}

#[test]
fn admitted_receipt_keeps_original_move_only_gui_local_payload_and_buffer() {
    let drops = Rc::new(RefCell::new(Vec::new()));
    let input = native(1, &drops);
    let pointer = input.original.as_ptr();
    let receipt = validate_group_receipt(StoredGroupState::Completed, true, Some(input), |input| {
        Ok::<_, Native>(input)
    })
    .unwrap()
    .unwrap();
    assert_eq!(receipt.original.as_ptr(), pointer);
    assert!(drops.borrow().is_empty());
    drop(receipt);
    assert_eq!(*drops.borrow(), [1]);
}

#[test]
fn native_receipt_refusal_moves_exact_error_without_formatting_or_source_leaks() {
    let drops = Rc::new(RefCell::new(Vec::new()));
    let error = native(2, &drops);
    let pointer = error.original.as_ptr();
    let result = validate_group_receipt(
        StoredGroupState::Completed,
        true,
        Some(native(1, &drops)),
        move |input| {
            drop(input);
            Err::<Native, _>(error)
        },
    )
    .err()
    .unwrap();
    assert_eq!(*drops.borrow(), [1]);
    assert!(!format!("{result:?}: {result}").contains("private-host-receipt"));
    assert!(std::error::Error::source(&result).is_none());
    let GroupReceiptError::Native(error) = result else {
        panic!()
    };
    assert_eq!(error.original.as_ptr(), pointer);
    drop(error);
    assert_eq!(*drops.borrow(), [1, 2]);
}

#[test]
fn uncalled_hook_owner_and_refused_input_clean_up_before_state_error_handoff() {
    let drops = Rc::new(RefCell::new(Vec::new()));
    let owner = native(2, &drops);
    let result = validate_group_receipt(
        StoredGroupState::Pending,
        false,
        Some(native(1, &drops)),
        move |_| -> Result<(), ()> {
            let _borrow_complete_owner = &owner;
            panic!("cold hook must not run");
        },
    );
    assert!(matches!(result, Err(GroupReceiptError::State)));
    assert_eq!(*drops.borrow(), [1, 2]);
}

struct PanicOwner {
    native: Native,
}
impl Drop for PanicOwner {
    fn drop(&mut self) {
        panic!("native hook cleanup");
    }
}

#[test]
fn uncalled_hook_cleanup_unwind_cannot_hand_off_a_refused_receipt() {
    let drops = Rc::new(RefCell::new(Vec::new()));
    let owner = PanicOwner {
        native: native(2, &drops),
    };
    let result = catch_unwind(AssertUnwindSafe(|| {
        let _ = validate_group_receipt(
            StoredGroupState::Pending,
            false,
            Some(native(1, &drops)),
            move |_| {
                let _borrow_complete_owner = &owner;
                Ok::<_, ()>(owner.native.id)
            },
        );
    }));
    assert!(result.is_err());
    assert_eq!(*drops.borrow(), [1, 2]);
}

#[test]
fn native_receipt_hook_unwind_consumes_input_without_retry_or_fabricated_completion() {
    let drops = Rc::new(RefCell::new(Vec::new()));
    let calls = Cell::new(0);
    let result = catch_unwind(AssertUnwindSafe(|| {
        let _ = validate_group_receipt(
            StoredGroupState::Completed,
            true,
            Some(native(1, &drops)),
            |input| -> Result<(), ()> {
                calls.set(calls.get() + 1);
                let _held = input;
                panic!("native decode unwind");
            },
        );
    }));
    assert!(result.is_err());
    assert_eq!(calls.get(), 1);
    assert_eq!(*drops.borrow(), [1]);
}

struct Batch {
    reserved: u64,
    state: StoredGroupState,
    all_completed: bool,
    receipt: Option<Native>,
}

fn native_checkpoint<Key: Ord>(keys: [Key; 2], corrupt: bool) {
    let drops = Rc::new(RefCell::new(Vec::new()));
    let calls = Cell::new(0);
    let commits = Cell::new(0);
    let mut recovery = CheckpointRecovery::<Key, Batch, ()>::new(2, 100);
    for (index, key) in keys.into_iter().enumerate() {
        recovery
            .push(RecoveryRecord {
                key,
                sequence: index as u64 + 1,
                payload: RecoveryPayload::Pending(Batch {
                    reserved: if corrupt { 10 } else { 10 + index as u64 },
                    state: StoredGroupState::Completed,
                    all_completed: true,
                    receipt: Some(native(index + 1, &drops)),
                }),
            })
            .unwrap();
    }
    let result = recovery.finish((), |_, snapshot| {
        let mut reservations = SequenceReservations::new(2, 100);
        for batch in snapshot.pending().values() {
            reservations.insert(batch.reserved)?;
        }
        let summary = reservations.finish()?;
        assert_eq!(summary.count, 2);
        for batch in snapshot.pending().values() {
            validate_reservation_proof(
                batch.reserved,
                false,
                ReservationObservation {
                    allocation_watermark: Some(20),
                    record_exists: false,
                    aliases_owner: false,
                },
            )
            .map_err(|_| ReservationError::Sequence)?;
            validate_group_receipt(
                batch.state,
                batch.all_completed,
                batch.receipt.as_ref(),
                |receipt| {
                    calls.set(calls.get() + 1);
                    assert_eq!(&*receipt.original, b"private-host-receipt");
                    Ok::<_, Infallible>(receipt)
                },
            )
            .map_err(|_| ReservationError::Sequence)?;
        }
        commits.set(commits.get() + 1);
        Ok(())
    });
    if corrupt {
        assert!(matches!(
            result,
            Err(CheckpointCommitError::Host(ReservationError::Duplicate))
        ));
        assert_eq!(calls.get(), 0);
        assert_eq!(commits.get(), 0);
        assert_eq!(drops.borrow().len(), 2);
    } else {
        let (snapshot, ()) = result.unwrap();
        assert_eq!(calls.get(), 2);
        assert_eq!(commits.get(), 1);
        assert!(drops.borrow().is_empty());
        drop(snapshot);
        assert_eq!(drops.borrow().len(), 2);
    }
}

#[test]
fn unrelated_numeric_and_gui_identity_schemas_share_fences_before_checkpoint_commit() {
    for corrupt in [true, false] {
        native_checkpoint([1u64, 2], corrupt);
        native_checkpoint([[1u8; 16], [2u8; 16]], corrupt);
    }
}
