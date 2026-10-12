use std::{cell::Cell, rc::Rc};

use leselang_runtime_core::{
    CheckpointCommitError, CheckpointRecovery, RecoveryPayload, RecoveryRecord,
    SequenceWatermarkError, SequenceWatermarkRequirement, SequenceWatermarkUpdate,
};

#[test]
fn empty_observation_requires_one_without_making_zero_an_identity() {
    let required = SequenceWatermarkRequirement::after_observation(0, 1).unwrap();
    assert_eq!(required.required_watermark(), 1);
    assert_eq!(
        required.reconcile(1).unwrap(),
        SequenceWatermarkUpdate {
            next_sequence: 1,
            write_required: false
        }
    );
}

#[test]
fn inclusive_ceiling_accepts_the_last_watermark_not_the_next_observation() {
    let required = SequenceWatermarkRequirement::after_observation(9, 10).unwrap();
    assert_eq!(required.required_watermark(), 10);
    assert_eq!(required.reconcile(10).unwrap().next_sequence, 10);
    assert_eq!(
        SequenceWatermarkRequirement::after_observation(10, 10),
        Err(SequenceWatermarkError::Exhausted)
    );
}

#[test]
fn full_width_arithmetic_refuses_overflow_instead_of_wrapping() {
    let required = SequenceWatermarkRequirement::after_observation(u64::MAX - 1, u64::MAX).unwrap();
    assert_eq!(required.required_watermark(), u64::MAX);
    assert!(!required.reconcile(u64::MAX).unwrap().write_required);
    assert_eq!(
        SequenceWatermarkRequirement::after_observation(u64::MAX, u64::MAX),
        Err(SequenceWatermarkError::Exhausted)
    );
}

#[test]
fn zero_ceiling_is_not_an_implicit_default() {
    for used in [0, 1, u64::MAX] {
        assert_eq!(
            SequenceWatermarkRequirement::after_observation(used, 0),
            Err(SequenceWatermarkError::Exhausted)
        );
    }
}

#[test]
fn reconciliation_rejects_invalid_stored_values_without_clamping() {
    let required = SequenceWatermarkRequirement::after_observation(2, 10).unwrap();
    for stored in [0, 11, u64::MAX] {
        assert_eq!(
            required.reconcile(stored),
            Err(SequenceWatermarkError::InvalidStored)
        );
    }
    assert_eq!(required.required_watermark(), 3);
}

#[test]
fn bounded_matrix_preserves_monotonicity_and_exact_write_decisions() {
    for used in 0..64 {
        let required = SequenceWatermarkRequirement::after_observation(used, 64).unwrap();
        for stored in 1..=64 {
            let update = required.reconcile(stored).unwrap();
            assert_eq!(update.next_sequence, stored.max(used + 1));
            assert_eq!(update.write_required, used + 1 > stored);
            assert!(update.next_sequence >= stored);
        }
    }
}

#[test]
fn old_imports_preserve_existing_gaps_without_an_unnecessary_write() {
    let required = SequenceWatermarkRequirement::after_observation(4, 100).unwrap();
    assert_eq!(
        required.reconcile(100).unwrap(),
        SequenceWatermarkUpdate {
            next_sequence: 100,
            write_required: false
        }
    );
    assert_eq!(
        required.reconcile(5).unwrap(),
        SequenceWatermarkUpdate {
            next_sequence: 5,
            write_required: false
        }
    );
}

#[test]
fn pure_requirements_can_observe_different_snapshots_but_do_not_remember_them() {
    let required = SequenceWatermarkRequirement::after_observation(10, 100).unwrap();
    assert_eq!(required.reconcile(1).unwrap().next_sequence, 11);
    assert_eq!(required.reconcile(20).unwrap().next_sequence, 20);
    assert!(required.reconcile(0).is_err());
    assert_eq!(required.reconcile(1).unwrap().next_sequence, 11);
}

#[test]
fn numeric_updates_are_mutable_data_not_an_authorized_allocator_handle() {
    let required = SequenceWatermarkRequirement::after_observation(4, 10).unwrap();
    let mut update = required.reconcile(1).unwrap();
    update.next_sequence = u64::MAX;
    update.write_required = false;
    assert_eq!(update.next_sequence, u64::MAX);
    assert_eq!(required.required_watermark(), 5);
    assert_eq!(required.reconcile(1).unwrap().next_sequence, 5);
}

#[test]
fn independent_host_ceilings_do_not_share_a_global_registry_or_sequence_grant() {
    let small = SequenceWatermarkRequirement::after_observation(4, 5).unwrap();
    let large = SequenceWatermarkRequirement::after_observation(4, u64::MAX).unwrap();
    assert!(small.reconcile(100).is_err());
    assert_eq!(large.reconcile(100).unwrap().next_sequence, 100);
    assert_eq!(small.reconcile(1).unwrap().next_sequence, 5);
}

// These GUI-local host models are protocol proofs, not a storage implementation.
struct GuiFrame {
    fuel: u64,
    _local: Rc<()>,
}
struct NativeCommitFault {
    secret: Box<str>,
}

#[test]
fn gui_local_checkpoint_handoff_waits_for_native_final_checks_and_commit() {
    let stored = Rc::new(Cell::new(2));
    let mut staging = CheckpointRecovery::<u64, GuiFrame, ()>::new(1, 9);
    staging
        .push(RecoveryRecord {
            key: 8,
            sequence: 8,
            payload: RecoveryPayload::Pending(GuiFrame {
                fuel: 37,
                _local: Rc::new(()),
            }),
        })
        .unwrap();
    let (snapshot, update) = staging
        .finish(stored.clone(), |host, snapshot| {
            assert_eq!(snapshot.pending()[&8].fuel, 37);
            let required =
                SequenceWatermarkRequirement::after_observation(snapshot.highest_sequence(), 10)
                    .unwrap();
            let update = required.reconcile(host.get()).unwrap();
            if update.write_required {
                host.set(update.next_sequence);
            }
            Ok::<_, NativeCommitFault>(update)
        })
        .unwrap();
    assert_eq!(stored.get(), 9);
    assert_eq!(Rc::strong_count(&stored), 1);
    assert_eq!(update.next_sequence, 9);
    assert_eq!(snapshot.pending()[&8].fuel, 37);
}

#[test]
fn native_commit_refusal_never_hands_off_staging_or_applies_a_prospective_update() {
    let stored = Rc::new(Cell::new(2));
    let local = Rc::new(());
    let mut staging = CheckpointRecovery::<u64, GuiFrame, ()>::new(1, 9);
    staging
        .push(RecoveryRecord {
            key: 8,
            sequence: 8,
            payload: RecoveryPayload::Pending(GuiFrame {
                fuel: 37,
                _local: local.clone(),
            }),
        })
        .unwrap();
    let result = staging.finish(stored.clone(), |host, snapshot| {
        let required =
            SequenceWatermarkRequirement::after_observation(snapshot.highest_sequence(), 10)
                .unwrap();
        let update = required.reconcile(host.get()).unwrap();
        assert!(update.write_required);
        Err::<SequenceWatermarkUpdate, _>(NativeCommitFault {
            secret: "private-commit-fault".into(),
        })
    });
    let error = result.err().unwrap();
    assert!(!format!("{error:?}").contains("private-commit-fault"));
    let CheckpointCommitError::Host(error) = error else {
        panic!("native failure expected")
    };
    assert_eq!(&*error.secret, "private-commit-fault");
    assert_eq!(stored.get(), 2);
    assert_eq!(Rc::strong_count(&stored), 1);
    assert_eq!(Rc::strong_count(&local), 1);
}
