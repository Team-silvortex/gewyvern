use std::cell::{Cell, RefCell};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;

use leselang_hir::group_exports::*;
use leselang_hir::ir::{Computation, ComputedArgument, ComputedBranch, GroupKind};
use leselang_runtime_core::*;

type Data = Computation<u8, u32, (), u8>;
const LIMITS: GroupExportLimits = GroupExportLimits {
    max_nodes: 128,
    max_depth: 16,
    max_groups: 16,
    max_members: 64,
};
fn integer(n: u64) -> Data {
    Data::Literal {
        value: ScalarValue::Integer(n),
    }
}
fn call(operation: u32) -> Data {
    Data::Call {
        operation,
        arguments: vec![],
    }
}
fn branch(name: &str, operation: u32) -> ComputedBranch<Data, u8> {
    ComputedBranch {
        name: name.into(),
        value: call(operation),
        result_type: 1,
    }
}
fn group(kind: GroupKind, members: &[(&str, u32)]) -> Data {
    Data::Group {
        group_kind: kind,
        branches: members.iter().map(|(name, op)| branch(name, *op)).collect(),
    }
}
fn one() -> Data {
    group(GroupKind::Sequence, &[("entry", 17)])
}
fn choose(then: Data, otherwise: Data) -> Data {
    Data::Choose {
        when: Box::new(Data::Literal {
            value: ScalarValue::Boolean(true),
        }),
        then: Box::new(then),
        otherwise: Box::new(otherwise),
    }
}
fn observe(expression: &Data) -> Result<GroupExports<'_, u32>, GroupExportError<()>> {
    observe_group_exports(
        expression,
        LIMITS,
        |_| Err(()),
        |member| match member.value {
            Data::Call { operation, .. } => Ok(operation),
            _ => Err(()),
        },
        |left, right| Ok(left == right),
    )
}
fn no_hooks(expression: &Data, limits: GroupExportLimits) -> GroupExportError<()> {
    observe_group_exports(
        expression,
        limits,
        |_| -> Result<GroupExports<'_, u32>, ()> { panic!("host hook must not run") },
        |_| -> Result<u32, ()> { panic!("branch hook must not run") },
        |_, _| -> Result<bool, ()> { panic!("comparison must not run") },
    )
    .unwrap_err()
}

#[test]
fn shared_group_shape_accepts_original_ir_labels_and_rechecks_current_export_ceiling() {
    let longest = "x".repeat(64);
    let input = group(
        GroupKind::Sequence,
        &[("1", 17), ("all-4", 18), ("while", 17), (&longest, 18)],
    );
    assert!(observe(&input).is_ok());
    let names = (0..64)
        .map(|index| format!("member_{index}"))
        .collect::<Vec<_>>();
    let members = names
        .iter()
        .map(|name| (name.as_str(), 17))
        .collect::<Vec<_>>();
    let input = group(GroupKind::Parallel, &members);
    let exports = observe(&input).unwrap();
    let Data::Group { branches, .. } = &input else {
        panic!()
    };
    assert_eq!(exports.members.len(), 64);
    for (branch, export) in branches.iter().zip(&exports.members) {
        assert!(std::ptr::eq(branch.name.as_str(), export.name));
    }
    for maximum in [0, 63] {
        assert!(matches!(
            no_hooks(
                &input,
                GroupExportLimits {
                    max_members: maximum,
                    ..LIMITS
                }
            ),
            GroupExportError::MemberShape
        ));
    }
    assert!(matches!(
        no_hooks(
            &input,
            GroupExportLimits {
                max_members: 65,
                ..LIMITS
            }
        ),
        GroupExportError::InvalidLimits
    ));
}

#[test]
fn shared_group_shape_rejects_complete_bad_names_before_tail_or_native_comparison() {
    for last in ["first", "bad tail"] {
        let input = Data::Group {
            group_kind: GroupKind::Sequence,
            branches: vec![
                ComputedBranch {
                    name: "first".into(),
                    value: integer(0),
                    result_type: 1,
                },
                branch(last, 17),
            ],
        };
        assert!(matches!(
            no_hooks(&input, LIMITS),
            GroupExportError::MemberShape
        ));
        let input = choose(
            one(),
            Data::Host {
                effect: Box::new(()),
            },
        );
        let visits = Cell::new(0);
        let failure = observe_group_exports(
            &input,
            LIMITS,
            |_| -> Result<GroupExports<'_, u32>, ()> {
                visits.set(visits.get() + 1);
                Ok(GroupExports {
                    kind: GroupKind::Sequence,
                    members: vec![
                        GroupExport {
                            name: "first",
                            operation: 17,
                        },
                        GroupExport {
                            name: last,
                            operation: 18,
                        },
                    ],
                })
            },
            |_| -> Result<u32, ()> { panic!("invalid returned shape must stop branch hooks") },
            |_, _| -> Result<bool, ()> { panic!("invalid returned shape must stop comparison") },
        )
        .unwrap_err();
        assert!(matches!(failure, GroupExportError::MemberShape));
        assert_eq!(visits.get(), 1);
    }
}

#[test]
fn direct_exports_borrow_original_names_and_keep_declaration_order() {
    let expression = group(GroupKind::Parallel, &[("second", 18), ("first", 17)]);
    let Data::Group { branches, .. } = &expression else {
        panic!()
    };
    let visited = RefCell::new(Vec::new());
    let exports = observe_group_exports(
        &expression,
        LIMITS,
        |_| Err(()),
        |member| {
            visited.borrow_mut().push(member.name.as_str());
            let Data::Call { operation, .. } = member.value else {
                panic!()
            };
            Ok(operation)
        },
        |_, _| -> Result<bool, ()> { panic!() },
    )
    .unwrap();
    assert_eq!(*visited.borrow(), ["second", "first"]);
    assert_eq!(exports.kind, GroupKind::Parallel);
    assert_eq!(exports.members[0].name.as_ptr(), branches[0].name.as_ptr());
    assert_eq!(exports.members[1].name.as_ptr(), branches[1].name.as_ptr());
    assert_eq!(exports.members[0].operation, 18);
}

#[test]
fn complete_cold_physical_name_and_literal_preflight_precedes_hooks() {
    let mut deep = integer(0);
    for _ in 0..17 {
        deep = Data::Unary {
            operator: UnaryOperator::ToString,
            value: Box::new(deep),
        };
    }
    for preparation in [
        deep,
        Data::Literal {
            value: ScalarValue::String("x".repeat(MAX_SCALAR_STRING_BYTES + 1)),
        },
        Data::Local {
            name: "bad name".into(),
        },
    ] {
        let expression = choose(
            one(),
            Data::Bind {
                name: "prep".into(),
                value: Box::new(preparation),
                body: Box::new(one()),
            },
        );
        assert!(matches!(
            no_hooks(&expression, LIMITS),
            GroupExportError::Structure(_) | GroupExportError::InvalidLanguageNode
        ));
    }
}

#[test]
fn cold_call_argument_names_and_count_are_bounded_before_native_operation_queries() {
    for arguments in [
        vec![ComputedArgument {
            name: "x".repeat(65),
            value: integer(0),
        }],
        vec![ComputedArgument {
            name: "bad-name".into(),
            value: integer(0),
        }],
        (0..65)
            .map(|index| ComputedArgument {
                name: format!("p{index}"),
                value: integer(0),
            })
            .collect(),
    ] {
        let expression = Data::Group {
            group_kind: GroupKind::Sequence,
            branches: vec![ComputedBranch {
                name: "entry".into(),
                value: Data::Call {
                    operation: 17,
                    arguments,
                },
                result_type: 1,
            }],
        };
        assert!(matches!(
            no_hooks(&expression, LIMITS),
            GroupExportError::InvalidLanguageNode
        ));
    }
    let expression = Data::Group {
        group_kind: GroupKind::Sequence,
        branches: vec![ComputedBranch {
            name: "entry".into(),
            value: Data::Call {
                operation: 17,
                arguments: vec![ComputedArgument {
                    name: "body".into(),
                    value: integer(0),
                }],
            },
            result_type: 1,
        }],
    };
    assert!(observe(&expression).is_ok());
}

#[test]
fn all_known_cold_group_kinds_names_and_order_match_before_native_queries() {
    for right in [
        group(GroupKind::Parallel, &[("a", 17), ("b", 18)]),
        group(GroupKind::Sequence, &[("b", 17), ("a", 18)]),
        group(GroupKind::Sequence, &[("a", 17), ("c", 18)]),
        group(GroupKind::Sequence, &[("a", 17)]),
    ] {
        let expression = choose(group(GroupKind::Sequence, &[("a", 17), ("b", 18)]), right);
        assert!(matches!(
            no_hooks(&expression, LIMITS),
            GroupExportError::SignatureMismatch
        ));
    }
}

#[test]
fn malformed_cold_group_labels_arity_and_duplicates_fail_before_callbacks() {
    for expression in [
        group(GroupKind::Sequence, &[]),
        group(GroupKind::Parallel, &[("a", 17)]),
        group(GroupKind::Sequence, &[("a", 17), ("a", 18)]),
        group(GroupKind::Sequence, &[("bad.name", 17)]),
        group(GroupKind::Sequence, &[("", 17)]),
        group(GroupKind::Sequence, &[("x".repeat(65).as_str(), 17)]),
    ] {
        assert!(matches!(
            no_hooks(&choose(one(), expression), LIMITS),
            GroupExportError::MemberShape
        ));
    }
}

#[test]
fn zero_group_member_and_root_policies_are_explicit() {
    for limits in [
        GroupExportLimits {
            max_nodes: 0,
            ..LIMITS
        },
        GroupExportLimits {
            max_groups: 0,
            ..LIMITS
        },
        GroupExportLimits {
            max_members: 0,
            ..LIMITS
        },
    ] {
        assert!(matches!(
            no_hooks(&one(), limits),
            GroupExportError::Structure(_)
                | GroupExportError::GroupLimit
                | GroupExportError::MemberShape
        ));
    }
    let host: Data = Data::Host {
        effect: Box::new(()),
    };
    assert!(matches!(
        no_hooks(
            &host,
            GroupExportLimits {
                max_members: 0,
                ..LIMITS
            }
        ),
        GroupExportError::MemberShape
    ));
    assert!(matches!(
        no_hooks(
            &one(),
            GroupExportLimits {
                max_depth: 0,
                ..LIMITS
            }
        ),
        GroupExportError::Structure(StructureError::DepthLimit)
    ));
    assert!(
        observe_group_exports(
            &host,
            GroupExportLimits {
                max_nodes: 1,
                max_depth: 0,
                ..LIMITS
            },
            |_| Ok::<_, ()>(GroupExports {
                kind: GroupKind::Sequence,
                members: vec![GroupExport {
                    name: "entry",
                    operation: 17
                }]
            }),
            |_| -> Result<u32, ()> { panic!() },
            |_, _| -> Result<bool, ()> { panic!() },
        )
        .is_ok()
    );
}

#[test]
fn every_limit_ceiling_and_whole_cold_group_count_precede_native_hooks() {
    for limits in [
        GroupExportLimits {
            max_nodes: 16_385,
            ..LIMITS
        },
        GroupExportLimits {
            max_depth: 65,
            ..LIMITS
        },
        GroupExportLimits {
            max_groups: 16_385,
            ..LIMITS
        },
        GroupExportLimits {
            max_members: 65,
            ..LIMITS
        },
    ] {
        assert!(matches!(
            no_hooks(&one(), limits),
            GroupExportError::InvalidLimits
        ));
    }
    assert!(matches!(
        no_hooks(
            &choose(one(), one()),
            GroupExportLimits {
                max_groups: 1,
                ..LIMITS
            }
        ),
        GroupExportError::GroupLimit
    ));
}

#[test]
fn unsupported_cold_tails_fail_before_other_valid_group_observations() {
    for tail in [
        integer(0),
        call(17),
        Data::Recover {
            value: Box::new(one()),
            fallback: Box::new(one()),
        },
    ] {
        assert!(matches!(
            no_hooks(&choose(tail, one()), LIMITS),
            GroupExportError::UnsupportedTail
        ));
    }
}

#[test]
fn zero_loop_and_fold_do_not_hide_effectful_preparation() {
    for value in [
        Data::Loop {
            name: "state".into(),
            initial: Box::new(integer(0)),
            condition: Box::new(Data::Literal {
                value: ScalarValue::Boolean(false),
            }),
            next: Box::new(call(17)),
            limit: 0,
        },
        Data::Fold {
            name: "state".into(),
            item: "item".into(),
            items: Box::new(Data::Strings { items: vec![] }),
            initial: Box::new(integer(0)),
            next: Box::new(call(17)),
            limit: 0,
        },
    ] {
        let expression = Data::Bind {
            name: "prep".into(),
            value: Box::new(value),
            body: Box::new(one()),
        };
        assert!(matches!(
            no_hooks(&expression, LIMITS),
            GroupExportError::ImpurePreparation
        ));
    }
}

#[test]
fn prepared_group_members_cannot_hide_nested_groups_or_effectful_call_operands() {
    for value in [
        one(),
        Data::Call {
            operation: 17,
            arguments: vec![ComputedArgument {
                name: "payload".into(),
                value: call(18),
            }],
        },
        Data::Bind {
            name: "r".into(),
            value: Box::new(call(17)),
            body: Box::new(call(18)),
        },
        Data::Choose {
            when: Box::new(call(17)),
            then: Box::new(call(18)),
            otherwise: Box::new(call(18)),
        },
        Data::Loop {
            name: "state".into(),
            initial: Box::new(integer(0)),
            condition: Box::new(Data::Literal {
                value: ScalarValue::Boolean(false),
            }),
            next: Box::new(integer(0)),
            limit: 0,
        },
    ] {
        let expression = Data::Group {
            group_kind: GroupKind::Sequence,
            branches: vec![ComputedBranch {
                name: "entry".into(),
                value,
                result_type: 1,
            }],
        };
        assert!(matches!(
            no_hooks(&expression, LIMITS),
            GroupExportError::UnsupportedTail
        ));
    }
}

#[test]
fn cold_operation_identity_mismatch_never_becomes_union_of_exports() {
    let expression = choose(one(), group(GroupKind::Sequence, &[("entry", 18)]));
    assert!(matches!(
        observe(&expression),
        Err(GroupExportError::SignatureMismatch)
    ));
}

struct Schema {
    key: u32,
    version: u32,
    result: u8,
    bytes: Vec<u8>,
}
struct Opaque {
    kind: GroupKind,
    names: Vec<String>,
    rows: Vec<Schema>,
    closed: bool,
}
type OpaqueNode = Computation<u8, u32, Opaque, u8>;
fn opaque(name: &str, key: u32) -> OpaqueNode {
    OpaqueNode::Host {
        effect: Box::new(Opaque {
            kind: GroupKind::Sequence,
            names: vec![name.into()],
            rows: vec![Schema {
                key,
                version: 3,
                result: 1,
                bytes: vec![7; 31],
            }],
            closed: true,
        }),
    }
}
fn opaque_choose(left: OpaqueNode, right: OpaqueNode) -> OpaqueNode {
    OpaqueNode::Choose {
        when: Box::new(OpaqueNode::Literal {
            value: ScalarValue::Boolean(true),
        }),
        then: Box::new(left),
        otherwise: Box::new(right),
    }
}

#[test]
fn opaque_observers_borrow_original_graphs_once_and_join_rightmost_first() {
    let expression = opaque_choose(opaque("entry", 17), opaque("entry", 17));
    let OpaqueNode::Choose {
        then, otherwise, ..
    } = &expression
    else {
        panic!()
    };
    let OpaqueNode::Host { effect: left } = then.as_ref() else {
        panic!()
    };
    let OpaqueNode::Host { effect: right } = otherwise.as_ref() else {
        panic!()
    };
    let events = RefCell::new(Vec::new());
    let exports = observe_group_exports(
        &expression,
        LIMITS,
        |graph| {
            assert!(graph.closed);
            events
                .borrow_mut()
                .push(if std::ptr::eq(graph, right.as_ref()) {
                    "right"
                } else {
                    "left"
                });
            Ok::<_, ()>(GroupExports {
                kind: graph.kind,
                members: graph
                    .names
                    .iter()
                    .zip(&graph.rows)
                    .map(|(name, row)| GroupExport {
                        name,
                        operation: row,
                    })
                    .collect(),
            })
        },
        |_| -> Result<&Schema, ()> { panic!() },
        |left, right| {
            events.borrow_mut().push("compare");
            Ok(left.key == right.key
                && left.version == right.version
                && left.result == right.result)
        },
    )
    .unwrap();
    assert_eq!(*events.borrow(), ["right", "left", "compare"]);
    assert_eq!(exports.members[0].name.as_ptr(), left.names[0].as_ptr());
    assert!(std::ptr::eq(exports.members[0].operation, &left.rows[0]));
    assert_eq!(
        exports.members[0].operation.bytes.as_ptr(),
        left.rows[0].bytes.as_ptr()
    );
}

#[test]
fn native_operation_schema_identity_is_explicit_not_same_spelling_or_result_tag() {
    let expression = opaque_choose(opaque("entry", 17), opaque("entry", 17));
    let outcome = observe_group_exports(
        &expression,
        LIMITS,
        |graph| {
            Ok::<_, ()>(GroupExports {
                kind: graph.kind,
                members: vec![GroupExport {
                    name: &graph.names[0],
                    operation: &graph.rows[0],
                }],
            })
        },
        |_| -> Result<&Schema, ()> { panic!() },
        |left, right| Ok(std::ptr::eq(*left, *right)),
    );
    assert!(matches!(outcome, Err(GroupExportError::SignatureMismatch)));
}

#[test]
fn opaque_shape_mismatch_precedes_operation_comparators() {
    let expression = opaque_choose(opaque("left_only", 17), opaque("right_only", 17));
    let outcome = observe_group_exports(
        &expression,
        LIMITS,
        |graph| {
            Ok::<_, ()>(GroupExports {
                kind: graph.kind,
                members: vec![GroupExport {
                    name: &graph.names[0],
                    operation: &graph.rows[0],
                }],
            })
        },
        |_| -> Result<&Schema, ()> { panic!() },
        |_, _| -> Result<bool, ()> { panic!("shape must match before comparing") },
    );
    assert!(matches!(outcome, Err(GroupExportError::SignatureMismatch)));
}

#[test]
fn opaque_invalid_member_observations_fail_before_comparison() {
    for (kind, names) in [
        (GroupKind::Sequence, vec![]),
        (GroupKind::Parallel, vec!["a"]),
        (GroupKind::Sequence, vec!["a", "a"]),
        (GroupKind::Sequence, vec!["bad.name"]),
    ] {
        let expression = opaque("entry", 17);
        let outcome = observe_group_exports(
            &expression,
            LIMITS,
            |_| {
                Ok::<_, ()>(GroupExports {
                    kind,
                    members: names
                        .iter()
                        .map(|name| GroupExport {
                            name,
                            operation: 17,
                        })
                        .collect(),
                })
            },
            |_| -> Result<u32, ()> { panic!() },
            |_, _| -> Result<bool, ()> { panic!() },
        );
        assert!(matches!(outcome, Err(GroupExportError::MemberShape)));
    }
}

#[test]
fn original_branch_return_declaration_is_available_for_native_corroboration() {
    let mut expression = one();
    let Data::Group { branches, .. } = &mut expression else {
        panic!()
    };
    branches[0].result_type = 9;
    let outcome = observe_group_exports(
        &expression,
        LIMITS,
        |_| Err(()),
        |member| {
            assert_eq!(member.result_type, 9);
            if member.result_type == 1 {
                Ok(17)
            } else {
                Err(())
            }
        },
        |_, _| Ok(true),
    );
    assert!(matches!(
        outcome,
        Err(GroupExportError::Native {
            group_index: 0,
            phase: GroupExportPhase::Branch { index: 0 },
            ..
        })
    ));
}

struct PrivateError(&'static str);
struct Token {
    bytes: Vec<u8>,
    drops: Rc<Cell<usize>>,
}
impl Drop for Token {
    fn drop(&mut self) {
        self.drops.set(self.drops.get() + 1);
    }
}

#[test]
fn move_only_observations_keep_buffers_without_clone_debug_serde_send_or_equality() {
    let expression = one();
    let drops = Rc::new(Cell::new(0));
    let token = Token {
        bytes: vec![8; 31],
        drops: drops.clone(),
    };
    let pointer = token.bytes.as_ptr();
    let mut token = Some(token);
    let exports = observe_group_exports(
        &expression,
        LIMITS,
        |_| -> Result<GroupExports<'_, Token>, PrivateError> { panic!() },
        |_| Ok(token.take().unwrap()),
        |_, _| -> Result<bool, PrivateError> { panic!() },
    )
    .unwrap();
    assert_eq!(exports.members[0].operation.bytes.as_ptr(), pointer);
    assert_eq!(drops.get(), 0);
    drop(exports);
    assert_eq!(drops.get(), 1);
}

#[test]
fn branch_error_releases_partial_owned_observations_without_retry_or_consuming_ir() {
    let expression = group(GroupKind::Parallel, &[("first", 17), ("second", 18)]);
    let drops = Rc::new(Cell::new(0));
    let visits = Cell::new(0);
    let error = observe_group_exports(
        &expression,
        LIMITS,
        |_| -> Result<GroupExports<'_, Token>, PrivateError> { panic!() },
        |_| {
            visits.set(visits.get() + 1);
            if visits.get() == 1 {
                Ok(Token {
                    bytes: vec![1],
                    drops: drops.clone(),
                })
            } else {
                Err(PrivateError("secret-schema"))
            }
        },
        |_, _| -> Result<bool, PrivateError> { panic!() },
    )
    .unwrap_err();
    assert!(matches!(
        &error,
        GroupExportError::Native {
            phase: GroupExportPhase::Branch { index: 1 },
            error: PrivateError("secret-schema"),
            ..
        }
    ));
    assert_eq!((visits.get(), drops.get()), (2, 1));
    assert!(!format!("{error:?} {error}").contains("secret-schema"));
    assert!(std::error::Error::source(&error).is_none());
    assert!(observe(&expression).is_ok());
}

#[test]
fn comparator_error_preserves_native_error_and_releases_both_observations() {
    let expression = choose(one(), one());
    let drops = Rc::new(Cell::new(0));
    let visits = Cell::new(0);
    let comparisons = Cell::new(0);
    let error = observe_group_exports(
        &expression,
        LIMITS,
        |_| -> Result<GroupExports<'_, Token>, PrivateError> { panic!() },
        |_| {
            visits.set(visits.get() + 1);
            Ok(Token {
                bytes: vec![1],
                drops: drops.clone(),
            })
        },
        |_, _| {
            comparisons.set(comparisons.get() + 1);
            Err(PrivateError("private-comparison"))
        },
    )
    .unwrap_err();
    assert!(matches!(
        &error,
        GroupExportError::Native {
            group_index: 1,
            phase: GroupExportPhase::Compare { index: 0 },
            error: PrivateError("private-comparison"),
            ..
        }
    ));
    assert_eq!((visits.get(), comparisons.get(), drops.get()), (2, 1, 2));
    assert!(!format!("{error:?} {error}").contains("private-comparison"));
}

#[test]
fn native_unwind_stops_later_hooks_and_drops_observations_without_partial_output() {
    for comparison_panics in [false, true] {
        let expression = choose(one(), one());
        let drops = Rc::new(Cell::new(0));
        let visits = Cell::new(0);
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            observe_group_exports(
                &expression,
                LIMITS,
                |_| -> Result<GroupExports<'_, Token>, ()> { panic!() },
                |_| {
                    visits.set(visits.get() + 1);
                    assert!(comparison_panics || visits.get() == 1, "branch unwind");
                    Ok(Token {
                        bytes: vec![1],
                        drops: drops.clone(),
                    })
                },
                |_, _| -> Result<bool, ()> { panic!("comparison unwind") },
            )
        }));
        assert!(outcome.is_err());
        assert_eq!(visits.get(), 2);
        assert_eq!(drops.get(), if comparison_panics { 2 } else { 1 });
        assert!(observe(&expression).is_ok());
    }
}

#[test]
fn debug_redacts_borrowed_names_and_native_observations() {
    let value = GroupExports {
        kind: GroupKind::Sequence,
        members: vec![GroupExport {
            name: "private-member",
            operation: PrivateError("private-operation"),
        }],
    };
    assert!(!format!("{value:?}").contains("private"));
    assert!(!format!("{:?}", value.members[0]).contains("private"));
}

#[test]
fn distinct_embedded_host_formats_can_preserve_original_native_schema_identity() {
    struct DeviceRow {
        command: &'static str,
        protocol: u16,
    }
    type Device = Computation<(), &'static str, (), u16>;
    let rows = [
        DeviceRow {
            command: "seek",
            protocol: 7,
        },
        DeviceRow {
            command: "inspect",
            protocol: 9,
        },
    ];
    let make = || Device::Group {
        group_kind: GroupKind::Parallel,
        branches: rows
            .iter()
            .map(|row| ComputedBranch {
                name: row.command.into(),
                value: Device::Call {
                    operation: row.command,
                    arguments: vec![],
                },
                result_type: row.protocol,
            })
            .collect(),
    };
    let expression = Device::Choose {
        when: Box::new(Device::Literal {
            value: ScalarValue::Boolean(false),
        }),
        then: Box::new(make()),
        otherwise: Box::new(make()),
    };
    let exports = observe_group_exports(
        &expression,
        LIMITS,
        |_| -> Result<GroupExports<'_, &DeviceRow>, ()> { Err(()) },
        |branch| {
            let Device::Call { operation, .. } = branch.value else {
                return Err(());
            };
            rows.iter()
                .find(|row| row.command == operation && row.protocol == branch.result_type)
                .ok_or(())
        },
        |left, right| Ok(std::ptr::eq(*left, *right)),
    )
    .unwrap();
    assert!(std::ptr::eq(exports.members[0].operation, &rows[0]));
    assert!(std::ptr::eq(exports.members[1].operation, &rows[1]));
}

#[test]
fn parsed_reference_helper_groups_preserve_canonical_wire_and_all_cold_authority() {
    for kind in ["seq", "all"] {
        let source = format!(
            r#"fn records(n: integer) = choose(when: eq(left: n, right: 0), then: {kind}(a: runtime.list(), b: runtime.list()), otherwise: {kind}(a: runtime.list(), b: runtime.list()))
fn main() = bind(g: records(n: 0), body: eq(left: field(value: member(value: g, name: "a"), name: "count"), right: 0))"#
        );
        let program = leselang_hir::lower(&leselang_syntax::parse(&source)).unwrap();
        let canonical = leselang_hir::canonical_source(&program.function.effect).unwrap();
        let again = leselang_hir::lower(&leselang_syntax::parse(&canonical)).unwrap();
        assert_eq!(
            serde_json::to_vec(&program).unwrap(),
            serde_json::to_vec(&again).unwrap()
        );
        assert!(
            leselang_hir::authorize(&program, &leselang_host_contract::CapabilitySet::default())
                .is_err()
        );
        assert!(
            leselang_hir::authorize(
                &program,
                &leselang_host_contract::CapabilitySet::new(["runtime.read"])
            )
            .is_ok()
        );
    }
}

#[test]
fn all_four_move_only_ir_slots_remain_original_borrowed_objects() {
    type Ir = Computation<Token, Token, Token, Token>;
    let drops = Rc::new(Cell::new(0));
    let token = || Token {
        bytes: vec![1; 31],
        drops: drops.clone(),
    };
    let expression = Ir::Group {
        group_kind: GroupKind::Sequence,
        branches: vec![
            ComputedBranch {
                name: "first".into(),
                value: Ir::Call {
                    operation: token(),
                    arguments: vec![ComputedArgument {
                        name: "payload".into(),
                        value: Ir::Field {
                            value: Box::new(Ir::Local {
                                name: "outer".into(),
                            }),
                            field: token(),
                        },
                    }],
                },
                result_type: token(),
            },
            ComputedBranch {
                name: "second".into(),
                value: Ir::Host {
                    effect: Box::new(token()),
                },
                result_type: token(),
            },
        ],
    };
    let Ir::Group { branches, .. } = &expression else {
        panic!()
    };
    let first = &branches[0] as *const ComputedBranch<Ir, Token>;
    let declared = branches[0].result_type.bytes.as_ptr();
    let Ir::Call {
        operation,
        arguments,
    } = &branches[0].value
    else {
        panic!()
    };
    let original_operation = operation as *const Token;
    let original_arguments = arguments.as_ptr();
    let Ir::Field { field, .. } = &arguments[0].value else {
        panic!()
    };
    let original_field = field as *const Token;
    let visits = Cell::new(0);
    let exports = observe_group_exports(
        &expression,
        LIMITS,
        |_| -> Result<GroupExports<'_, &Token>, PrivateError> { panic!() },
        |branch| {
            visits.set(visits.get() + 1);
            match &branch.value {
                Ir::Call {
                    operation,
                    arguments,
                } => {
                    assert!(std::ptr::eq(branch, first));
                    assert_eq!(branch.result_type.bytes.as_ptr(), declared);
                    assert_eq!(arguments.as_ptr(), original_arguments);
                    let Ir::Field { field, .. } = &arguments[0].value else {
                        panic!()
                    };
                    assert!(std::ptr::eq(field, original_field));
                    Ok(operation)
                }
                Ir::Host { effect } => Ok(effect.as_ref()),
                _ => panic!(),
            }
        },
        |_, _| -> Result<bool, PrivateError> { panic!() },
    )
    .unwrap();
    assert!(std::ptr::eq(
        exports.members[0].operation,
        original_operation
    ));
    assert_eq!(visits.get(), 2);
    drop(exports);
    assert_eq!(drops.get(), 0);
    drop(expression);
    assert_eq!(drops.get(), 5);
}

#[test]
fn native_catalog_version_and_grants_are_not_bypassed_by_matching_group_shape() {
    for (version, granted) in [(2, false), (2, true), (3, false), (3, true)] {
        let expression = choose(one(), one());
        let visits = Cell::new(0);
        let outcome = observe_group_exports(
            &expression,
            LIMITS,
            |_| Err(PrivateError("host")),
            |member| {
                visits.set(visits.get() + 1);
                if version != 3 || !granted {
                    return Err(PrivateError("native-policy"));
                }
                let Data::Call { operation, .. } = member.value else {
                    return Err(PrivateError("call"));
                };
                Ok(operation)
            },
            |left, right| Ok(left == right),
        );
        assert_eq!(outcome.is_ok(), version == 3 && granted);
        assert_eq!(visits.get(), if version == 3 && granted { 2 } else { 1 });
    }
}

#[test]
fn opaque_graph_rejection_retains_private_failure_and_stops_remaining_observers() {
    let expression = opaque_choose(opaque("entry", 17), opaque("entry", 17));
    let visits = Cell::new(0);
    let error = observe_group_exports(
        &expression,
        LIMITS,
        |_| {
            visits.set(visits.get() + 1);
            Err(PrivateError("private-graph"))
        },
        |_| -> Result<u32, PrivateError> { panic!() },
        |_, _| -> Result<bool, PrivateError> { panic!() },
    )
    .unwrap_err();
    assert!(matches!(
        &error,
        GroupExportError::Native {
            group_index: 0,
            phase: GroupExportPhase::Host,
            error: PrivateError("private-graph")
        }
    ));
    assert_eq!(visits.get(), 1);
    assert!(!format!("{error:?} {error}").contains("private-graph"));
    assert!(std::error::Error::source(&error).is_none());
}

#[test]
fn inclusive_sixty_four_member_limit_and_physical_budget_are_separate_from_dispatch() {
    for count in [64, 65] {
        let expression = Data::Group {
            group_kind: GroupKind::Sequence,
            branches: (0..count).map(|i| branch(&format!("m{i}"), 17)).collect(),
        };
        assert_eq!(observe(&expression).is_ok(), count == 64);
    }
}

#[test]
fn parsed_native_source_calls_feed_closed_exports_and_prepare_values_with_exact_schema_and_fuel() {
    use leselang_hir::call_evaluation::{
        CallEvaluationHost, CallEvaluationLimits, prepare_call_in_scope,
    };
    use leselang_hir::pure_evaluation::{
        PureEvaluationEnvironment, PureEvaluationLimits, PureValue, evaluate_pure_in_scope,
    };
    use leselang_hir::scalar_source::lower_scalar_source;
    use leselang_hir::source_call::{SourceCallHost, SourceCallLimits, lower_source_call};
    type Device = Computation<(), &'static str, (), u8>;
    struct Environment;
    impl PureEvaluationEnvironment<(), &'static str> for Environment {
        type Result = ();
        type Error = ();
        fn field(&self, _: &(), _: &()) -> Result<ScalarValue, ()> {
            Err(())
        }
        fn member(&self, _: &(), _: &str, _: &&'static str) -> Result<(), ()> {
            Err(())
        }
    }
    let parameters = [NamedParameter::required(
        "position",
        ScalarTypeSet::only(ScalarType::Integer),
    )];
    let schemas = [
        OperationSchema {
            key: "device.seek",
            parameters: &parameters,
            result: 1u8,
            required_capability: 31u8,
        },
        OperationSchema {
            key: "device.inspect",
            parameters: &parameters,
            result: 2u8,
            required_capability: 31u8,
        },
    ];
    let catalog = OperationCatalog::new(
        7,
        &schemas,
        OperationCatalogLimits {
            max_operations: 2,
            max_parameters_per_operation: 1,
        },
    )
    .unwrap();
    let source_host = SourceCallHost {
        catalog: &catalog,
        version: 7,
        granted: &[31],
    };
    let source_limits = SourceCallLimits {
        max_source_nodes: 128,
        max_source_depth: 16,
        max_lowered_nodes: 128,
        max_lowered_depth: 16,
        max_arguments: 64,
    };
    let tree = leselang_syntax::parse(
        "fn main() = seq(a: device.seek(position: add(left: 40, right: 1)), b: device.inspect(position: 42))",
    );
    assert!(tree.diagnostics.is_empty());
    let leselang_syntax::Expression::Call { arguments, .. } = &tree.function.as_ref().unwrap().body
    else {
        panic!()
    };
    let branches = arguments
        .iter()
        .map(|source| {
            let lowered =
                lower_source_call(&source.value, &source_host, source_limits, |argument| {
                    lower_scalar_source(
                        &argument.value,
                        source_limits,
                        |_| -> Result<(Device, Option<ScalarType>), ()> { Err(()) },
                    )
                    .map(|(value, ty)| (value, Some(ty)))
                })
                .unwrap();
            let operation = lowered.schema().key;
            let result_type = lowered.schema().result;
            ComputedBranch {
                name: source.name.clone(),
                value: Device::Call {
                    operation,
                    arguments: lowered.into_arguments(),
                },
                result_type,
            }
        })
        .collect();
    let expression = Device::Group {
        group_kind: GroupKind::Sequence,
        branches,
    };
    let exports = observe_group_exports(
        &expression,
        LIMITS,
        |_| Err(OperationCatalogError::UnknownOperation),
        |branch| {
            let Device::Call { operation, .. } = &branch.value else {
                return Err(OperationCatalogError::UnknownOperation);
            };
            let schema = catalog.authorize(operation, 7, &[31])?;
            assert_eq!(schema.result, branch.result_type);
            Ok(schema)
        },
        |left, right| Ok(std::ptr::eq(*left, *right)),
    )
    .unwrap();
    let Device::Group { branches, .. } = &expression else {
        panic!()
    };
    for (index, branch) in branches.iter().enumerate() {
        assert!(std::ptr::eq(
            exports.members[index].operation,
            &schemas[index]
        ));
        let Device::Call { arguments, .. } = &branch.value else {
            panic!()
        };
        let mut values = Vec::new();
        let mut scope = ScopeFrame::new(&mut values);
        let pure_limits = PureEvaluationLimits {
            max_nodes: 128,
            max_depth: 16,
            max_bindings: 16,
        };
        let mut before = Fuel::new(100);
        let direct = evaluate_pure_in_scope(
            &arguments[0].value,
            &mut scope,
            &Environment,
            &mut before,
            pure_limits,
        )
        .unwrap();
        let mut after = Fuel::new(100);
        let prepared = prepare_call_in_scope(
            &branch.value,
            &mut scope,
            &CallEvaluationHost {
                catalog: &catalog,
                version: 7,
                granted: &[31],
                environment: &Environment,
            },
            &mut after,
            CallEvaluationLimits {
                pure: pure_limits,
                max_arguments: 1,
            },
        )
        .unwrap();
        let PureValue::Scalar(direct) = direct else {
            panic!()
        };
        assert_eq!(direct, ScalarValue::Integer(41 + index as u64));
        assert_eq!(prepared.arguments()[0].value, direct);
        assert!(std::ptr::eq(
            prepared.schema(),
            exports.members[index].operation
        ));
        assert_eq!(before.remaining() - after.remaining(), 1);
        assert!(scope.is_empty());
    }
}

#[test]
fn reference_cold_group_reordering_result_forgery_and_nonflat_helpers_are_rejected() {
    for cold in [
        "seq(b: runtime.list(), a: runtime.list())",
        "all(a: runtime.list(), b: runtime.list())",
        "seq(a: ui.focus(node_id: \"a\"), b: runtime.list())",
        "seq(a: seq(inner: runtime.list()), b: runtime.list())",
    ] {
        let source = format!(
            r#"fn main() = bind(g: choose(when: true, then: seq(a: runtime.list(), b: runtime.list()), otherwise: {cold}), body: field(value: member(value: g, name: "a"), name: "count"))"#
        );
        assert!(leselang_hir::lower(&leselang_syntax::parse(&source)).is_err());
    }
    let program = leselang_hir::lower(&leselang_syntax::parse(r#"fn main() = bind(g: seq(a: ui.focus(node_id: to_string(value: 1))), body: field(value: member(value: g, name: "a"), name: "node_id"))"#)).unwrap();
    let mut effect = program.function.effect;
    let leselang_hir::Effect::Compute { expression } = &mut effect else {
        panic!()
    };
    let leselang_hir::computation::Computation::Bind { value, .. } = expression.as_mut() else {
        panic!()
    };
    let leselang_hir::computation::Computation::Group { branches, .. } = value.as_mut() else {
        panic!("requires an explicit computed group")
    };
    branches[0].result_type = leselang_hir::Type::Scalar(ScalarType::Boolean);
    assert!(leselang_hir::canonical_source(&effect).is_err());
}
