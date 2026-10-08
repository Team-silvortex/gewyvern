use std::cell::{Cell, RefCell};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;

use leselang_hir::ir::GroupKind;
use leselang_hir::native_graph::{NativeGraphLimits, NativeGraphShape, inspect_native_graph};
use leselang_hir::native_group::*;

#[derive(Default)]
struct Trace {
    events: RefCell<Vec<(&'static str, usize)>>,
    granted: Cell<bool>,
    drops: Cell<usize>,
}
struct Declaration {
    tag: u8,
    private: Vec<u8>,
}
struct Branch<'row> {
    name: String,
    row: &'row Declaration,
    declared: &'row Declaration,
    nested: bool,
    owner: Rc<Trace>,
}
impl Drop for Branch<'_> {
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
fn row(tag: u8) -> Declaration {
    Declaration {
        tag,
        private: vec![17, 31],
    }
}
fn branch<'row>(trace: &Rc<Trace>, name: &str, row: &'row Declaration) -> Branch<'row> {
    Branch {
        name: name.into(),
        row,
        declared: row,
        nested: false,
        owner: trace.clone(),
    }
}
fn error(marker: u8) -> PrivateError {
    PrivateError {
        marker,
        secret: "private native schema and GUI input",
    }
}
fn admit<'group, 'row>(
    trace: &Trace,
    kind: GroupKind,
    members: &'group [Branch<'row>],
    maximum: usize,
) -> Result<NativeGroupMembers<'group, Branch<'row>>, NativeGroupMemberError<PrivateError>> {
    admit_native_group_members(
        kind,
        members,
        maximum,
        |member| {
            trace
                .events
                .borrow_mut()
                .push(("name", std::ptr::from_ref(member) as usize));
            Ok(member.name.as_str())
        },
        |member| {
            trace
                .events
                .borrow_mut()
                .push(("admit", std::ptr::from_ref(member) as usize));
            if trace.granted.get()
                && !member.nested
                && std::ptr::eq(member.row, member.declared)
                && member.row.tag == 3
                && !member.row.private.is_empty()
            {
                Ok(())
            } else {
                Err(error(9))
            }
        },
    )
}

#[test]
fn original_nonclone_names_branch_slots_and_modes_are_borrowed_in_declaration_order() {
    let trace = trace();
    let row = row(3);
    let members = [
        branch(&trace, "first", &row),
        branch(&trace, "second", &row),
    ];
    for kind in [GroupKind::Sequence, GroupKind::Parallel] {
        trace.events.borrow_mut().clear();
        let view = admit(&trace, kind, &members, 2).unwrap();
        assert_eq!(view.kind(), kind);
        assert!(std::ptr::eq(view.members(), &members[..]));
        for (index, member) in members.iter().enumerate() {
            assert_eq!(view.name(index).unwrap().as_ptr(), member.name.as_ptr());
            assert!(std::ptr::eq(&view.members()[index], member));
        }
        assert_eq!(view.name(2), None);
        assert_eq!(view.name(64), None);
        assert_eq!(trace.drops.get(), 0);
        assert_eq!(
            *trace.events.borrow(),
            [
                ("name", std::ptr::from_ref(&members[0]) as usize),
                ("name", std::ptr::from_ref(&members[1]) as usize),
                ("admit", std::ptr::from_ref(&members[0]) as usize),
                ("admit", std::ptr::from_ref(&members[1]) as usize),
            ]
        );
        let formatted = format!("{view:?}");
        assert!(!formatted.contains("first"));
        assert!(!formatted.contains("second"));
    }
}

#[test]
fn inclusive_arity_zero_and_safety_ceiling_deny_before_any_hook() {
    let trace = trace();
    let row = row(3);
    for (kind, count, maximum, invalid_limit) in [
        (GroupKind::Sequence, 0, 64, false),
        (GroupKind::Parallel, 1, 64, false),
        (GroupKind::Sequence, 1, 0, false),
        (GroupKind::Sequence, 2, 1, false),
        (GroupKind::Parallel, 65, 64, false),
        (GroupKind::Sequence, 0, 65, true),
        (GroupKind::Sequence, 1, usize::MAX, true),
    ] {
        let members = (0..count)
            .map(|index| branch(&trace, &format!("m_{index}"), &row))
            .collect::<Vec<_>>();
        trace.events.borrow_mut().clear();
        let result = admit(&trace, kind, &members, maximum);
        if invalid_limit {
            assert!(matches!(result, Err(NativeGroupMemberError::InvalidLimit)));
        } else {
            assert!(matches!(result, Err(NativeGroupMemberError::Arity)));
        }
        assert!(trace.events.borrow().is_empty());
    }
    for (kind, count) in [
        (GroupKind::Sequence, 1),
        (GroupKind::Parallel, 2),
        (GroupKind::Parallel, 64),
    ] {
        let members = (0..count)
            .map(|index| branch(&trace, &format!("m_{index}"), &row))
            .collect::<Vec<_>>();
        let view = admit(&trace, kind, &members, count).unwrap();
        assert_eq!(view.members().len(), count);
        assert_eq!(view.name(count - 1).unwrap(), members[count - 1].name);
        assert_eq!(view.name(count), None);
    }
}

#[test]
fn all_names_are_checked_before_any_native_admission_even_if_first_native_row_is_invalid() {
    let trace = trace();
    let row = row(3);
    for name in [
        "",
        "bad name",
        "with.dot",
        "with/slash",
        "\u{00e9}",
        &"x".repeat(65),
    ] {
        let mut first = branch(&trace, "first", &row);
        first.nested = true;
        let members = [first, branch(&trace, name, &row)];
        trace.events.borrow_mut().clear();
        assert!(matches!(
            admit(&trace, GroupKind::Parallel, &members, 2),
            Err(NativeGroupMemberError::InvalidName { member_index: 1 })
        ));
        assert_eq!(
            trace
                .events
                .borrow()
                .iter()
                .map(|(phase, _)| *phase)
                .collect::<Vec<_>>(),
            ["name", "name"]
        );
    }
}

#[test]
fn labels_follow_member_not_lexical_grammar_and_original_buffers_never_copy() {
    let trace = trace();
    let row = row(3);
    let maximum_name = "a".repeat(64);
    let members = [
        "0",
        "fn",
        "none",
        "body",
        "with-dash",
        "_",
        maximum_name.as_str(),
    ]
    .map(|name| branch(&trace, name, &row));
    let view = admit(&trace, GroupKind::Sequence, &members, 7).unwrap();
    for (index, member) in members.iter().enumerate() {
        assert_eq!(view.name(index).unwrap().as_ptr(), member.name.as_ptr());
    }
}

#[test]
fn duplicate_equal_but_distinct_native_buffers_are_rejected_without_native_queries() {
    let trace = trace();
    let row = row(3);
    let members = [branch(&trace, "same", &row), branch(&trace, "same", &row)];
    assert_ne!(members[0].name.as_ptr(), members[1].name.as_ptr());
    assert!(matches!(
        admit(&trace, GroupKind::Parallel, &members, 2),
        Err(NativeGroupMemberError::DuplicateName { member_index: 1 })
    ));
    assert_eq!(trace.events.borrow().len(), 2);
    assert!(
        trace
            .events
            .borrow()
            .iter()
            .all(|(phase, _)| *phase == "name")
    );
}

#[test]
fn native_name_and_admission_errors_keep_original_phase_index_payload_without_formatting() {
    let trace = trace();
    let row = row(3);
    let members = [
        branch(&trace, "first", &row),
        branch(&trace, "second", &row),
    ];
    for phase in [
        NativeGroupMemberPhase::Name,
        NativeGroupMemberPhase::Admission,
    ] {
        let names = Cell::new(0);
        let admissions = Cell::new(0);
        let result = admit_native_group_members(
            GroupKind::Parallel,
            &members,
            2,
            |member| {
                names.set(names.get() + 1);
                if phase == NativeGroupMemberPhase::Name && names.get() == 2 {
                    Err(error(7))
                } else {
                    Ok(member.name.as_str())
                }
            },
            |_| {
                admissions.set(admissions.get() + 1);
                if phase == NativeGroupMemberPhase::Admission && admissions.get() == 2 {
                    Err(error(7))
                } else {
                    Ok(())
                }
            },
        );
        let failure = result.unwrap_err();
        assert!(!format!("{failure:?}: {failure}").contains("private"));
        assert!(std::error::Error::source(&failure).is_none());
        let NativeGroupMemberError::Native {
            member_index,
            phase: actual,
            error,
        } = failure
        else {
            panic!()
        };
        assert_eq!(member_index, 1);
        assert_eq!(actual, phase);
        assert_eq!(error.marker, 7);
        assert_eq!(error.secret, "private native schema and GUI input");
        assert_eq!(names.get(), 2);
        assert_eq!(
            admissions.get(),
            if phase == NativeGroupMemberPhase::Name {
                0
            } else {
                2
            }
        );
        assert_eq!(trace.drops.get(), 0);
    }
}

#[test]
fn native_unwind_stops_once_keeps_graphs_and_never_returns_partial_views() {
    let trace = trace();
    let row = row(3);
    let members = [
        branch(&trace, "first", &row),
        branch(&trace, "second", &row),
    ];
    for phase in [
        NativeGroupMemberPhase::Name,
        NativeGroupMemberPhase::Admission,
    ] {
        let names = Cell::new(0);
        let admissions = Cell::new(0);
        let result = catch_unwind(AssertUnwindSafe(|| {
            admit_native_group_members(
                GroupKind::Parallel,
                &members,
                2,
                |member| -> Result<_, PrivateError> {
                    names.set(names.get() + 1);
                    assert!(
                        phase != NativeGroupMemberPhase::Name || names.get() != 2,
                        "private name callback"
                    );
                    Ok(member.name.as_str())
                },
                |_| {
                    admissions.set(admissions.get() + 1);
                    assert!(
                        phase != NativeGroupMemberPhase::Admission || admissions.get() != 2,
                        "private native callback"
                    );
                    Ok(())
                },
            )
        }));
        assert!(result.is_err());
        assert_eq!(names.get(), 2);
        assert_eq!(
            admissions.get(),
            if phase == NativeGroupMemberPhase::Name {
                0
            } else {
                2
            }
        );
        assert_eq!(trace.drops.get(), 0);
        assert!(admit(&trace, GroupKind::Parallel, &members, 2).is_ok());
    }
}

#[test]
fn current_native_policy_type_identity_and_atomic_eligibility_are_not_inferred_by_the_core() {
    let trace = trace();
    let row = row(3);
    let lookalike = Declaration {
        tag: row.tag,
        private: row.private.clone(),
    };
    let wrong = Declaration {
        tag: 2,
        private: vec![31],
    };
    for scenario in 0..4 {
        let mut member = branch(&trace, "first", &row);
        match scenario {
            0 => member.declared = &lookalike,
            1 => {
                member.row = &wrong;
                member.declared = &wrong;
            }
            2 => member.nested = true,
            _ => trace.granted.set(false),
        }
        let members = [member];
        trace.events.borrow_mut().clear();
        assert!(matches!(
            admit(&trace, GroupKind::Sequence, &members, 1),
            Err(NativeGroupMemberError::Native {
                member_index: 0,
                phase: NativeGroupMemberPhase::Admission,
                ..
            })
        ));
        assert_eq!(trace.events.borrow().len(), 2);
        trace.granted.set(true);
    }
}

#[test]
fn successful_view_does_not_certify_changed_live_policy_or_hidden_payload_limits() {
    let trace = trace();
    let row = row(3);
    let members = [branch(&trace, "first", &row)];
    let view = admit(&trace, GroupKind::Sequence, &members, 1).unwrap();
    trace.granted.set(false);
    assert!(std::ptr::eq(view.members(), &members[..]));
    assert!(admit(&trace, GroupKind::Sequence, view.members(), 1).is_err());
    assert_eq!(view.name(0), Some("first"));
    let unconstrained = [vec![0u8; 4096]];
    let opaque = admit_native_group_members(
        GroupKind::Sequence,
        &unconstrained,
        1,
        |_| -> Result<_, ()> { Ok("private") },
        |_| Ok(()),
    )
    .unwrap();
    assert_eq!(opaque.members()[0].len(), 4096);
}

#[test]
fn callback_observed_names_are_ephemeral_native_metadata_not_automatic_authenticity() {
    let trace = trace();
    let row = row(3);
    let members = [branch(&trace, "original", &row)];
    let supplied = String::from("explicit_native_name");
    let view = admit_native_group_members(
        GroupKind::Sequence,
        &members,
        1,
        |_| -> Result<_, ()> { Ok(supplied.as_str()) },
        |_| Ok(()),
    )
    .unwrap();
    assert_eq!(view.name(0).unwrap().as_ptr(), supplied.as_ptr());
    assert_ne!(view.name(0), Some(members[0].name.as_str()));
    let rejected = admit_native_group_members(
        GroupKind::Sequence,
        &members,
        1,
        |_| -> Result<_, ()> { Ok(supplied.as_str()) },
        |member| {
            if supplied == member.name {
                Ok(())
            } else {
                Err(())
            }
        },
    );
    assert!(rejected.is_err());
}

#[test]
fn shared_member_admission_composes_with_graph_preflight_but_never_replaces_its_budget() {
    struct Graph {
        members: Vec<Graph>,
        name: String,
    }
    let graph = Graph {
        name: "root".into(),
        members: vec![Graph {
            name: "one".into(),
            members: vec![],
        }],
    };
    let admissions = Cell::new(0);
    let run = |nodes| -> Result<(), ()> {
        let _counts = inspect_native_graph(
            &graph,
            NativeGraphLimits {
                max_nodes: nodes,
                max_depth: 1,
                max_members: 1,
            },
            |node| -> Result<_, ()> {
                Ok(if node.members.is_empty() {
                    NativeGraphShape::Leaf
                } else {
                    NativeGraphShape::Group {
                        kind: GroupKind::Sequence,
                        members: &node.members,
                    }
                })
            },
            Ok,
            |_| Ok(()),
        )
        .map_err(|_| ())?;
        let _view = admit_native_group_members(
            GroupKind::Sequence,
            &graph.members,
            1,
            |member| Ok(member.name.as_str()),
            |_| {
                admissions.set(admissions.get() + 1);
                Ok(())
            },
        )
        .map_err(|_: NativeGroupMemberError<()>| ())?;
        Ok(())
    };
    assert!(run(1).is_err());
    assert_eq!(admissions.get(), 0);
    run(2).unwrap();
    assert_eq!(admissions.get(), 1);
}

#[test]
fn unsized_gui_operation_views_and_move_only_branches_need_no_box_conversion_or_thread_transfer() {
    trait GuiRow {
        fn tag(&self) -> u8;
    }
    struct Local(Rc<Cell<u8>>);
    impl GuiRow for Local {
        fn tag(&self) -> u8 {
            self.0.get()
        }
    }
    struct Member<'a> {
        name: String,
        operation: &'a dyn GuiRow,
    }
    let local = Local(Rc::new(Cell::new(4)));
    let members = [Member {
        name: "gui".into(),
        operation: &local,
    }];
    let view = admit_native_group_members(
        GroupKind::Sequence,
        &members,
        1,
        |member| -> Result<_, PrivateError> { Ok(&member.name) },
        |member| {
            if member.operation.tag() == 4 {
                Ok(())
            } else {
                Err(error(4))
            }
        },
    )
    .unwrap();
    assert!(std::ptr::eq(
        view.members()[0].operation,
        &local as &dyn GuiRow
    ));
    assert_eq!(Rc::strong_count(&local.0), 1);
    local.0.set(5);
    assert_eq!(view.members()[0].operation.tag(), 5);
}
