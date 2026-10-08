use std::cell::{Cell, RefCell};
use std::marker::PhantomData;
use std::rc::Rc;

use leselang_hir::call_typing::{CallTypeError, CallTypeHost, CallTypeLimits};
use leselang_hir::flow_typing::{
    CallFlowEnvironment, CallFlowTypeError as Error, CallFlowTypeLimits, GroupFlowEnvironment,
    GroupFlowTypeLimits, GroupMemberType, GroupTypeError, infer_call_flow_type,
    infer_group_flow_type,
};
use leselang_hir::ir::{Computation, ComputedArgument, ComputedBranch, GroupKind};
use leselang_hir::prepared_typing::{
    PreparedCallSchemas, PreparedCallTypeError, SelectedPreparedCall,
};
use leselang_hir::pure_typing::{
    PureType, PureTypeEnvironment, PureTypeError, TypeInferenceLimits,
};
use leselang_runtime_core::{
    NamedArgumentError, NamedParameter, OperationCatalog, OperationCatalogError,
    OperationCatalogLimits, OperationSchema, ScalarType, ScalarTypeSet, ScalarValue,
};

type Ir = Computation<u8, u32, Rc<()>, u8>;
const LIMITS: GroupFlowTypeLimits = GroupFlowTypeLimits {
    flow: CallFlowTypeLimits {
        call: CallTypeLimits {
            pure: TypeInferenceLimits {
                max_nodes: 256,
                max_depth: 16,
                max_bindings: 16,
            },
            max_arguments: 2,
        },
        max_calls: 32,
    },
    max_groups: 8,
    max_branches: 64,
};
#[derive(Clone, PartialEq)]
enum Metadata<'e> {
    Receipt(u8),
    Group {
        kind: GroupKind,
        members: Rc<[(&'e str, &'e u32, u8)]>,
    },
}
#[derive(Default)]
struct Environment<'e> {
    events: RefCell<Vec<&'static str>>,
    missing: bool,
    panic_on: Cell<Option<&'static str>>,
    life: PhantomData<&'e ()>,
}
impl Environment<'_> {
    fn query(&self, name: &'static str) {
        self.events.borrow_mut().push(name);
        assert_ne!(self.panic_on.get(), Some(name), "private native callback");
    }
}
impl<'e> PureTypeEnvironment<u8, u32> for Environment<'e> {
    type Result = Metadata<'e>;
    fn field_type(&self, result: &Self::Result, field: &u8) -> Option<ScalarType> {
        self.query("field");
        (matches!(result, Metadata::Receipt(1 | 2)) && *field == 7).then_some(ScalarType::Integer)
    }
    fn member_result(
        &self,
        group: &Self::Result,
        name: &str,
        operation: &u32,
    ) -> Option<Self::Result> {
        self.query("member");
        let Metadata::Group { members, .. } = group else {
            return None;
        };
        let (_, _, result) = members
            .iter()
            .find(|(declared, tag, _)| *declared == name && *tag == operation)?;
        Some(Metadata::Receipt(*result))
    }
}
impl<'e, 's> CallFlowEnvironment<'s, u8, u32, u8> for Environment<'e> {
    fn call_result_type(&self, _: &u32, declaration: &'s u8) -> Option<PureType<Self::Result>> {
        self.query("result");
        Some(PureType::Result(Metadata::Receipt(*declaration)))
    }
}
impl<'e, 's> GroupFlowEnvironment<'e, 's, u8, u32, u8, u8> for Environment<'e> {
    fn group_branch_type_matches(
        &self,
        declaration: &u8,
        inferred: &PureType<Self::Result>,
    ) -> bool {
        self.query("declaration");
        matches!(inferred, PureType::Result(Metadata::Receipt(id)) if id == declaration)
    }
    fn group_result_type(
        &self,
        kind: GroupKind,
        members: &[GroupMemberType<'e, u32, Self::Result>],
    ) -> Option<Self::Result> {
        self.query("group");
        if self.missing {
            return None;
        }
        let rows = members
            .iter()
            .map(|member| {
                let PureType::Result(Metadata::Receipt(id)) = &member.inferred else {
                    return None;
                };
                Some((member.name, member.operation, *id))
            })
            .collect::<Option<Rc<[_]>>>()?;
        Some(Metadata::Group {
            kind,
            members: rows,
        })
    }
}
fn number(n: u64) -> Ir {
    Ir::Literal {
        value: ScalarValue::Integer(n),
    }
}
fn boolean(b: bool) -> Ir {
    Ir::Literal {
        value: ScalarValue::Boolean(b),
    }
}
fn local(name: &str) -> Ir {
    Ir::Local { name: name.into() }
}
fn field(value: Ir) -> Ir {
    Ir::Field {
        value: Box::new(value),
        field: 7,
    }
}
fn member(group: &str, name: &str, operation: u32) -> Ir {
    Ir::Member {
        group: group.into(),
        name: name.into(),
        operation,
    }
}
fn call(operation: u32, value: Ir) -> Ir {
    Ir::Call {
        operation,
        arguments: vec![ComputedArgument {
            name: "position".into(),
            value,
        }],
    }
}
fn bind(name: &str, value: Ir, body: Ir) -> Ir {
    Ir::Bind {
        name: name.into(),
        value: Box::new(value),
        body: Box::new(body),
    }
}
fn choose(then: Ir, otherwise: Ir) -> Ir {
    Ir::Choose {
        when: Box::new(boolean(false)),
        then: Box::new(then),
        otherwise: Box::new(otherwise),
    }
}
fn branch(name: &str, value: Ir, result_type: u8) -> ComputedBranch<Ir, u8> {
    ComputedBranch {
        name: name.into(),
        value,
        result_type,
    }
}
fn group(kind: GroupKind, branches: Vec<ComputedBranch<Ir, u8>>) -> Ir {
    Ir::Group {
        group_kind: kind,
        branches,
    }
}
fn one(name: &str) -> Ir {
    group(
        GroupKind::Sequence,
        vec![branch(name, call(17, number(0)), 1)],
    )
}
fn check<'e>(
    expression: &'e Ir,
    prefix: &[(&'e str, PureType<Metadata<'e>>)],
    environment: &Environment<'e>,
    limits: GroupFlowTypeLimits,
) -> Result<PureType<Metadata<'e>>, Error> {
    check_policy(expression, prefix, environment, limits, 9, &[31, 32])
}
fn check_policy<'e>(
    expression: &'e Ir,
    prefix: &[(&'e str, PureType<Metadata<'e>>)],
    environment: &Environment<'e>,
    limits: GroupFlowTypeLimits,
    version: u32,
    granted: &[u8],
) -> Result<PureType<Metadata<'e>>, Error> {
    let parameters = [NamedParameter::required(
        "position",
        ScalarTypeSet::only(ScalarType::Integer),
    )];
    let rows = [
        OperationSchema {
            key: 17,
            parameters: &parameters,
            result: 1u8,
            required_capability: 31,
        },
        OperationSchema {
            key: 18,
            parameters: &parameters,
            result: 2u8,
            required_capability: 32,
        },
    ];
    let catalog = OperationCatalog::new(
        9,
        &rows,
        OperationCatalogLimits {
            max_operations: 2,
            max_parameters_per_operation: 1,
        },
    )
    .unwrap();
    let host = CallTypeHost {
        catalog: &catalog,
        version,
        granted,
        environment,
    };
    infer_group_flow_type(expression, prefix, environment, &host, limits)
}

#[test]
fn shared_group_shape_preserves_exact_indices_and_precedes_atomic_child_preparation() {
    let cases = [
        (
            vec!["bad.name", "second"],
            GroupTypeError::InvalidName { branch_index: 0 },
        ),
        (
            vec!["first", "bad tail"],
            GroupTypeError::InvalidName { branch_index: 1 },
        ),
        (
            vec!["first", "second", "first"],
            GroupTypeError::DuplicateName { branch_index: 2 },
        ),
        (
            vec!["first", "first", "bad tail"],
            GroupTypeError::DuplicateName { branch_index: 1 },
        ),
    ];
    for (names, expected) in cases {
        let expression = group(
            GroupKind::Sequence,
            names
                .into_iter()
                .map(|name| branch(name, number(0), 1))
                .collect(),
        );
        let environment = Environment::default();
        assert_eq!(
            check(&expression, &[], &environment, LIMITS).err(),
            Some(Error::Group {
                group_index: 0,
                error: expected
            })
        );
        assert!(environment.events.borrow().is_empty());
    }
}

#[test]
fn shared_group_shape_keeps_ir_label_grammar_and_current_inclusive_branch_limits() {
    let longest = "x".repeat(64);
    let expression = group(
        GroupKind::Sequence,
        ["1", "all-4", "while", &longest]
            .into_iter()
            .map(|name| branch(name, call(17, number(0)), 1))
            .collect(),
    );
    assert!(check(&expression, &[], &Environment::default(), LIMITS).is_ok());
    let expression = group(
        GroupKind::Parallel,
        (0..64)
            .map(|index| branch(&format!("member_{index}"), call(17, number(0)), 1))
            .collect(),
    );
    let limits = GroupFlowTypeLimits {
        flow: CallFlowTypeLimits {
            max_calls: 64,
            ..LIMITS.flow
        },
        ..LIMITS
    };
    let environment = Environment::default();
    assert!(check(&expression, &[], &environment, limits).is_ok());
    environment.events.borrow_mut().clear();
    for maximum in [0, 63] {
        assert_eq!(
            check(
                &expression,
                &[],
                &environment,
                GroupFlowTypeLimits {
                    max_branches: maximum,
                    ..limits
                }
            )
            .err(),
            Some(Error::Group {
                group_index: 0,
                error: GroupTypeError::Arity
            })
        );
        assert!(environment.events.borrow().is_empty());
    }
    assert!(
        check(
            &expression,
            &[],
            &environment,
            GroupFlowTypeLimits {
                max_branches: 65,
                ..limits
            }
        )
        .is_err()
    );
    assert!(environment.events.borrow().is_empty());
}

#[test]
fn sequence_and_parallel_capture_aliases_export_exact_members_to_successors() {
    for kind in [GroupKind::Sequence, GroupKind::Parallel] {
        let environment = Environment::default();
        let value = group(
            kind,
            vec![
                branch("first", call(17, number(0)), 1),
                branch(
                    "second",
                    bind(
                        "n",
                        number(1),
                        choose(call(18, local("n")), call(18, number(2))),
                    ),
                    2,
                ),
            ],
        );
        let expression = bind(
            "g",
            value,
            bind(
                "alias",
                local("g"),
                bind(
                    "r",
                    call(17, field(member("alias", "second", 18))),
                    field(local("r")),
                ),
            ),
        );
        assert!(matches!(
            check(&expression, &[], &environment, LIMITS),
            Ok(PureType::Scalar(ScalarType::Integer))
        ));
        let events = environment.events.borrow();
        assert_eq!(events.iter().filter(|name| **name == "group").count(), 1);
        assert!(
            events.iter().position(|name| *name == "group").unwrap()
                < events.iter().position(|name| *name == "member").unwrap()
        );
    }
}

#[test]
fn native_group_metadata_borrows_original_names_operations_and_declaration_order() {
    let expression = group(
        GroupKind::Parallel,
        vec![
            branch("last", call(18, number(0)), 2),
            branch("first", call(17, number(0)), 1),
        ],
    );
    let environment = Environment::default();
    let inferred = check(&expression, &[], &environment, LIMITS).ok().unwrap();
    let PureType::Result(Metadata::Group { kind, members }) = inferred else {
        panic!()
    };
    assert_eq!(kind, GroupKind::Parallel);
    let Ir::Group { branches, .. } = &expression else {
        panic!()
    };
    for (index, (name, operation, _)) in members.iter().enumerate() {
        assert!(std::ptr::eq(*name, branches[index].name.as_str()));
        let Ir::Call {
            operation: original,
            ..
        } = &branches[index].value
        else {
            panic!()
        };
        assert!(std::ptr::eq(*operation, original));
    }
    assert_eq!(
        members.iter().map(|row| row.0).collect::<Vec<_>>(),
        ["last", "first"]
    );
}

#[test]
fn conditional_group_exports_cannot_union_reorder_change_operations_or_modes() {
    let environment = Environment::default();
    let expression = bind("g", choose(one("a"), one("a")), field(member("g", "a", 17)));
    assert!(check(&expression, &[], &environment, LIMITS).is_ok());
    for otherwise in [
        one("b"),
        group(
            GroupKind::Sequence,
            vec![branch("a", call(18, number(0)), 2)],
        ),
        group(
            GroupKind::Parallel,
            vec![
                branch("a", call(17, number(0)), 1),
                branch("b", call(17, number(0)), 1),
            ],
        ),
    ] {
        assert_eq!(
            check(&choose(one("a"), otherwise), &[], &environment, LIMITS).err(),
            Some(Error::BranchTypes)
        );
    }
    let left = group(
        GroupKind::Sequence,
        vec![
            branch("a", call(17, number(0)), 1),
            branch("b", call(17, number(0)), 1),
        ],
    );
    let right = group(
        GroupKind::Sequence,
        vec![
            branch("b", call(17, number(0)), 1),
            branch("a", call(17, number(0)), 1),
        ],
    );
    assert_eq!(
        check(&choose(left, right), &[], &environment, LIMITS).err(),
        Some(Error::BranchTypes)
    );
}

#[test]
fn malformed_group_names_arity_and_member_shapes_precede_any_native_callback() {
    struct Poison;
    impl PreparedCallSchemas<'static, u32> for Poison {
        type Key = u32;
        type Domain = ScalarTypeSet;
        type Result = u8;
        type Capability = u8;
        fn select(
            &self,
            _: &u32,
        ) -> Result<SelectedPreparedCall<'static, u32, Self>, OperationCatalogError> {
            panic!("native selector must remain cold")
        }
    }
    let environment = Environment::default();
    let malformed = vec![
        group(GroupKind::Sequence, vec![]),
        group(
            GroupKind::Parallel,
            vec![branch("a", call(17, number(0)), 1)],
        ),
        group(
            GroupKind::Sequence,
            vec![branch("", call(17, number(0)), 1)],
        ),
        group(
            GroupKind::Sequence,
            vec![branch("bad name", call(17, number(0)), 1)],
        ),
        group(
            GroupKind::Sequence,
            vec![branch(&"x".repeat(65), call(17, number(0)), 1)],
        ),
        group(
            GroupKind::Sequence,
            vec![
                branch("a", call(17, number(0)), 1),
                branch("a", call(17, number(0)), 1),
            ],
        ),
        group(GroupKind::Sequence, vec![branch("a", number(1), 1)]),
        group(GroupKind::Sequence, vec![branch("a", one("nested"), 1)]),
        group(
            GroupKind::Sequence,
            vec![branch(
                "a",
                bind("r", call(17, number(0)), call(17, number(0))),
                1,
            )],
        ),
        choose(
            one("valid"),
            Ir::Host {
                effect: Box::new(Rc::new(())),
            },
        ),
        group(
            GroupKind::Sequence,
            vec![branch(
                "a",
                call(17, bind("r", call(17, number(0)), number(1))),
                1,
            )],
        ),
    ];
    for expression in malformed {
        assert!(infer_group_flow_type(&expression, &[], &environment, &Poison, LIMITS).is_err());
    }
    assert!(environment.events.borrow().is_empty());
}

#[test]
fn all_cold_catalog_and_named_shapes_precede_declaration_and_group_type_queries() {
    let environment = Environment::default();
    environment.panic_on.set(Some("result"));
    let expression = group(
        GroupKind::Parallel,
        vec![
            branch("a", call(17, number(0)), 1),
            branch("b", call(18, number(0)), 2),
        ],
    );
    assert_eq!(
        check_policy(&expression, &[], &environment, LIMITS, 9, &[31]).err(),
        Some(Error::Call {
            call_index: 1,
            error: CallTypeError::Catalog(OperationCatalogError::CapabilityDenied)
        })
    );
    assert_eq!(
        check_policy(&expression, &[], &environment, LIMITS, 8, &[31, 32]).err(),
        Some(Error::Call {
            call_index: 0,
            error: CallTypeError::Catalog(OperationCatalogError::UnsupportedVersion)
        })
    );
    let missing = group(
        GroupKind::Parallel,
        vec![
            branch("a", call(17, number(0)), 1),
            branch(
                "b",
                Ir::Call {
                    operation: 18,
                    arguments: vec![],
                },
                2,
            ),
        ],
    );
    assert_eq!(
        check(&missing, &[], &environment, LIMITS).err(),
        Some(Error::Call {
            call_index: 1,
            error: CallTypeError::Names(NamedArgumentError::MissingArgument { parameter_index: 0 })
        })
    );
    let unknown = choose(
        one("a"),
        group(
            GroupKind::Sequence,
            vec![branch("b", call(99, number(0)), 1)],
        ),
    );
    assert_eq!(
        check(&unknown, &[], &environment, LIMITS).err(),
        Some(Error::Call {
            call_index: 1,
            error: CallTypeError::Catalog(OperationCatalogError::UnknownOperation)
        })
    );
    assert!(environment.events.borrow().is_empty());
}

#[test]
fn each_member_requires_one_original_operation_row_even_with_matching_result_tags() {
    let parameters = [NamedParameter::required(
        "position",
        ScalarTypeSet::only(ScalarType::Integer),
    )];
    let rows = [
        OperationSchema {
            key: 17,
            parameters: &parameters,
            result: 1u8,
            required_capability: (),
        },
        OperationSchema {
            key: 18,
            parameters: &parameters,
            result: 1u8,
            required_capability: (),
        },
    ];
    let catalog = OperationCatalog::new(
        9,
        &rows,
        OperationCatalogLimits {
            max_operations: 2,
            max_parameters_per_operation: 1,
        },
    )
    .unwrap();
    let environment = Environment::default();
    environment.panic_on.set(Some("result"));
    let host = CallTypeHost {
        catalog: &catalog,
        version: 9,
        granted: &[()],
        environment: &environment,
    };
    let expression = group(
        GroupKind::Sequence,
        vec![branch(
            "a",
            choose(call(17, number(0)), call(18, number(0))),
            1,
        )],
    );
    assert_eq!(
        infer_group_flow_type(&expression, &[], &environment, &host, LIMITS).err(),
        Some(Error::Group {
            group_index: 0,
            error: GroupTypeError::InconsistentOperation { branch_index: 0 }
        })
    );
    assert!(environment.events.borrow().is_empty());
}

#[test]
fn original_branch_result_declarations_are_checked_before_group_exports_exist() {
    let environment = Environment::default();
    let expression = group(
        GroupKind::Parallel,
        vec![
            branch("a", call(17, number(0)), 1),
            branch("b", call(18, number(0)), 1),
        ],
    );
    assert_eq!(
        check(&expression, &[], &environment, LIMITS).err(),
        Some(Error::Group {
            group_index: 0,
            error: GroupTypeError::BranchType { branch_index: 1 }
        })
    );
    assert!(!environment.events.borrow().contains(&"group"));
    let environment = Environment {
        missing: true,
        ..Default::default()
    };
    assert_eq!(
        check(&one("a"), &[], &environment, LIMITS).err(),
        Some(Error::Group {
            group_index: 0,
            error: GroupTypeError::ResultType
        })
    );
}

#[test]
fn sibling_preparations_and_prior_sequence_receipts_are_not_bound_prefixes() {
    let environment = Environment::default();
    let expression = group(
        GroupKind::Sequence,
        vec![
            branch("a", bind("n", number(0), call(17, local("n"))), 1),
            branch("b", call(17, local("n")), 1),
        ],
    );
    assert!(matches!(
        check(&expression, &[], &environment, LIMITS),
        Err(Error::Call {
            call_index: 1,
            error: CallTypeError::Inference {
                error: PureTypeError::UnknownLocal,
                ..
            }
        })
    ));
    let expression = bind(
        "g",
        group(
            GroupKind::Sequence,
            vec![
                branch("a", call(17, number(0)), 1),
                branch("b", call(17, field(member("g", "a", 17))), 1),
            ],
        ),
        boolean(true),
    );
    assert!(matches!(
        check(&expression, &[], &environment, LIMITS),
        Err(Error::Call {
            error: CallTypeError::Inference {
                error: PureTypeError::MemberNotExported,
                ..
            },
            ..
        })
    ));
    let expression = group(
        GroupKind::Parallel,
        vec![
            branch("a", bind("n", number(0), call(17, local("n"))), 1),
            branch("b", bind("n", number(0), call(17, local("n"))), 1),
        ],
    );
    assert!(check(&expression, &[], &environment, LIMITS).is_ok());
}

#[test]
fn members_require_the_exact_exported_name_and_operation_tag() {
    let environment = Environment::default();
    for (name, operation) in [("missing", 17), ("a", 18)] {
        let expression = bind("g", one("a"), field(member("g", name, operation)));
        assert_eq!(
            check(&expression, &[], &environment, LIMITS).err(),
            Some(Error::Pure(PureTypeError::MemberNotExported))
        );
    }
}

#[test]
fn aggregate_node_depth_cold_group_and_branch_limits_are_inclusive_and_explicit() {
    let environment = Environment::default();
    let expression = bind("g", one("a"), field(member("g", "a", 17)));
    let exact = GroupFlowTypeLimits {
        flow: CallFlowTypeLimits {
            call: CallTypeLimits {
                pure: TypeInferenceLimits {
                    max_nodes: 6,
                    max_depth: 3,
                    max_bindings: 1,
                },
                max_arguments: 1,
            },
            max_calls: 1,
        },
        max_groups: 1,
        max_branches: 1,
    };
    assert!(check(&expression, &[], &environment, exact).is_ok());
    let mut small = exact;
    small.flow.call.pure.max_nodes = 5;
    assert!(check(&expression, &[], &environment, small).is_err());
    small = exact;
    small.flow.call.pure.max_depth = 2;
    assert!(check(&expression, &[], &environment, small).is_err());
    small = exact;
    small.flow.call.pure.max_bindings = 0;
    assert_eq!(
        check(&expression, &[], &environment, small).err(),
        Some(Error::Pure(PureTypeError::BindingLimit))
    );
    small = LIMITS;
    small.max_groups = 1;
    assert_eq!(
        check(&choose(one("a"), one("a")), &[], &environment, small).err(),
        Some(Error::Group {
            group_index: 1,
            error: GroupTypeError::GroupLimit
        })
    );
    small.max_groups = 0;
    assert!(check(&boolean(true), &[], &environment, small).is_ok());
    assert_eq!(
        check(&one("a"), &[], &environment, small).err(),
        Some(Error::Group {
            group_index: 0,
            error: GroupTypeError::GroupLimit
        })
    );
    small = LIMITS;
    small.max_branches = 65;
    assert_eq!(
        check(&boolean(true), &[], &environment, small).err(),
        Some(Error::InvalidLimits)
    );
    small = LIMITS;
    small.max_groups = 16_385;
    assert_eq!(
        check(&boolean(true), &[], &environment, small).err(),
        Some(Error::InvalidLimits)
    );
    for count in [64, 65] {
        let expression = group(
            GroupKind::Sequence,
            (0..count)
                .map(|n| branch(&format!("m{n}"), call(17, number(0)), 1))
                .collect(),
        );
        let mut limits = LIMITS;
        limits.flow.max_calls = 64;
        assert_eq!(
            check(&expression, &[], &environment, limits).is_ok(),
            count == 64
        );
    }
}

#[test]
fn existing_call_only_entry_still_rejects_new_groups() {
    let environment = Environment::default();
    let rows = [OperationSchema {
        key: 17u32,
        parameters: &[] as &[NamedParameter<&str, ScalarTypeSet>],
        result: 1u8,
        required_capability: (),
    }];
    let catalog = OperationCatalog::new(
        9,
        &rows,
        OperationCatalogLimits {
            max_operations: 1,
            max_parameters_per_operation: 0,
        },
    )
    .unwrap();
    let host = CallTypeHost {
        catalog: &catalog,
        version: 9,
        granted: &[()],
        environment: &environment,
    };
    assert_eq!(
        infer_call_flow_type(&one("a"), &[], &environment, &host, LIMITS.flow).err(),
        Some(Error::UnsupportedFlow)
    );
    assert!(environment.events.borrow().is_empty());
}

#[test]
fn group_callback_unwind_does_not_mutate_prefix_or_replay_and_leaves_later_checks_usable() {
    let environment = Environment::default();
    let expression = bind("g", one("a"), boolean(true));
    let prefix = [("prior", PureType::Result(Metadata::Receipt(1)))];
    for point in ["declaration", "group"] {
        environment.panic_on.set(Some(point));
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| check(
                &expression,
                &prefix,
                &environment,
                LIMITS
            )))
            .is_err()
        );
        assert!(matches!(
            prefix[0].1,
            PureType::Result(Metadata::Receipt(1))
        ));
        environment.panic_on.set(None);
        assert!(check(&expression, &prefix, &environment, LIMITS).is_ok());
    }
}

#[test]
fn group_failures_are_closed_payload_free_positions_not_restorable_handles() {
    let error = Error::Group {
        group_index: 3,
        error: GroupTypeError::MemberPreparation {
            branch_index: 7,
            error: PreparedCallTypeError::InvalidFlow,
        },
    };
    assert!(format!("{error:?}").contains("branch_index: 7"));
    assert_eq!(
        error.to_string(),
        "group type declaration or preparation is invalid"
    );
}

#[test]
fn cold_unknown_operation_and_uniform_row_checks_precede_unused_prefix_clone() {
    struct Metadata;
    impl Clone for Metadata {
        fn clone(&self) -> Self {
            panic!("unused prefix clone")
        }
    }
    impl PartialEq for Metadata {
        fn eq(&self, _: &Self) -> bool {
            panic!("native type equality")
        }
    }
    struct Poison;
    impl PureTypeEnvironment<u8, u32> for Poison {
        type Result = Metadata;
        fn field_type(&self, _: &Metadata, _: &u8) -> Option<ScalarType> {
            panic!("field query")
        }
        fn member_result(&self, _: &Metadata, _: &str, _: &u32) -> Option<Metadata> {
            panic!("member query")
        }
    }
    impl<'s> CallFlowEnvironment<'s, u8, u32, u8> for Poison {
        fn call_result_type(&self, _: &u32, _: &'s u8) -> Option<PureType<Metadata>> {
            panic!("result query")
        }
    }
    impl<'e, 's> GroupFlowEnvironment<'e, 's, u8, u32, u8, u8> for Poison {
        fn group_branch_type_matches(&self, _: &u8, _: &PureType<Metadata>) -> bool {
            panic!("declaration query")
        }
        fn group_result_type(
            &self,
            _: GroupKind,
            _: &[GroupMemberType<'e, u32, Metadata>],
        ) -> Option<Metadata> {
            panic!("group query")
        }
    }
    let parameters = [NamedParameter::required(
        "position",
        ScalarTypeSet::only(ScalarType::Integer),
    )];
    let rows = [
        OperationSchema {
            key: 17,
            parameters: &parameters,
            result: 1u8,
            required_capability: (),
        },
        OperationSchema {
            key: 18,
            parameters: &parameters,
            result: 1u8,
            required_capability: (),
        },
    ];
    let catalog = OperationCatalog::new(
        9,
        &rows,
        OperationCatalogLimits {
            max_operations: 2,
            max_parameters_per_operation: 1,
        },
    )
    .unwrap();
    let host = CallTypeHost {
        catalog: &catalog,
        version: 9,
        granted: &[()],
        environment: &Poison,
    };
    let prefix = [("unused", PureType::Result(Metadata))];
    let expression = choose(
        one("a"),
        group(
            GroupKind::Sequence,
            vec![branch("b", call(99, number(0)), 1)],
        ),
    );
    assert_eq!(
        infer_group_flow_type(&expression, &prefix, &Poison, &host, LIMITS).err(),
        Some(Error::Call {
            call_index: 1,
            error: CallTypeError::Catalog(OperationCatalogError::UnknownOperation)
        })
    );
    let expression = group(
        GroupKind::Sequence,
        vec![branch(
            "a",
            choose(call(17, number(0)), call(18, number(0))),
            1,
        )],
    );
    assert_eq!(
        infer_group_flow_type(&expression, &prefix, &Poison, &host, LIMITS).err(),
        Some(Error::Group {
            group_index: 0,
            error: GroupTypeError::InconsistentOperation { branch_index: 0 }
        })
    );
}

#[test]
fn unrelated_gui_group_profile_borrows_nonclone_native_ir_and_result_declarations() {
    struct Field;
    #[derive(PartialEq)]
    struct Operation(u32);
    struct HostEffect(Rc<()>);
    struct Declared(u8);
    struct Declaration {
        kind: u8,
        private: Rc<()>,
    }
    impl PartialEq for Declaration {
        fn eq(&self, other: &Self) -> bool {
            self.kind == other.kind
        }
    }
    #[derive(Clone, PartialEq)]
    enum Query<'e, 's> {
        Receipt(&'s Declaration),
        Group(Rc<[(&'e str, &'e Operation, &'s Declaration)]>),
    }
    struct Gui<'e, 's>(PhantomData<(&'e (), &'s ())>);
    impl<'e, 's> PureTypeEnvironment<Field, Operation> for Gui<'e, 's> {
        type Result = Query<'e, 's>;
        fn field_type(&self, result: &Self::Result, _: &Field) -> Option<ScalarType> {
            matches!(result, Query::Receipt(_)).then_some(ScalarType::String)
        }
        fn member_result(
            &self,
            group: &Self::Result,
            name: &str,
            operation: &Operation,
        ) -> Option<Self::Result> {
            let Query::Group(members) = group else {
                return None;
            };
            let row = members
                .iter()
                .find(|row| row.0 == name && row.1 == operation)?;
            Some(Query::Receipt(row.2))
        }
    }
    impl<'e, 's> CallFlowEnvironment<'s, Field, Operation, Declaration> for Gui<'e, 's> {
        fn call_result_type(
            &self,
            _: &Operation,
            declaration: &'s Declaration,
        ) -> Option<PureType<Self::Result>> {
            Some(PureType::Result(Query::Receipt(declaration)))
        }
    }
    impl<'e, 's> GroupFlowEnvironment<'e, 's, Field, Operation, Declaration, Declared> for Gui<'e, 's> {
        fn group_branch_type_matches(
            &self,
            declaration: &Declared,
            inferred: &PureType<Self::Result>,
        ) -> bool {
            matches!(inferred, PureType::Result(Query::Receipt(result)) if declaration.0 == result.kind)
        }
        fn group_result_type(
            &self,
            _: GroupKind,
            members: &[GroupMemberType<'e, Operation, Self::Result>],
        ) -> Option<Self::Result> {
            let rows = members
                .iter()
                .map(|member| {
                    let PureType::Result(Query::Receipt(declaration)) = &member.inferred else {
                        return None;
                    };
                    Some((member.name, member.operation, *declaration))
                })
                .collect::<Option<Rc<[_]>>>()?;
            Some(Query::Group(rows))
        }
    }
    type GuiIr = Computation<Field, Operation, HostEffect, Declared>;
    let rows = [OperationSchema {
        key: Operation(7),
        parameters: &[] as &[NamedParameter<&str, ScalarTypeSet>],
        result: Declaration {
            kind: 1,
            private: Rc::new(()),
        },
        required_capability: (),
    }];
    let catalog = OperationCatalog::new(
        9,
        &rows,
        OperationCatalogLimits {
            max_operations: 1,
            max_parameters_per_operation: 0,
        },
    )
    .unwrap();
    let gui = Gui(PhantomData);
    let host = CallTypeHost {
        catalog: &catalog,
        version: 9,
        granted: &[()],
        environment: &gui,
    };
    let expression = GuiIr::Group {
        group_kind: GroupKind::Sequence,
        branches: vec![ComputedBranch {
            name: "window".into(),
            value: GuiIr::Call {
                operation: Operation(7),
                arguments: vec![],
            },
            result_type: Declared(1),
        }],
    };
    let result = infer_group_flow_type(&expression, &[], &gui, &host, LIMITS)
        .ok()
        .unwrap();
    let PureType::Result(Query::Group(members)) = result else {
        panic!()
    };
    let GuiIr::Group { branches, .. } = &expression else {
        panic!()
    };
    assert!(std::ptr::eq(members[0].0, branches[0].name.as_str()));
    assert!(std::ptr::eq(members[0].2, &rows[0].result));
    assert_eq!(Rc::strong_count(&rows[0].result.private), 1);
    let effect = HostEffect(Rc::new(()));
    assert_eq!(Rc::strong_count(&effect.0), 1);
}
