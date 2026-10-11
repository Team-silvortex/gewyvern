use std::{
    cell::{Cell, RefCell},
    convert::Infallible,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};

use leselang_runtime_core::{SuccessorChainError, SuccessorChainSite, inspect_successor_chain};

#[test]
fn cold_bounds_precede_first_and_all_link_hooks() {
    let calls = Cell::new(0);
    let rows = [9, 2, 7];
    for (count, limit, invalid) in [(0, 0, false), (1, 64, false), (3, 2, false), (3, 65, true)] {
        let error = inspect_successor_chain(
            &rows[..count],
            limit,
            |_| {
                calls.set(calls.get() + 1);
                Ok::<_, Infallible>(())
            },
            |_, _, _, _| {
                calls.set(calls.get() + 1);
                Ok(())
            },
        )
        .unwrap_err();
        assert!(if invalid {
            matches!(error, SuccessorChainError::InvalidLimits)
        } else {
            matches!(error, SuccessorChainError::Count)
        });
    }
    assert_eq!(calls.get(), 0);
}

#[test]
fn original_adjacent_rows_and_frames_are_not_sorted_or_renumbered() {
    let rows = [90, 3, 71, 2];
    let seen = RefCell::new(Vec::new());
    inspect_successor_chain(
        &rows,
        4,
        |row| {
            assert!(std::ptr::eq(row, &rows[0]));
            seen.borrow_mut().push(*row);
            Ok::<_, Infallible>(row)
        },
        |site, parent, previous, next| {
            let SuccessorChainSite::Link {
                parent: index,
                next: following,
            } = site
            else {
                panic!()
            };
            assert_eq!(following, index + 1);
            assert!(std::ptr::eq(*parent, previous));
            assert!(std::ptr::eq(previous, &rows[index]));
            assert!(std::ptr::eq(next, &rows[following]));
            seen.borrow_mut().push(*next);
            Ok(next)
        },
    )
    .unwrap();
    assert_eq!(*seen.borrow(), rows);
}

#[test]
fn full_inclusive_ceiling_visits_all_sixty_three_links() {
    let rows = [0_u8; 64];
    let mut links = 0;
    inspect_successor_chain(
        &rows,
        64,
        |_| Ok::<_, Infallible>(0),
        |_, parent, _, _| {
            links += 1;
            Ok(parent + 1)
        },
    )
    .unwrap();
    assert_eq!(links, 63);
}

struct Frame {
    index: usize,
    payload: Box<[u8]>,
    log: Rc<RefCell<Vec<usize>>>,
    panic_at: Option<usize>,
}
impl Drop for Frame {
    fn drop(&mut self) {
        self.log.borrow_mut().push(self.index);
        assert_ne!(self.panic_at, Some(self.index), "native frame cleanup");
    }
}
fn frame(index: usize, log: &Rc<RefCell<Vec<usize>>>) -> Frame {
    Frame {
        index,
        payload: vec![index as u8].into_boxed_slice(),
        log: log.clone(),
        panic_at: None,
    }
}

#[test]
fn gui_local_move_only_frames_release_before_next_link_and_final_handoff() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let first = frame(0, &log);
    let original = first.payload.as_ptr();
    inspect_successor_chain(
        &[0, 1, 2],
        3,
        |_| Ok::<_, Infallible>(first),
        |_, parent, previous, next| {
            assert_eq!(parent.index, *previous);
            if *previous == 0 {
                assert_eq!(parent.payload.as_ptr(), original);
            }
            assert_eq!(*log.borrow(), (0..*previous).collect::<Vec<_>>());
            Ok(frame(*next, &log))
        },
    )
    .unwrap();
    assert_eq!(*log.borrow(), [0, 1, 2]);
}

struct PrivateError(Box<[u8]>);

#[test]
fn first_native_error_moves_unchanged_and_never_observes_links() {
    let error = PrivateError(b"private-first".to_vec().into_boxed_slice());
    let pointer = error.0.as_ptr();
    let result =
        inspect_successor_chain(&[1, 2], 2, |_| Err::<(), _>(error), |_, _, _, _| panic!())
            .unwrap_err();
    assert!(!format!("{result:?}: {result}").contains("private-first"));
    assert!(std::error::Error::source(&result).is_none());
    let SuccessorChainError::Native {
        site: SuccessorChainSite::First,
        error,
    } = result
    else {
        panic!()
    };
    assert_eq!(error.0.as_ptr(), pointer);
}

#[test]
fn later_native_refusal_stops_exactly_at_original_link_and_releases_parent() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let mut calls = Vec::new();
    let mut error = Some(PrivateError(b"private-link".to_vec().into_boxed_slice()));
    let pointer = error.as_ref().unwrap().0.as_ptr();
    let outcome = inspect_successor_chain(
        &[0, 1, 2, 3],
        4,
        |_| Ok(frame(0, &log)),
        |site, _, _, next| {
            calls.push(site);
            if *next == 2 {
                return Err(error.take().unwrap());
            }
            Ok(frame(*next, &log))
        },
    )
    .unwrap_err();
    assert_eq!(calls.len(), 2);
    assert_eq!(*log.borrow(), [0, 1]);
    let SuccessorChainError::Native {
        site: SuccessorChainSite::Link { parent: 1, next: 2 },
        error,
    } = outcome
    else {
        panic!()
    };
    assert_eq!(error.0.as_ptr(), pointer);
}

#[test]
fn native_link_owns_result_decode_before_successor_observation() {
    let reads = RefCell::new(Vec::new());
    let result = inspect_successor_chain(
        &[1, 2, 3],
        3,
        |row| {
            reads.borrow_mut().push(*row);
            Ok(1)
        },
        |_, _, previous, next| {
            if *previous == 1 {
                return Err("parent-wire");
            }
            reads.borrow_mut().push(*next);
            Err("next-wire")
        },
    );
    assert!(matches!(
        result,
        Err(SuccessorChainError::Native {
            error: "parent-wire",
            ..
        })
    ));
    assert_eq!(*reads.borrow(), [1]);
}

#[test]
fn native_link_unwind_releases_original_frame_without_retry_or_later_observation() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let calls = Cell::new(0);
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        let _ = inspect_successor_chain(
            &[0, 1, 2],
            3,
            |_| Ok::<_, Infallible>(frame(0, &log)),
            |_, _, _, _| {
                calls.set(calls.get() + 1);
                panic!("native link unwind");
            },
        );
    }));
    assert!(outcome.is_err());
    assert_eq!(calls.get(), 1);
    assert_eq!(*log.borrow(), [0]);
}

#[test]
fn original_frame_cleanup_unwind_releases_unpublished_next_frame() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let calls = Cell::new(0);
    let mut first = frame(0, &log);
    first.panic_at = Some(0);
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        let _ = inspect_successor_chain(
            &[0, 1, 2],
            3,
            |_| Ok::<_, Infallible>(first),
            |_, _, _, next| {
                calls.set(calls.get() + 1);
                Ok(frame(*next, &log))
            },
        );
    }));
    assert!(outcome.is_err());
    assert_eq!(calls.get(), 1);
    assert_eq!(*log.borrow(), [0, 1]);
}

struct Owner {
    log: Rc<RefCell<Vec<usize>>>,
    marker: usize,
    panic: bool,
}
impl Drop for Owner {
    fn drop(&mut self) {
        self.log.borrow_mut().push(self.marker);
        assert!(!self.panic, "native hook owner cleanup");
    }
}

#[test]
fn cold_refusal_releases_uncalled_hook_owners_before_handoff() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let first = Owner {
        log: log.clone(),
        marker: 10,
        panic: false,
    };
    let next = Owner {
        log: log.clone(),
        marker: 11,
        panic: false,
    };
    let result = inspect_successor_chain(
        &[0],
        1,
        move |_| {
            drop(first);
            Ok::<_, Infallible>(())
        },
        move |_, _, _, _| {
            let _ = &next;
            panic!();
        },
    );
    assert!(matches!(result, Err(SuccessorChainError::Count)));
    assert_eq!(*log.borrow(), [10, 11]);
}

#[test]
fn callback_owner_cleanup_unwind_prevents_success_handoff_after_final_frame_cleanup() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let owner = Owner {
        log: log.clone(),
        marker: 10,
        panic: true,
    };
    let local = log.clone();
    let published = Cell::new(false);
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        let _ = inspect_successor_chain(
            &[0, 1],
            2,
            |_| Ok::<_, Infallible>(frame(0, &log)),
            move |_, _, _, next| {
                let _ = &owner;
                Ok(frame(*next, &local))
            },
        );
        published.set(true);
    }));
    assert!(outcome.is_err());
    assert!(!published.get());
    assert_eq!(*log.borrow(), [0, 1, 10]);
}

#[test]
fn numeric_native_policy_is_not_replaced_with_a_generic_frame_or_execution_grant() {
    struct Native {
        identity: u64,
        fuel: u8,
    }
    let rows = [
        Native {
            identity: 9,
            fuel: 3,
        },
        Native {
            identity: 8,
            fuel: 4,
        },
    ];
    let result = inspect_successor_chain(&rows, 2, Ok, |_, parent, _, next| {
        if next.identity <= parent.identity || next.fuel >= parent.fuel {
            return Err("native-authority");
        }
        Ok(next)
    });
    assert!(matches!(
        result,
        Err(SuccessorChainError::Native {
            error: "native-authority",
            ..
        })
    ));
}
