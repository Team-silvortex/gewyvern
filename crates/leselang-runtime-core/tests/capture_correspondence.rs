use std::{
    cell::{Cell, RefCell},
    convert::Infallible,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};

use leselang_runtime_core::{
    CaptureCorrespondenceError as Error, CaptureCorrespondenceLimits as Limits,
    CaptureCorrespondenceSite as Site, validate_capture_correspondence,
};

fn limits(saved: usize, originals: usize) -> Limits {
    Limits {
        max_saved_bindings: saved,
        max_original_rows: originals,
    }
}

#[test]
fn cold_aggregate_and_original_limits_precede_every_native_hook() {
    let calls = Cell::new(0);
    for (limit, invalid, originals) in [
        (limits(65, 2), true, 2),
        (limits(2, 65), true, 2),
        (limits(1, 1), false, 2),
        (limits(2, 1), false, 2),
    ] {
        let error = validate_capture_correspondence(
            &[1],
            &[2],
            &[7, 9][..originals],
            limit,
            |_, _| {
                calls.set(calls.get() + 1);
                Ok::<_, Infallible>(true)
            },
            |_, _, _| {
                calls.set(calls.get() + 1);
                Ok(true)
            },
        )
        .unwrap_err();
        assert!(if invalid {
            matches!(error, Error::InvalidLimits)
        } else if limit.max_saved_bindings == 1 {
            matches!(error, Error::BindingCount)
        } else {
            matches!(error, Error::OriginalCount)
        });
    }
    assert_eq!(calls.get(), 0);
}

#[test]
fn groups_precede_capture_major_original_scans_without_sorting_or_shortcutting() {
    let groups = [90, 2];
    let captures = [8, 3];
    let rows = [71, 4, 9];
    let seen = RefCell::new(Vec::new());
    validate_capture_correspondence(
        &groups,
        &captures,
        &rows,
        limits(4, 3),
        |index, group| {
            assert!(std::ptr::eq(group, &groups[index]));
            seen.borrow_mut().push(Site::Group(index));
            Ok::<_, Infallible>(true)
        },
        |site, result, row| {
            let Site::Result { capture, original } = site else {
                panic!()
            };
            assert!(std::ptr::eq(result, &captures[capture]));
            assert!(std::ptr::eq(row, &rows[original]));
            seen.borrow_mut().push(site);
            Ok(original == 0)
        },
    )
    .unwrap();
    assert_eq!(
        *seen.borrow(),
        [
            Site::Group(0),
            Site::Group(1),
            Site::Result {
                capture: 0,
                original: 0
            },
            Site::Result {
                capture: 0,
                original: 1
            },
            Site::Result {
                capture: 0,
                original: 2
            },
            Site::Result {
                capture: 1,
                original: 0
            },
            Site::Result {
                capture: 1,
                original: 1
            },
            Site::Result {
                capture: 1,
                original: 2
            }
        ]
    );
}

#[test]
fn full_inclusive_matrix_is_bounded_to_sixty_four_times_sixty_four() {
    let captures = [0; 64];
    let rows = [0; 64];
    let mut calls = 0;
    validate_capture_correspondence(
        &[] as &[()],
        &captures,
        &rows,
        limits(64, 64),
        |_, _| panic!(),
        |site, _, _| {
            let Site::Result { capture, original } = site else {
                panic!()
            };
            assert_eq!(calls, capture * 64 + original);
            calls += 1;
            Ok::<_, Infallible>(true)
        },
    )
    .unwrap();
    assert_eq!(calls, 4096);
}

#[test]
fn unmatched_group_stops_before_later_groups_or_scalar_correspondence() {
    let seen = RefCell::new(Vec::new());
    let error = validate_capture_correspondence(
        &[1, 2, 3],
        &[1],
        &[1],
        limits(4, 1),
        |index, _| {
            seen.borrow_mut().push(index);
            Ok::<_, Infallible>(index == 0)
        },
        |_, _, _| panic!(),
    )
    .unwrap_err();
    assert!(matches!(error, Error::GroupMismatch { index: 1 }));
    assert_eq!(*seen.borrow(), [0, 1]);
}

#[test]
fn unmatched_capture_scans_all_originals_then_stops_before_next_capture() {
    let seen = RefCell::new(Vec::new());
    let error = validate_capture_correspondence(
        &[] as &[()],
        &[7, 8],
        &[3, 1],
        limits(2, 2),
        |_, _| panic!(),
        |site, _, _| {
            seen.borrow_mut().push(site);
            Ok::<_, Infallible>(false)
        },
    )
    .unwrap_err();
    assert!(matches!(error, Error::ResultMismatch { index: 0 }));
    assert_eq!(seen.borrow().len(), 2);
}

struct PrivateError(Box<[u8]>);

#[test]
fn late_native_error_after_true_match_moves_unchanged_instead_of_being_hidden() {
    let mut error = Some(PrivateError(
        b"private-native-match".to_vec().into_boxed_slice(),
    ));
    let pointer = error.as_ref().unwrap().0.as_ptr();
    let seen = RefCell::new(Vec::new());
    let outcome = validate_capture_correspondence(
        &[] as &[()],
        &[8, 9],
        &[1, 2, 3],
        limits(2, 3),
        |_, _| panic!(),
        |site, _, _| {
            seen.borrow_mut().push(site);
            if matches!(site, Site::Result { original: 1, .. }) {
                Err(error.take().unwrap())
            } else {
                Ok(true)
            }
        },
    )
    .unwrap_err();
    assert_eq!(seen.borrow().len(), 2);
    assert!(!format!("{outcome:?}: {outcome}").contains("private-native-match"));
    assert!(std::error::Error::source(&outcome).is_none());
    let Error::Native {
        site: Site::Result {
            capture: 0,
            original: 1,
        },
        error,
    } = outcome
    else {
        panic!()
    };
    assert_eq!(error.0.as_ptr(), pointer);
}

#[test]
fn native_group_error_precedes_boolean_mismatch_and_retains_original_ownership() {
    let error = PrivateError(vec![1, 2].into_boxed_slice());
    let pointer = error.0.as_ptr();
    let mut failure = Some(error);
    let outcome = validate_capture_correspondence(
        &[1],
        &[2],
        &[3],
        limits(2, 1),
        |_, _| Err(failure.take().unwrap()),
        |_, _, _| panic!(),
    )
    .unwrap_err();
    let Error::Native {
        site: Site::Group(0),
        error,
    } = outcome
    else {
        panic!()
    };
    assert_eq!(error.0.as_ptr(), pointer);
}

#[test]
fn empty_capture_lists_are_not_invented_and_empty_originals_cannot_match_a_saved_result() {
    validate_capture_correspondence(
        &[] as &[()],
        &[] as &[()],
        &[] as &[()],
        limits(0, 0),
        |_, _| -> Result<bool, Infallible> { panic!() },
        |_, _, _| panic!(),
    )
    .unwrap();
    let outcome = validate_capture_correspondence(
        &[] as &[()],
        &[1],
        &[] as &[()],
        limits(1, 0),
        |_, _| panic!(),
        |_, _, _| Ok::<_, Infallible>(true),
    );
    assert!(matches!(outcome, Err(Error::ResultMismatch { index: 0 })));
}

#[test]
fn native_result_unwind_stops_before_retry_or_later_capture() {
    let calls = Cell::new(0);
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let _ = validate_capture_correspondence(
                &[] as &[()],
                &[1, 2],
                &[1, 2],
                limits(2, 2),
                |_, _| Ok::<_, Infallible>(true),
                |_, _, _| {
                    calls.set(calls.get() + 1);
                    panic!("native match unwind");
                },
            );
        }))
        .is_err()
    );
    assert_eq!(calls.get(), 1);
}

struct Owner {
    drops: Rc<RefCell<Vec<usize>>>,
    id: usize,
    panic: bool,
}
impl Drop for Owner {
    fn drop(&mut self) {
        self.drops.borrow_mut().push(self.id);
        assert!(!self.panic, "native match owner cleanup");
    }
}

#[test]
fn uncalled_hook_owners_release_before_cold_limit_handoff() {
    let drops = Rc::new(RefCell::new(Vec::new()));
    let group = Owner {
        drops: drops.clone(),
        id: 1,
        panic: false,
    };
    let result = Owner {
        drops: drops.clone(),
        id: 2,
        panic: false,
    };
    let outcome = validate_capture_correspondence(
        &[1],
        &[2],
        &[3],
        limits(0, 0),
        move |_, _| {
            let _ = &group;
            Ok::<_, Infallible>(true)
        },
        move |_, _, _| {
            let _ = &result;
            Ok(true)
        },
    );
    assert!(matches!(outcome, Err(Error::BindingCount)));
    assert_eq!(*drops.borrow(), [1, 2]);
}

#[test]
fn hook_cleanup_unwind_releases_unpublished_native_error_before_handoff() {
    struct NativeError(Rc<Cell<usize>>);
    impl Drop for NativeError {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }
    let errors = Rc::new(Cell::new(0));
    let error = NativeError(errors.clone());
    let mut failure = Some(error);
    let drops = Rc::new(RefCell::new(Vec::new()));
    let owner = Owner {
        drops: drops.clone(),
        id: 1,
        panic: true,
    };
    let handed_off = Cell::new(false);
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let _ = validate_capture_correspondence(
                &[] as &[()],
                &[1],
                &[1],
                limits(1, 1),
                move |_, _| {
                    let _ = &owner;
                    Ok(true)
                },
                |_, _, _| Err(failure.take().unwrap()),
            );
            handed_off.set(true);
        }))
        .is_err()
    );
    assert!(!handed_off.get());
    assert_eq!(errors.get(), 1);
    assert_eq!(*drops.borrow(), [1]);
}

#[test]
fn gui_local_opaque_rows_use_native_policy_without_copying_or_generic_value_equality() {
    struct Widget {
        handle: Rc<Cell<u32>>,
        payload: Box<[u8]>,
    }
    let row = Widget {
        handle: Rc::new(Cell::new(7)),
        payload: vec![9].into_boxed_slice(),
    };
    let saved = Widget {
        handle: row.handle.clone(),
        payload: vec![2].into_boxed_slice(),
    };
    let original = row.payload.as_ptr();
    let rows = [row];
    let saved = [saved];
    validate_capture_correspondence(
        &[] as &[()],
        &saved,
        &rows,
        limits(1, 1),
        |_, _| panic!(),
        |_, capture, row| {
            assert_eq!(row.payload.as_ptr(), original);
            Ok::<_, Infallible>(Rc::ptr_eq(&capture.handle, &row.handle))
        },
    )
    .unwrap();
}
