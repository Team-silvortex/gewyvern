use std::cell::{Cell, RefCell};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;

use leselang_hir::group_exports::GroupExports;
use leselang_hir::ir::GroupKind;
use leselang_hir::native_group::{NativeGroupMemberError, NativeGroupMemberPhase};
use leselang_hir::native_group_exports::*;

#[derive(Default)]
struct Trace {
    events: RefCell<Vec<&'static str>>,
    observations_dropped: Cell<usize>,
    branches_dropped: Cell<usize>,
    granted: Cell<bool>,
    panic_drop: Cell<bool>,
}
struct Row {
    tag: u8,
    private: Vec<u8>,
}
struct Branch<'row> {
    name: String,
    row: &'row Row,
    declared: &'row Row,
    nested: bool,
    owner: Rc<Trace>,
}
impl Drop for Branch<'_> {
    fn drop(&mut self) {
        self.owner
            .branches_dropped
            .set(self.owner.branches_dropped.get() + 1);
    }
}
struct Observation<'row> {
    row: &'row Row,
    branch: usize,
    payload: Vec<u8>,
    owner: Rc<Trace>,
}
impl Drop for Observation<'_> {
    fn drop(&mut self) {
        self.owner
            .observations_dropped
            .set(self.owner.observations_dropped.get() + 1);
        assert!(
            !self.owner.panic_drop.replace(false),
            "private observation destructor"
        );
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
        tag: 3,
        private: vec![7, 11],
    }
}
fn branch<'row>(trace: &Rc<Trace>, name: &str, row: &'row Row) -> Branch<'row> {
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
        secret: "private GUI schema and input",
    }
}
fn observation<'row>(trace: &Rc<Trace>, branch: &Branch<'row>) -> Observation<'row> {
    Observation {
        row: branch.row,
        branch: std::ptr::from_ref(branch) as usize,
        payload: vec![31, 41],
        owner: trace.clone(),
    }
}
fn assemble<'group, 'row>(
    trace: &Rc<Trace>,
    kind: GroupKind,
    members: &'group [Branch<'row>],
    maximum: usize,
) -> Result<GroupExports<'group, Observation<'row>>, NativeGroupExportError<PrivateError>> {
    observe_native_group_exports(
        kind,
        members,
        maximum,
        |member| {
            trace.events.borrow_mut().push("name");
            Ok(member.name.as_str())
        },
        |member| {
            trace.events.borrow_mut().push("operation");
            Ok(observation(trace, member))
        },
        |candidate_kind, original, candidate| {
            trace.events.borrow_mut().push("signature");
            if trace.granted.get()
                && candidate_kind == kind
                && original.len() == candidate.len()
                && original.iter().zip(candidate).all(|(member, exported)| {
                    std::ptr::eq(member.name.as_str(), exported.name)
                        && !member.nested
                        && member.row.tag == 3
                        && !member.row.private.is_empty()
                        && std::ptr::eq(member.row, member.declared)
                        && std::ptr::eq(member.row, exported.operation.row)
                        && exported.operation.branch == std::ptr::from_ref(member) as usize
                })
            {
                Ok(())
            } else {
                Err(error(9))
            }
        },
    )
}

#[test]
fn fresh_names_original_branch_order_and_move_only_observations_reach_one_complete_signature() {
    let trace = trace();
    let row = row();
    let members = [
        branch(&trace, "first", &row),
        branch(&trace, "second", &row),
    ];
    let exports = assemble(&trace, GroupKind::Parallel, &members, 2).unwrap();
    assert_eq!(exports.kind, GroupKind::Parallel);
    assert_eq!(
        *trace.events.borrow(),
        ["name", "name", "operation", "operation", "signature"]
    );
    for (member, exported) in members.iter().zip(&exports.members) {
        assert_eq!(member.name.as_ptr(), exported.name.as_ptr());
        assert!(std::ptr::eq(member.row, exported.operation.row));
        assert_eq!(
            exported.operation.branch,
            std::ptr::from_ref(member) as usize
        );
        assert_eq!(exported.operation.payload, [31, 41]);
    }
    assert_eq!(trace.observations_dropped.get(), 0);
    assert_eq!(trace.branches_dropped.get(), 0);
    let formatted = format!("{exports:?}");
    assert!(!formatted.contains("first"));
    assert!(!formatted.contains("private"));
    drop(exports);
    assert_eq!(trace.observations_dropped.get(), 2);
    assert_eq!(trace.branches_dropped.get(), 0);
}

#[test]
fn current_limits_arity_invalid_and_duplicate_names_precede_any_native_operation() {
    let trace = trace();
    let row = row();
    for (kind, names, maximum) in [
        (GroupKind::Sequence, vec![], 64),
        (GroupKind::Parallel, vec!["one"], 64),
        (GroupKind::Sequence, vec!["one"], 0),
        (GroupKind::Sequence, vec!["one"], 65),
        (GroupKind::Parallel, vec!["one", "bad name"], 2),
        (GroupKind::Parallel, vec!["one", "one"], 2),
    ] {
        let members = names
            .iter()
            .map(|name| branch(&trace, name, &row))
            .collect::<Vec<_>>();
        trace.events.borrow_mut().clear();
        assert!(matches!(
            assemble(&trace, kind, &members, maximum),
            Err(NativeGroupExportError::Members(_))
        ));
        assert!(trace.events.borrow().iter().all(|phase| *phase == "name"));
    }
    let members = (0..64)
        .map(|index| branch(&trace, &format!("m_{index}"), &row))
        .collect::<Vec<_>>();
    assert_eq!(
        assemble(&trace, GroupKind::Sequence, &members, 64)
            .unwrap()
            .members
            .len(),
        64
    );
}

#[test]
fn opaque_name_and_operation_errors_keep_original_positions_without_private_formatting() {
    let trace = trace();
    let row = row();
    let members = [
        branch(&trace, "first", &row),
        branch(&trace, "second", &row),
    ];
    for name_failure in [true, false] {
        let names = Cell::new(0);
        let operations = Cell::new(0);
        let signatures = Cell::new(0);
        let dropped = trace.observations_dropped.get();
        let failure = observe_native_group_exports(
            GroupKind::Parallel,
            &members,
            2,
            |member| {
                names.set(names.get() + 1);
                if name_failure && names.get() == 2 {
                    Err(error(7))
                } else {
                    Ok(member.name.as_str())
                }
            },
            |member| {
                operations.set(operations.get() + 1);
                if operations.get() == 2 {
                    Err(error(8))
                } else {
                    Ok(observation(&trace, member))
                }
            },
            |_, _, _| {
                signatures.set(signatures.get() + 1);
                Ok(())
            },
        )
        .unwrap_err();
        assert!(!format!("{failure:?}: {failure}").contains("private"));
        assert!(std::error::Error::source(&failure).is_none());
        let native = match failure {
            NativeGroupExportError::Members(NativeGroupMemberError::Native {
                member_index: 1,
                phase: NativeGroupMemberPhase::Name,
                error,
            }) if name_failure => error,
            NativeGroupExportError::Native {
                phase: NativeGroupExportPhase::Operation { member_index: 1 },
                error,
            } if !name_failure => error,
            _ => panic!(),
        };
        assert_eq!(native.marker, if name_failure { 7 } else { 8 });
        assert_eq!(native.secret, "private GUI schema and input");
        assert_eq!(names.get(), 2);
        assert_eq!(operations.get(), if name_failure { 0 } else { 2 });
        assert_eq!(signatures.get(), 0);
        assert_eq!(
            trace.observations_dropped.get() - dropped,
            usize::from(!name_failure)
        );
        assert_eq!(trace.branches_dropped.get(), 0);
    }
}

#[test]
fn late_signature_rejection_drops_every_observation_without_partial_output_or_graph_consumption() {
    let trace = trace();
    let row = row();
    let members = [
        branch(&trace, "first", &row),
        branch(&trace, "second", &row),
    ];
    trace.granted.set(false);
    let failure = assemble(&trace, GroupKind::Parallel, &members, 2).unwrap_err();
    assert!(matches!(
        failure,
        NativeGroupExportError::Native {
            phase: NativeGroupExportPhase::Signature,
            error: PrivateError { marker: 9, .. }
        }
    ));
    assert_eq!(
        *trace.events.borrow(),
        ["name", "name", "operation", "operation", "signature"]
    );
    assert_eq!(trace.observations_dropped.get(), 2);
    assert_eq!(trace.branches_dropped.get(), 0);
    trace.granted.set(true);
    assert!(assemble(&trace, GroupKind::Parallel, &members, 2).is_ok());
}

#[test]
fn final_native_admission_corroborates_original_name_row_type_atomicity_and_current_policy() {
    let trace = trace();
    let row = row();
    let lookalike = Row {
        tag: row.tag,
        private: vec![7, 11],
    };
    for mismatch in 0..4 {
        let mut member = branch(&trace, "first", &row);
        if mismatch == 0 {
            member.declared = &lookalike;
        }
        if mismatch == 1 {
            member.nested = true;
        }
        let members = [member];
        let result = observe_native_group_exports(
            GroupKind::Sequence,
            &members,
            1,
            |member| {
                Ok(if mismatch == 2 {
                    "fake-name"
                } else {
                    member.name.as_str()
                })
            },
            |member| {
                if mismatch == 3 {
                    trace.granted.set(false);
                }
                Ok(observation(&trace, member))
            },
            |_, original, exports| {
                if trace.granted.get()
                    && !original[0].nested
                    && std::ptr::eq(original[0].row, original[0].declared)
                    && std::ptr::eq(original[0].name.as_str(), exports[0].name)
                {
                    Ok(())
                } else {
                    Err(error(4))
                }
            },
        );
        assert!(matches!(
            result,
            Err(NativeGroupExportError::Native {
                phase: NativeGroupExportPhase::Signature,
                ..
            })
        ));
        trace.granted.set(true);
    }
}

#[test]
fn callbacks_and_signature_unwind_stop_without_retry_and_release_owned_prefixes() {
    let trace = trace();
    let row = row();
    let members = [
        branch(&trace, "first", &row),
        branch(&trace, "second", &row),
    ];
    for phase in 0..3 {
        let names = Cell::new(0);
        let operations = Cell::new(0);
        let signatures = Cell::new(0);
        let dropped = trace.observations_dropped.get();
        let result = catch_unwind(AssertUnwindSafe(|| {
            observe_native_group_exports(
                GroupKind::Parallel,
                &members,
                2,
                |member| -> Result<_, PrivateError> {
                    names.set(names.get() + 1);
                    assert!(phase != 0 || names.get() != 2, "private name");
                    Ok(&member.name)
                },
                |member| {
                    operations.set(operations.get() + 1);
                    assert!(phase != 1 || operations.get() != 2, "private operation");
                    Ok(observation(&trace, member))
                },
                |_, _, _| {
                    signatures.set(signatures.get() + 1);
                    assert_ne!(phase, 2, "private signature");
                    Ok(())
                },
            )
        }));
        assert!(result.is_err());
        assert_eq!(names.get(), 2);
        assert_eq!(operations.get(), if phase == 0 { 0 } else { 2 });
        assert_eq!(signatures.get(), usize::from(phase == 2));
        assert_eq!(trace.observations_dropped.get() - dropped, phase);
        assert_eq!(trace.branches_dropped.get(), 0);
        assert!(assemble(&trace, GroupKind::Parallel, &members, 2).is_ok());
    }
}

#[test]
fn cleanup_unwind_does_not_retry_signature_or_consume_borrowed_native_members() {
    let trace = trace();
    let row = row();
    let members = [
        branch(&trace, "first", &row),
        branch(&trace, "second", &row),
    ];
    trace.granted.set(false);
    trace.panic_drop.set(true);
    let result = catch_unwind(AssertUnwindSafe(|| {
        assemble(&trace, GroupKind::Parallel, &members, 2)
    }));
    assert!(result.is_err());
    assert_eq!(trace.observations_dropped.get(), 2);
    assert_eq!(trace.branches_dropped.get(), 0);
    assert_eq!(
        trace
            .events
            .borrow()
            .iter()
            .filter(|phase| **phase == "signature")
            .count(),
        1
    );
}

#[test]
fn every_entry_rechecks_current_names_and_limits_without_accepting_a_cached_member_view() {
    let trace = trace();
    let row = row();
    let mut members = [branch(&trace, "first", &row)];
    drop(assemble(&trace, GroupKind::Sequence, &members, 1).unwrap());
    members[0].name = "changed name".into();
    trace.events.borrow_mut().clear();
    assert!(assemble(&trace, GroupKind::Sequence, &members, 1).is_err());
    assert_eq!(*trace.events.borrow(), ["name"]);
    members[0].name = "first".into();
    trace.events.borrow_mut().clear();
    assert!(assemble(&trace, GroupKind::Sequence, &members, 0).is_err());
    assert!(trace.events.borrow().is_empty());
}

#[test]
fn signature_admission_is_once_owned_and_success_is_mutable_metadata_not_authority() {
    struct Finish(Rc<Cell<usize>>);
    impl Drop for Finish {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }
    let dropped = Rc::new(Cell::new(0));
    let finish = Finish(dropped.clone());
    let branches = [()];
    let mut exports = observe_native_group_exports(
        GroupKind::Sequence,
        &branches,
        1,
        |_| -> Result<_, ()> { Ok("member") },
        |_| Ok(vec![0u8; 8192]),
        move |_, _, _| {
            drop(finish);
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(dropped.get(), 1);
    assert_eq!(exports.members[0].operation.len(), 8192);
    exports.kind = GroupKind::Parallel;
    exports.members[0].name = "unchecked later mutation";
    assert_eq!(exports.members.len(), 1);
}

#[test]
fn unrelated_unsized_gui_and_opaque_device_schemas_need_no_native_clone_equality_or_wire_traits() {
    trait GuiOperation {
        fn token(&self) -> u8;
    }
    struct Local(Rc<Cell<u8>>);
    impl GuiOperation for Local {
        fn token(&self) -> u8 {
            self.0.get()
        }
    }
    struct GuiMember<'a> {
        label: String,
        operation: &'a dyn GuiOperation,
    }
    let local = Local(Rc::new(Cell::new(7)));
    let gui = [GuiMember {
        label: "gui".into(),
        operation: &local,
    }];
    let exports = observe_native_group_exports(
        GroupKind::Sequence,
        &gui,
        1,
        |member| -> Result<_, PrivateError> { Ok(&member.label) },
        |member| Ok(member.operation),
        |_, original, exports| {
            if std::ptr::eq(original[0].operation, exports[0].operation)
                && exports[0].operation.token() == 7
            {
                Ok(())
            } else {
                Err(error(7))
            }
        },
    )
    .unwrap();
    assert!(std::ptr::eq(
        exports.members[0].operation,
        &local as &dyn GuiOperation
    ));
    assert_eq!(Rc::strong_count(&local.0), 1);
    struct Device {
        key: u128,
        private: Vec<u8>,
    }
    struct DeviceMember<'a> {
        label: String,
        device: &'a Device,
    }
    let device = Device {
        key: 997,
        private: vec![9],
    };
    let members = [DeviceMember {
        label: "device".into(),
        device: &device,
    }];
    let exports = observe_native_group_exports(
        GroupKind::Sequence,
        &members,
        1,
        |member| -> Result<_, PrivateError> { Ok(&member.label) },
        |member| Ok(member.device),
        |_, original, exports| {
            if std::ptr::eq(original[0].device, exports[0].operation)
                && exports[0].operation.key == 997
                && !exports[0].operation.private.is_empty()
            {
                Ok(())
            } else {
                Err(error(9))
            }
        },
    )
    .unwrap();
    assert!(std::ptr::eq(exports.members[0].operation, &device));
}

#[test]
fn explicit_native_noop_admission_is_not_an_automatic_row_or_name_authenticity_proof() {
    let original = [1u32];
    let foreign = [2u32];
    let exports = observe_native_group_exports(
        GroupKind::Sequence,
        &original,
        1,
        |_| -> Result<_, ()> { Ok("explicit-name") },
        |_| Ok(&foreign[0]),
        |_, _, _| Ok(()),
    )
    .unwrap();
    assert!(std::ptr::eq(exports.members[0].operation, &foreign[0]));
    assert!(!std::ptr::eq(exports.members[0].operation, &original[0]));
}
