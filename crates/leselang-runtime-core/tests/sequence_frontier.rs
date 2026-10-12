use std::{
    cell::{Cell, RefCell},
    convert::Infallible,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};

use leselang_runtime_core::{
    CheckpointCommitError, CheckpointRecovery, RecoveryPayload, RecoveryRecord,
    SequenceFrontierError, SequenceFrontierSite, SequentialMemberState as State,
    inspect_sequential_frontier,
};

#[test]
fn cold_limits_precede_every_native_hook_and_empty_input_keeps_both_admission_phases() {
    let called = Cell::new(0);
    for (rows, limit, invalid) in [(&[State::Pending][..], 0, false), (&[][..], 65, true)] {
        let error = inspect_sequential_frontier(
            rows,
            limit,
            |_, state| {
                called.set(called.get() + 1);
                Ok::<_, Infallible>(*state)
            },
            |_| {
                called.set(called.get() + 1);
                Ok(())
            },
            |_, _| {
                called.set(called.get() + 1);
                Ok(())
            },
        )
        .unwrap_err();
        assert!(if invalid {
            matches!(error, SequenceFrontierError::InvalidLimits)
        } else {
            matches!(error, SequenceFrontierError::Count)
        });
    }
    assert_eq!(called.get(), 0);
    let phases = RefCell::new(Vec::new());
    inspect_sequential_frontier(
        &[] as &[State],
        0,
        |_, _| panic!(),
        |prefix| {
            assert!(prefix.is_empty());
            phases.borrow_mut().push("prefix");
            Ok::<_, Infallible>(())
        },
        |frontier, blocked| {
            assert_eq!(frontier, 0);
            assert!(blocked.is_empty());
            phases.borrow_mut().push("blocked");
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(*phases.borrow(), ["prefix", "blocked"]);
}

#[test]
fn inclusive_ceiling_classifies_all_original_members_before_admission() {
    let rows = [State::Completed; 64];
    let called = Cell::new(0);
    inspect_sequential_frontier(
        &rows,
        64,
        |index, state| {
            assert_eq!(called.get(), index);
            called.set(index + 1);
            Ok::<_, Infallible>(*state)
        },
        |prefix| {
            assert_eq!(called.get(), 64);
            assert_eq!(prefix.len(), 64);
            Ok(())
        },
        |frontier, blocked| {
            assert_eq!(frontier, 64);
            assert!(blocked.is_empty());
            Ok(())
        },
    )
    .unwrap();
}

#[test]
fn every_frontier_preserves_original_rows_and_excludes_only_the_active_pending_member() {
    for frontier in 0..=4 {
        let rows = (0..4)
            .map(|index| {
                (
                    99 - index,
                    if index < frontier {
                        State::Completed
                    } else {
                        State::Pending
                    },
                )
            })
            .collect::<Vec<_>>();
        let seen = RefCell::new(Vec::new());
        inspect_sequential_frontier(
            &rows,
            4,
            |index, row| {
                assert!(std::ptr::eq(row, &rows[index]));
                seen.borrow_mut().push(row.0);
                Ok::<_, Infallible>(row.1)
            },
            |prefix| {
                assert_eq!(seen.borrow().len(), 4);
                assert_eq!(prefix.len(), frontier);
                assert!(std::ptr::eq(prefix.as_ptr(), rows.as_ptr()));
                Ok(())
            },
            |actual, blocked| {
                assert_eq!(actual, frontier);
                let start = (frontier + 1).min(rows.len());
                assert_eq!(blocked.len(), rows.len() - start);
                assert!(std::ptr::eq(blocked.as_ptr(), rows[start..].as_ptr()));
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(*seen.borrow(), [99, 98, 97, 96]);
    }
}

#[test]
fn late_completion_refuses_before_prefix_decode_or_blocked_dispatch_checks() {
    let rows = [
        State::Completed,
        State::Pending,
        State::Completed,
        State::Pending,
    ];
    let seen = RefCell::new(Vec::new());
    let error = inspect_sequential_frontier(
        &rows,
        4,
        |index, state| {
            seen.borrow_mut().push(index);
            Ok::<_, Infallible>(*state)
        },
        |_| panic!("prefix must not be decoded"),
        |_, _| panic!("dispatch must not be read"),
    )
    .unwrap_err();
    assert!(matches!(
        error,
        SequenceFrontierError::OutOfOrder { index: 2 }
    ));
    assert_eq!(*seen.borrow(), [0, 1, 2]);
}

struct PrivateError(Box<[u8]>);

#[test]
fn original_native_member_fault_precedes_order_refusal_and_moves_without_formatting() {
    let mut error = Some(PrivateError(
        b"secret-native-member".to_vec().into_boxed_slice(),
    ));
    let pointer = error.as_ref().unwrap().0.as_ptr();
    let rows = [State::Pending, State::Completed];
    let result = inspect_sequential_frontier(
        &rows,
        2,
        |index, state| {
            if index == 1 {
                return Err(error.take().unwrap());
            }
            Ok(*state)
        },
        |_| panic!(),
        |_, _| panic!(),
    )
    .unwrap_err();
    assert!(!format!("{result:?}: {result}").contains("secret-native-member"));
    assert!(std::error::Error::source(&result).is_none());
    let SequenceFrontierError::Native {
        site: SequenceFrontierSite::Member(1),
        error,
    } = result
    else {
        panic!()
    };
    assert_eq!(error.0.as_ptr(), pointer);
}

#[test]
fn prefix_native_refusal_stops_before_blocked_observation() {
    let error = PrivateError(b"private-prefix".to_vec().into_boxed_slice());
    let pointer = error.0.as_ptr();
    let result = inspect_sequential_frontier(
        &[State::Completed, State::Pending],
        2,
        |_, state| Ok(*state),
        |prefix| {
            assert_eq!(prefix.len(), 1);
            Err(error)
        },
        |_, _| panic!(),
    )
    .unwrap_err();
    let SequenceFrontierError::Native {
        site: SequenceFrontierSite::Prefix,
        error,
    } = result
    else {
        panic!()
    };
    assert_eq!(error.0.as_ptr(), pointer);
}

#[test]
fn blocked_native_error_moves_unchanged_after_complete_prefix_admission() {
    let prefix_done = Cell::new(false);
    let error = PrivateError(b"private-blocked".to_vec().into_boxed_slice());
    let pointer = error.0.as_ptr();
    let result = inspect_sequential_frontier(
        &[State::Pending; 3],
        3,
        |_, state| Ok(*state),
        |_| {
            prefix_done.set(true);
            Ok(())
        },
        |frontier, blocked| {
            assert!(prefix_done.get());
            assert_eq!(frontier, 0);
            assert_eq!(blocked.len(), 2);
            Err(error)
        },
    )
    .unwrap_err();
    let SequenceFrontierError::Native {
        site: SequenceFrontierSite::Blocked,
        error,
    } = result
    else {
        panic!()
    };
    assert_eq!(error.0.as_ptr(), pointer);
}

#[test]
fn each_native_hook_unwind_stops_without_retry_or_later_phase() {
    for panic_phase in 0..3 {
        let phases = RefCell::new(Vec::new());
        let result = catch_unwind(AssertUnwindSafe(|| {
            let _ = inspect_sequential_frontier(
                &[State::Pending],
                1,
                |_, state| {
                    phases.borrow_mut().push(0);
                    assert_ne!(panic_phase, 0);
                    Ok::<_, Infallible>(*state)
                },
                |_| {
                    phases.borrow_mut().push(1);
                    assert_ne!(panic_phase, 1);
                    Ok(())
                },
                |_, _| {
                    phases.borrow_mut().push(2);
                    assert_ne!(panic_phase, 2);
                    Ok(())
                },
            );
        }));
        assert!(result.is_err());
        assert_eq!(*phases.borrow(), (0..=panic_phase).collect::<Vec<_>>());
    }
}

struct Owner {
    drops: Rc<RefCell<Vec<usize>>>,
    marker: usize,
    panic: bool,
}
impl Drop for Owner {
    fn drop(&mut self) {
        self.drops.borrow_mut().push(self.marker);
        assert!(!self.panic, "hook owner cleanup");
    }
}

#[test]
fn uncalled_prefix_blocked_and_state_owners_release_before_cold_handoff() {
    let drops = Rc::new(RefCell::new(Vec::new()));
    let owner = |marker| Owner {
        drops: drops.clone(),
        marker,
        panic: false,
    };
    let (state, prefix, blocked) = (owner(0), owner(1), owner(2));
    let result = inspect_sequential_frontier(
        &[State::Pending],
        0,
        move |_, row| {
            let _ = &state;
            Ok::<_, Infallible>(*row)
        },
        move |_| {
            drop(prefix);
            Ok(())
        },
        move |_, _| {
            drop(blocked);
            Ok(())
        },
    );
    assert!(matches!(result, Err(SequenceFrontierError::Count)));
    drops.borrow_mut().sort();
    assert_eq!(*drops.borrow(), [0, 1, 2]);
}

#[test]
fn state_owner_cleanup_unwind_prevents_success_handoff() {
    let drops = Rc::new(RefCell::new(Vec::new()));
    let owner = Owner {
        drops: drops.clone(),
        marker: 0,
        panic: true,
    };
    let handed_off = Cell::new(false);
    let result = catch_unwind(AssertUnwindSafe(|| {
        let _ = inspect_sequential_frontier(
            &[State::Pending],
            1,
            move |_, state| {
                let _ = &owner;
                Ok::<_, Infallible>(*state)
            },
            |_| Ok(()),
            |_, _| Ok(()),
        );
        handed_off.set(true);
    }));
    assert!(result.is_err());
    assert!(!handed_off.get());
    assert_eq!(*drops.borrow(), [0]);
}

#[test]
fn gui_local_opaque_rows_keep_active_attempts_separate_from_blocked_native_policy() {
    struct Widget {
        state: State,
        submitted: Rc<Cell<bool>>,
        opaque: Box<[u8]>,
    }
    for advanced in [false, true] {
        let rows = [
            Widget {
                state: State::Completed,
                submitted: Rc::new(Cell::new(true)),
                opaque: vec![9].into_boxed_slice(),
            },
            Widget {
                state: State::Pending,
                submitted: Rc::new(Cell::new(true)),
                opaque: vec![3].into_boxed_slice(),
            },
            Widget {
                state: State::Pending,
                submitted: Rc::new(Cell::new(advanced)),
                opaque: vec![8].into_boxed_slice(),
            },
        ];
        let original = rows[2].opaque.as_ptr();
        let result = inspect_sequential_frontier(
            &rows,
            3,
            |_, row| Ok(row.state),
            |prefix| {
                assert_eq!(prefix.len(), 1);
                Ok(())
            },
            |frontier, blocked| {
                assert_eq!(frontier, 1);
                assert_eq!(blocked.len(), 1);
                assert_eq!(blocked[0].opaque.as_ptr(), original);
                if blocked[0].submitted.get() {
                    Err("native-blocked-attempt")
                } else {
                    Ok(())
                }
            },
        );
        assert_eq!(result.is_err(), advanced);
    }
}

#[test]
fn owner_cleanup_unwind_releases_unpublished_native_error_without_later_hooks() {
    struct NativeError(Rc<Cell<usize>>);
    impl Drop for NativeError {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }
    let drops = Rc::new(RefCell::new(Vec::new()));
    let owner = Owner {
        drops: drops.clone(),
        marker: 0,
        panic: true,
    };
    let errors = Rc::new(Cell::new(0));
    let error = NativeError(errors.clone());
    let blocked = Cell::new(false);
    let handed_off = Cell::new(false);
    let result = catch_unwind(AssertUnwindSafe(|| {
        let _ = inspect_sequential_frontier(
            &[State::Pending],
            1,
            move |_, state| {
                let _ = &owner;
                Ok(*state)
            },
            |_| Err(error),
            |_, _| {
                blocked.set(true);
                Ok(())
            },
        );
        handed_off.set(true);
    }));
    assert!(result.is_err());
    assert!(!blocked.get());
    assert!(!handed_off.get());
    assert_eq!(errors.get(), 1);
    assert_eq!(*drops.borrow(), [0]);
}

#[test]
fn numeric_frontier_refusal_blocks_checkpoint_commit_and_snapshot_handoff() {
    for late_completed in [false, true] {
        let states = [
            State::Completed,
            State::Pending,
            if late_completed {
                State::Completed
            } else {
                State::Pending
            },
        ];
        let mut recovery = CheckpointRecovery::<u64, (), ()>::new(1, 100);
        recovery
            .push(RecoveryRecord {
                key: 9,
                sequence: 41,
                payload: RecoveryPayload::Pending(()),
            })
            .unwrap();
        let watermark = Cell::new(1);
        let outcome = recovery.finish(states, |rows, snapshot| {
            inspect_sequential_frontier(
                rows,
                3,
                |_, state| Ok::<_, Infallible>(*state),
                |_| Ok(()),
                |_, _| Ok(()),
            )?;
            watermark.set(snapshot.highest_sequence() + 1);
            Ok(())
        });
        if late_completed {
            assert!(matches!(
                outcome,
                Err(CheckpointCommitError::Host(
                    SequenceFrontierError::OutOfOrder { index: 2 }
                ))
            ));
            assert_eq!(watermark.get(), 1);
        } else {
            let (snapshot, ()) = outcome.unwrap();
            assert_eq!(snapshot.pending().len(), 1);
            assert_eq!(watermark.get(), 42);
        }
    }
}
