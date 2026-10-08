use std::cell::{Cell, RefCell};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;

use leselang_hir::ir::GroupKind;
use leselang_hir::native_group::{NativeGroupMemberError, NativeGroupMemberPhase};
use leselang_hir::native_group_signature::*;

#[derive(Default)]
struct Trace {
    events: RefCell<Vec<&'static str>>,
    pairs: RefCell<Vec<(usize, usize)>>,
    drops: Cell<usize>,
    granted: Cell<bool>,
}
struct Row {
    token: u8,
    private: Vec<u8>,
}
struct Member<'row> {
    name: String,
    row: &'row Row,
    declared: &'row Row,
    owner: Rc<Trace>,
}
impl Drop for Member<'_> {
    fn drop(&mut self) {
        self.owner.drops.set(self.owner.drops.get() + 1);
    }
}
struct PrivateError {
    marker: u8,
    secret: &'static str,
}
fn trace() -> Rc<Trace> {
    Rc::new(Trace {
        granted: Cell::new(true),
        ..Trace::default()
    })
}
fn row() -> Row {
    Row {
        token: 3,
        private: vec![7, 11],
    }
}
fn member<'row>(trace: &Rc<Trace>, name: &str, row: &'row Row) -> Member<'row> {
    Member {
        name: name.into(),
        row,
        declared: row,
        owner: trace.clone(),
    }
}
fn error(marker: u8) -> PrivateError {
    PrivateError {
        marker,
        secret: "private native signature",
    }
}
fn compare(
    trace: &Rc<Trace>,
    left: (GroupKind, &[Member<'_>]),
    right: (GroupKind, &[Member<'_>]),
    maximum: usize,
) -> Result<bool, NativeGroupSignatureError<PrivateError>> {
    compare_native_group_signatures(
        left,
        right,
        maximum,
        |member| {
            trace.events.borrow_mut().push("left-name");
            Ok(&member.name)
        },
        |member| {
            trace.events.borrow_mut().push("right-name");
            Ok(&member.name)
        },
        |left, right| {
            trace.events.borrow_mut().push("pair");
            trace.pairs.borrow_mut().push((
                std::ptr::from_ref(left) as usize,
                std::ptr::from_ref(right) as usize,
            ));
            Ok(trace.granted.get()
                && std::ptr::eq(left.row, right.row)
                && std::ptr::eq(left.declared, right.declared)
                && std::ptr::eq(left.row, left.declared)
                && std::ptr::eq(right.row, right.declared)
                && left.row.token == 3
                && !left.row.private.is_empty())
        },
    )
}

#[test]
fn both_complete_once_borrowed_names_precede_original_pairs_in_declaration_order() {
    let trace = trace();
    let row = row();
    let left = [
        member(&trace, "first", &row),
        member(&trace, "second", &row),
    ];
    let right = [
        member(&trace, "first", &row),
        member(&trace, "second", &row),
    ];
    assert_ne!(left[0].name.as_ptr(), right[0].name.as_ptr());
    assert!(
        compare(
            &trace,
            (GroupKind::Parallel, &left),
            (GroupKind::Parallel, &right),
            2
        )
        .unwrap()
    );
    assert_eq!(
        *trace.events.borrow(),
        [
            "left-name",
            "left-name",
            "right-name",
            "right-name",
            "pair",
            "pair"
        ]
    );
    assert_eq!(
        *trace.pairs.borrow(),
        left.iter()
            .zip(&right)
            .map(|(left, right)| {
                (
                    std::ptr::from_ref(left) as usize,
                    std::ptr::from_ref(right) as usize,
                )
            })
            .collect::<Vec<_>>()
    );
    assert_eq!(trace.drops.get(), 0);
}

#[test]
fn valid_kind_count_and_name_order_mismatches_never_enter_native_comparison() {
    let trace = trace();
    let row = row();
    let left = [
        member(&trace, "first", &row),
        member(&trace, "second", &row),
    ];
    for (kind, names) in [
        (GroupKind::Parallel, vec!["first", "second"]),
        (GroupKind::Sequence, vec!["first"]),
        (GroupKind::Sequence, vec!["second", "first"]),
        (GroupKind::Sequence, vec!["first", "changed"]),
    ] {
        let right = names
            .iter()
            .map(|name| member(&trace, name, &row))
            .collect::<Vec<_>>();
        trace.events.borrow_mut().clear();
        assert!(!compare(&trace, (GroupKind::Sequence, &left), (kind, &right), 2).unwrap());
        assert_eq!(trace.events.borrow().len(), left.len() + right.len());
        assert!(trace.pairs.borrow().is_empty());
    }
}

#[test]
fn limits_and_invalid_members_report_original_side_before_any_native_pair() {
    let trace = trace();
    let row = row();
    let good = [member(&trace, "first", &row)];
    for maximum in [0, 65] {
        assert!(matches!(
            compare(
                &trace,
                (GroupKind::Sequence, &good),
                (GroupKind::Sequence, &good),
                maximum
            ),
            Err(NativeGroupSignatureError::Members {
                side: NativeGroupSignatureSide::Left,
                ..
            })
        ));
        assert!(trace.events.borrow().is_empty());
    }
    for (side, names) in [
        (NativeGroupSignatureSide::Left, vec![]),
        (NativeGroupSignatureSide::Right, vec![]),
        (NativeGroupSignatureSide::Right, vec!["first", "bad name"]),
        (NativeGroupSignatureSide::Right, vec!["first", "first"]),
    ] {
        let bad = names
            .iter()
            .map(|name| member(&trace, name, &row))
            .collect::<Vec<_>>();
        trace.events.borrow_mut().clear();
        let (left, right) = if side == NativeGroupSignatureSide::Left {
            (&bad[..], &good[..])
        } else {
            (&good[..], &bad[..])
        };
        let failure = compare(
            &trace,
            (GroupKind::Sequence, left),
            (GroupKind::Sequence, right),
            2,
        )
        .unwrap_err();
        let NativeGroupSignatureError::Members {
            side: actual,
            error,
        } = failure
        else {
            panic!()
        };
        assert_eq!(actual, side);
        match names.as_slice() {
            [] => assert!(matches!(error, NativeGroupMemberError::Arity)),
            [_, "bad name"] => assert!(matches!(
                error,
                NativeGroupMemberError::InvalidName { member_index: 1 }
            )),
            [_, "first"] => assert!(matches!(
                error,
                NativeGroupMemberError::DuplicateName { member_index: 1 }
            )),
            _ => panic!(),
        }
        assert!(trace.pairs.borrow().is_empty());
    }
}

#[test]
fn opaque_name_errors_keep_side_index_and_payload_without_formatting_or_sources() {
    let trace = trace();
    let row = row();
    let members = [
        member(&trace, "first", &row),
        member(&trace, "second", &row),
    ];
    for failing_side in [
        NativeGroupSignatureSide::Left,
        NativeGroupSignatureSide::Right,
    ] {
        let left_names = Cell::new(0);
        let right_names = Cell::new(0);
        let pairs = Cell::new(0);
        let failure = compare_native_group_signatures(
            (GroupKind::Parallel, &members),
            (GroupKind::Parallel, &members),
            2,
            |member| {
                left_names.set(left_names.get() + 1);
                if failing_side == NativeGroupSignatureSide::Left && left_names.get() == 2 {
                    Err(error(7))
                } else {
                    Ok(&member.name)
                }
            },
            |member| {
                right_names.set(right_names.get() + 1);
                if failing_side == NativeGroupSignatureSide::Right && right_names.get() == 2 {
                    Err(error(8))
                } else {
                    Ok(&member.name)
                }
            },
            |_, _| {
                pairs.set(pairs.get() + 1);
                Ok(true)
            },
        )
        .unwrap_err();
        assert!(!format!("{failure:?}: {failure}").contains("private"));
        assert!(std::error::Error::source(&failure).is_none());
        let NativeGroupSignatureError::Members {
            side,
            error:
                NativeGroupMemberError::Native {
                    member_index: 1,
                    phase: NativeGroupMemberPhase::Name,
                    error,
                },
        } = failure
        else {
            panic!()
        };
        assert_eq!(side, failing_side);
        assert_eq!(
            error.marker,
            if failing_side == NativeGroupSignatureSide::Left {
                7
            } else {
                8
            }
        );
        assert_eq!(error.secret, "private native signature");
        assert_eq!(left_names.get(), 2);
        assert_eq!(
            right_names.get(),
            if failing_side == NativeGroupSignatureSide::Left {
                0
            } else {
                2
            }
        );
        assert_eq!(pairs.get(), 0);
        assert_eq!(trace.drops.get(), 0);
    }
}

#[test]
fn false_and_opaque_pair_errors_stop_once_at_the_original_member_index() {
    let names = ["first", "second", "third"];
    for native_error in [false, true] {
        let pairs = Cell::new(0);
        let result = compare_native_group_signatures(
            (GroupKind::Sequence, &names),
            (GroupKind::Sequence, &names),
            3,
            |name| Ok(*name),
            |name| Ok(*name),
            |_, _| {
                pairs.set(pairs.get() + 1);
                if pairs.get() == 2 {
                    if native_error {
                        Err(error(9))
                    } else {
                        Ok(false)
                    }
                } else {
                    Ok(true)
                }
            },
        );
        if native_error {
            let failure = result.unwrap_err();
            assert!(std::error::Error::source(&failure).is_none());
            assert!(!format!("{failure:?}: {failure}").contains("private"));
            assert!(matches!(
                failure,
                NativeGroupSignatureError::Native {
                    member_index: 1,
                    error: PrivateError { marker: 9, .. }
                }
            ));
        } else {
            assert!(!result.unwrap());
        }
        assert_eq!(pairs.get(), 2);
    }
}

#[test]
fn native_name_and_pair_unwind_stop_without_retry_or_consuming_borrowed_members() {
    let trace = trace();
    let row = row();
    let members = [
        member(&trace, "first", &row),
        member(&trace, "second", &row),
    ];
    for phase in 0..3 {
        let left_names = Cell::new(0);
        let right_names = Cell::new(0);
        let pairs = Cell::new(0);
        let result = catch_unwind(AssertUnwindSafe(|| {
            compare_native_group_signatures(
                (GroupKind::Parallel, &members),
                (GroupKind::Parallel, &members),
                2,
                |member| -> Result<_, PrivateError> {
                    left_names.set(left_names.get() + 1);
                    assert!(phase != 0 || left_names.get() != 2, "private left name");
                    Ok(&member.name)
                },
                |member| {
                    right_names.set(right_names.get() + 1);
                    assert!(phase != 1 || right_names.get() != 2, "private right name");
                    Ok(&member.name)
                },
                |_, _| {
                    pairs.set(pairs.get() + 1);
                    assert!(phase != 2 || pairs.get() != 2, "private native pair");
                    Ok(true)
                },
            )
        }));
        assert!(result.is_err());
        assert_eq!(left_names.get(), 2);
        assert_eq!(right_names.get(), if phase == 0 { 0 } else { 2 });
        assert_eq!(pairs.get(), if phase == 2 { 2 } else { 0 });
        assert_eq!(trace.drops.get(), 0);
    }
}

#[test]
fn fresh_current_names_limits_and_live_row_policy_cannot_reuse_a_prior_true_result() {
    let trace = trace();
    let row = row();
    let mut members = [member(&trace, "first", &row)];
    assert!(
        compare(
            &trace,
            (GroupKind::Sequence, &members),
            (GroupKind::Sequence, &members),
            1
        )
        .unwrap()
    );
    trace.granted.set(false);
    assert!(
        !compare(
            &trace,
            (GroupKind::Sequence, &members),
            (GroupKind::Sequence, &members),
            1
        )
        .unwrap()
    );
    trace.granted.set(true);
    members[0].name = "changed name".into();
    trace.events.borrow_mut().clear();
    assert!(
        compare(
            &trace,
            (GroupKind::Sequence, &members),
            (GroupKind::Sequence, &members),
            1
        )
        .is_err()
    );
    assert_eq!(*trace.events.borrow(), ["left-name"]);
    members[0].name = "first".into();
    trace.events.borrow_mut().clear();
    assert!(
        compare(
            &trace,
            (GroupKind::Sequence, &members),
            (GroupKind::Sequence, &members),
            0
        )
        .is_err()
    );
    assert!(trace.events.borrow().is_empty());
}

#[test]
fn aliased_slices_still_query_both_sides_and_every_current_native_pair() {
    let trace = trace();
    let row = row();
    let members = (0..64)
        .map(|index| member(&trace, &format!("m_{index}"), &row))
        .collect::<Vec<_>>();
    assert!(
        compare(
            &trace,
            (GroupKind::Sequence, &members),
            (GroupKind::Sequence, &members),
            64
        )
        .unwrap()
    );
    assert_eq!(trace.events.borrow().len(), 192);
    assert_eq!(trace.pairs.borrow().len(), 64);
    assert!(
        trace
            .pairs
            .borrow()
            .iter()
            .all(|(left, right)| left == right)
    );
    assert_eq!(trace.drops.get(), 0);
    let names = ["1", "while", "all-4"];
    assert!(
        compare_native_group_signatures(
            (GroupKind::Sequence, &names),
            (GroupKind::Sequence, &names),
            3,
            |name| -> Result<_, ()> { Ok(*name) },
            |name| Ok(*name),
            |_, _| Ok(true),
        )
        .unwrap()
    );
}

#[test]
fn heterogeneous_unsized_gui_and_device_members_need_no_native_clone_equality_or_wire_traits() {
    trait GuiOperation {
        fn token(&self) -> u128;
    }
    struct Local(Rc<Cell<u128>>);
    impl GuiOperation for Local {
        fn token(&self) -> u128 {
            self.0.get()
        }
    }
    struct GuiMember<'a> {
        name: String,
        operation: &'a dyn GuiOperation,
    }
    struct Device {
        token: u128,
        private: Vec<u8>,
    }
    let local = Local(Rc::new(Cell::new(97)));
    let gui = [GuiMember {
        name: "move".into(),
        operation: &local,
    }];
    let device = [(
        String::from("move"),
        Device {
            token: 97,
            private: vec![1],
        },
    )];
    assert!(
        compare_native_group_signatures(
            (GroupKind::Sequence, &gui),
            (GroupKind::Sequence, &device),
            1,
            |member| -> Result<_, PrivateError> { Ok(&member.name) },
            |member| Ok(&member.0),
            |gui, device| Ok(
                gui.operation.token() == device.1.token && !device.1.private.is_empty()
            ),
        )
        .unwrap()
    );
    assert_eq!(Rc::strong_count(&local.0), 1);
}

#[test]
fn explicit_native_comparison_is_not_automatic_schema_identity_or_future_authority() {
    let trace = trace();
    let foreign = row();
    let row = row();
    let left = [member(&trace, "member", &row)];
    let mut right = [member(&trace, "member", &foreign)];
    assert!(
        !compare(
            &trace,
            (GroupKind::Sequence, &left),
            (GroupKind::Sequence, &right),
            1
        )
        .unwrap()
    );
    assert!(
        compare_native_group_signatures(
            (GroupKind::Sequence, &left),
            (GroupKind::Sequence, &right),
            1,
            |member| -> Result<_, PrivateError> { Ok(&member.name) },
            |member| Ok(&member.name),
            |_, _| Ok(true),
        )
        .unwrap()
    );
    right[0].declared = &row;
    assert!(
        !compare(
            &trace,
            (GroupKind::Sequence, &left),
            (GroupKind::Sequence, &right),
            1
        )
        .unwrap()
    );
    assert_eq!(trace.drops.get(), 0);
}
