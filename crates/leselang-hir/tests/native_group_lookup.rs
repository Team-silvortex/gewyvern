use std::cell::{Cell, RefCell};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;

use leselang_hir::ir::GroupKind;
use leselang_hir::native_group::{NativeGroupMemberError, NativeGroupMemberPhase};
use leselang_hir::native_group_lookup::*;

#[derive(Default)]
struct Trace {
    events: RefCell<Vec<&'static str>>,
    selected: Cell<Option<usize>>,
    drops: Cell<usize>,
    granted: Cell<bool>,
}
struct Row {
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
        secret: "private native query",
    }
}
fn lookup<'group, 'row>(
    trace: &Rc<Trace>,
    members: &'group [Member<'row>],
    kind: GroupKind,
    maximum: usize,
    query: &str,
    requested: &Row,
) -> Result<Option<&'group Member<'row>>, NativeGroupLookupError<PrivateError>> {
    lookup_native_group_member(
        kind,
        members,
        maximum,
        query,
        |member| {
            trace.events.borrow_mut().push("name");
            Ok(&member.name)
        },
        |member| {
            trace.events.borrow_mut().push("admit");
            trace
                .selected
                .set(Some(std::ptr::from_ref(member) as usize));
            Ok(trace.granted.get()
                && std::ptr::eq(member.row, requested)
                && std::ptr::eq(member.declared, requested)
                && !requested.private.is_empty())
        },
    )
}

#[test]
fn complete_once_borrowed_names_precede_one_original_selected_member_admission() {
    let trace = trace();
    let row = row();
    let members = [
        member(&trace, "first", &row),
        member(&trace, "second", &row),
        member(&trace, "third", &row),
    ];
    let query = String::from("second");
    assert_ne!(query.as_ptr(), members[1].name.as_ptr());
    let selected = lookup(&trace, &members, GroupKind::Sequence, 3, &query, &row)
        .unwrap()
        .unwrap();
    assert!(std::ptr::eq(selected, &members[1]));
    assert_eq!(
        trace.selected.get(),
        Some(std::ptr::from_ref(&members[1]) as usize)
    );
    assert_eq!(*trace.events.borrow(), ["name", "name", "name", "admit"]);
    assert_eq!(trace.drops.get(), 0);
}

#[test]
fn invalid_ceiling_query_and_arity_deny_before_name_or_selected_native_hooks() {
    let trace = trace();
    let row = row();
    let members = [member(&trace, "member", &row)];
    let failure = lookup(&trace, &members, GroupKind::Sequence, 65, "bad query", &row)
        .err()
        .unwrap();
    assert!(matches!(
        failure,
        NativeGroupLookupError::Members(NativeGroupMemberError::InvalidLimit)
    ));
    for query in ["", "bad query", "bad.name", "\u{e9}", &"a".repeat(65)] {
        assert!(matches!(
            lookup(&trace, &members, GroupKind::Sequence, 1, query, &row),
            Err(NativeGroupLookupError::InvalidQuery)
        ));
    }
    for (kind, maximum, inputs) in [
        (GroupKind::Sequence, 0, &members[..]),
        (GroupKind::Parallel, 1, &members[..]),
        (GroupKind::Sequence, 1, &members[..0]),
    ] {
        assert!(matches!(
            lookup(&trace, inputs, kind, maximum, "member", &row),
            Err(NativeGroupLookupError::Members(
                NativeGroupMemberError::Arity
            ))
        ));
    }
    assert!(trace.events.borrow().is_empty());
    assert_eq!(trace.drops.get(), 0);
}

#[test]
fn invalid_or_duplicate_tail_cannot_hide_behind_an_earlier_matching_name() {
    let trace = trace();
    let row = row();
    for bad_name in ["bad name", "first"] {
        let members = [
            member(&trace, "first", &row),
            member(&trace, "second", &row),
            member(&trace, bad_name, &row),
        ];
        trace.events.borrow_mut().clear();
        let failure = lookup(&trace, &members, GroupKind::Sequence, 3, "first", &row)
            .err()
            .unwrap();
        if bad_name == "first" {
            assert!(matches!(
                failure,
                NativeGroupLookupError::Members(NativeGroupMemberError::DuplicateName {
                    member_index: 2
                })
            ));
        } else {
            assert!(matches!(
                failure,
                NativeGroupLookupError::Members(NativeGroupMemberError::InvalidName {
                    member_index: 2
                })
            ));
        }
        assert_eq!(*trace.events.borrow(), ["name", "name", "name"]);
        assert!(trace.selected.get().is_none());
    }
}

#[test]
fn missing_names_never_query_native_rows_and_rejection_never_falls_back_to_another_slot() {
    let trace = trace();
    let foreign = row();
    let row = row();
    let members = [
        member(&trace, "first", &foreign),
        member(&trace, "second", &row),
    ];
    assert!(
        lookup(&trace, &members, GroupKind::Parallel, 2, "absent", &row)
            .unwrap()
            .is_none()
    );
    assert_eq!(*trace.events.borrow(), ["name", "name"]);
    assert!(trace.selected.get().is_none());
    trace.events.borrow_mut().clear();
    assert!(
        lookup(&trace, &members, GroupKind::Parallel, 2, "first", &row)
            .unwrap()
            .is_none()
    );
    assert_eq!(*trace.events.borrow(), ["name", "name", "admit"]);
    assert_eq!(
        trace.selected.get(),
        Some(std::ptr::from_ref(&members[0]) as usize)
    );
    assert_eq!(trace.drops.get(), 0);
}

#[test]
fn opaque_name_and_selected_errors_keep_original_phase_index_and_redacted_payloads() {
    let trace = trace();
    let row = row();
    let members = [
        member(&trace, "first", &row),
        member(&trace, "second", &row),
        member(&trace, "third", &row),
    ];
    for name_failure in [true, false] {
        let names = Cell::new(0);
        let admissions = Cell::new(0);
        let failure = lookup_native_group_member(
            GroupKind::Sequence,
            &members,
            3,
            "third",
            |member| {
                names.set(names.get() + 1);
                if name_failure && names.get() == 3 {
                    Err(error(7))
                } else {
                    Ok(&member.name)
                }
            },
            |_| {
                admissions.set(admissions.get() + 1);
                Err(error(8))
            },
        )
        .err()
        .unwrap();
        assert!(!format!("{failure:?}: {failure}").contains("private"));
        assert!(std::error::Error::source(&failure).is_none());
        let native = match failure {
            NativeGroupLookupError::Members(NativeGroupMemberError::Native {
                member_index: 2,
                phase: NativeGroupMemberPhase::Name,
                error,
            }) if name_failure => error,
            NativeGroupLookupError::Native {
                member_index: 2,
                error,
            } if !name_failure => error,
            _ => panic!(),
        };
        assert_eq!(native.marker, if name_failure { 7 } else { 8 });
        assert_eq!(native.secret, "private native query");
        assert_eq!(names.get(), 3);
        assert_eq!(admissions.get(), usize::from(!name_failure));
        assert_eq!(trace.drops.get(), 0);
    }
}

#[test]
fn native_name_and_admission_unwind_stop_without_retry_or_consuming_borrowed_members() {
    let trace = trace();
    let row = row();
    let members = [
        member(&trace, "first", &row),
        member(&trace, "second", &row),
    ];
    for name_panics in [true, false] {
        let names = Cell::new(0);
        let admissions = Cell::new(0);
        let result = catch_unwind(AssertUnwindSafe(|| {
            lookup_native_group_member(
                GroupKind::Parallel,
                &members,
                2,
                "first",
                |member| -> Result<_, PrivateError> {
                    names.set(names.get() + 1);
                    assert!(!name_panics || names.get() != 2, "private name");
                    Ok(&member.name)
                },
                |_| {
                    admissions.set(admissions.get() + 1);
                    panic!("private selected row")
                },
            )
        }));
        assert!(result.is_err());
        assert_eq!(names.get(), 2);
        assert_eq!(admissions.get(), usize::from(!name_panics));
        assert_eq!(trace.drops.get(), 0);
    }
}

#[test]
fn once_owned_admission_state_cleans_up_even_without_invocation_or_on_unwind() {
    struct Finish(Rc<Cell<usize>>, Vec<u8>);
    impl Drop for Finish {
        fn drop(&mut self) {
            assert_eq!(self.1.len(), 17);
            self.0.set(self.0.get() + 1);
        }
    }
    let members = ["member"];
    for phase in 0..7 {
        let drops = Rc::new(Cell::new(0));
        let calls = Cell::new(0);
        let finish = Finish(drops.clone(), vec![7; 17]);
        let result = catch_unwind(AssertUnwindSafe(|| {
            lookup_native_group_member(
                GroupKind::Sequence,
                &members,
                1,
                if phase == 0 {
                    "absent"
                } else if phase == 1 {
                    "bad query"
                } else {
                    "member"
                },
                |name| -> Result<_, PrivateError> {
                    assert_ne!(phase, 6, "private name");
                    if phase == 5 {
                        Err(error(10))
                    } else {
                        Ok(*name)
                    }
                },
                |_| {
                    calls.set(calls.get() + 1);
                    drop(finish);
                    if phase == 3 {
                        Err(error(9))
                    } else {
                        assert_ne!(phase, 4, "private admission");
                        Ok(true)
                    }
                },
            )
        }));
        assert_eq!(drops.get(), 1);
        assert_eq!(calls.get(), usize::from((2..=4).contains(&phase)));
        assert_eq!(result.is_err(), matches!(phase, 4 | 6));
        match result {
            Ok(Ok(None)) => assert_eq!(phase, 0),
            Ok(Err(NativeGroupLookupError::InvalidQuery)) => assert_eq!(phase, 1),
            Ok(Ok(Some(selected))) => {
                assert_eq!(phase, 2);
                assert!(std::ptr::eq(selected, &members[0]));
            }
            Ok(Err(NativeGroupLookupError::Native {
                member_index: 0, ..
            })) => {
                assert_eq!(phase, 3);
            }
            Ok(Err(NativeGroupLookupError::Members(NativeGroupMemberError::Native {
                member_index: 0,
                phase: NativeGroupMemberPhase::Name,
                ..
            }))) => assert_eq!(phase, 5),
            Err(_) => assert!(matches!(phase, 4 | 6)),
            _ => panic!(),
        }
    }
}

#[test]
fn prior_selection_does_not_certify_changed_names_limits_or_current_native_policy() {
    let trace = trace();
    let row = row();
    let mut members = [member(&trace, "member", &row)];
    assert!(
        lookup(&trace, &members, GroupKind::Sequence, 1, "member", &row)
            .unwrap()
            .is_some()
    );
    trace.granted.set(false);
    assert!(
        lookup(&trace, &members, GroupKind::Sequence, 1, "member", &row)
            .unwrap()
            .is_none()
    );
    trace.granted.set(true);
    members[0].name = "changed name".into();
    trace.events.borrow_mut().clear();
    assert!(lookup(&trace, &members, GroupKind::Sequence, 1, "member", &row).is_err());
    assert_eq!(*trace.events.borrow(), ["name"]);
    members[0].name = "member".into();
    trace.events.borrow_mut().clear();
    assert!(lookup(&trace, &members, GroupKind::Sequence, 0, "member", &row).is_err());
    assert!(trace.events.borrow().is_empty());
}

#[test]
fn inclusive_ceiling_shared_label_grammar_and_unsized_gui_local_slots_need_no_native_traits() {
    let trace = trace();
    let row = row();
    let members = (0..64)
        .map(|index| member(&trace, &format!("member_{index}"), &row))
        .collect::<Vec<_>>();
    let selected = lookup(&trace, &members, GroupKind::Sequence, 64, "member_63", &row)
        .unwrap()
        .unwrap();
    assert!(std::ptr::eq(selected, &members[63]));
    assert_eq!(trace.events.borrow().len(), 65);
    trait GuiOperation {
        fn token(&self) -> u8;
    }
    struct Local(Rc<Cell<u8>>);
    impl GuiOperation for Local {
        fn token(&self) -> u8 {
            self.0.get()
        }
    }
    let local = Local(Rc::new(Cell::new(7)));
    let gui: [(&str, &dyn GuiOperation); 3] = [("1", &local), ("while", &local), ("all-4", &local)];
    for query in ["1", "while", "all-4"] {
        let selected = lookup_native_group_member(
            GroupKind::Sequence,
            &gui,
            3,
            query,
            |member| -> Result<_, PrivateError> { Ok(member.0) },
            |member| {
                Ok(std::ptr::eq(member.1, &local as &dyn GuiOperation) && member.1.token() == 7)
            },
        )
        .unwrap()
        .unwrap();
        assert_eq!(selected.0, query);
    }
    assert_eq!(Rc::strong_count(&local.0), 1);
}

#[test]
fn explicit_native_observations_are_not_automatic_original_name_row_or_grant_authenticity() {
    let trace = trace();
    let foreign = row();
    let row = row();
    let members = [member(&trace, "original", &foreign)];
    assert!(
        lookup(&trace, &members, GroupKind::Sequence, 1, "query", &row)
            .unwrap()
            .is_none()
    );
    let selected = lookup_native_group_member(
        GroupKind::Sequence,
        &members,
        1,
        "query",
        |_| -> Result<_, PrivateError> { Ok("query") },
        |_| Ok(true),
    )
    .unwrap()
    .unwrap();
    assert!(std::ptr::eq(selected, &members[0]));
    assert_ne!(selected.name, "query");
    assert!(!std::ptr::eq(selected.row, &row));
    trace.granted.set(false);
    assert_eq!(trace.drops.get(), 0);
}
