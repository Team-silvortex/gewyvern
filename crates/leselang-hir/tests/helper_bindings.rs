use std::cell::RefCell;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;

use leselang_hir::call_evaluation::{
    CallEvaluationHost, CallEvaluationLimits, prepare_call_in_scope,
};
use leselang_hir::helper_bindings::*;
use leselang_hir::helper_dependencies::{HelperDependencyLimits, helper_dependency_order};
use leselang_hir::helper_hygiene::{HelperHygieneLimits, hygienic_helper_body};
use leselang_hir::helper_source::{
    HelperSignature, HelperSourceLimits, helper_parameters, lower_helper_arguments,
};
use leselang_hir::ir::{Computation, ComputedArgument, ComputedBranch, GroupKind};
use leselang_hir::pure_evaluation::*;
use leselang_hir::pure_typing::{
    PureType, PureTypeEnvironment, PureTypeError, TypeInferenceLimits, infer_pure_type,
};
use leselang_hir::scalar_source::{ScalarSourceLimits, lower_scalar_source_with_scope};
use leselang_hir::source_call::SourceCallLimits;
use leselang_runtime_core::*;

// All four native slots deliberately lack Clone/Debug/serde/Send.
struct Native {
    id: &'static str,
    buffer: Vec<u8>,
    drops: Rc<RefCell<Vec<&'static str>>>,
}
impl Drop for Native {
    fn drop(&mut self) {
        self.drops.borrow_mut().push(self.id);
    }
}
struct PrivateError;
type Node = Computation<Native, Native, Native, Native>;
const LIMITS: HelperBindingLimits = HelperBindingLimits {
    max_nodes: 256,
    max_depth: 32,
    max_parameters: 8,
};
const TYPES: TypeInferenceLimits = TypeInferenceLimits {
    max_nodes: 256,
    max_depth: 32,
    max_bindings: 16,
};
const PURE: PureEvaluationLimits = PureEvaluationLimits {
    max_nodes: 256,
    max_depth: 32,
    max_bindings: 16,
};
struct Host(RefCell<Vec<&'static str>>);
impl PureTypeEnvironment<Native, Native> for Host {
    type Result = ();
    fn field_type(&self, _: &(), _: &Native) -> Option<ScalarType> {
        Some(ScalarType::Integer)
    }
    fn member_result(&self, _: &(), _: &str, _: &Native) -> Option<()> {
        Some(())
    }
}
impl PureEvaluationEnvironment<Native, Native> for Host {
    type Result = ();
    type Error = PrivateError;
    fn field(&self, _: &(), field: &Native) -> Result<ScalarValue, PrivateError> {
        self.0.borrow_mut().push(field.id);
        match field.id {
            "fail" => Err(PrivateError),
            "panic" => panic!("trusted native projection unwind"),
            "first" => Ok(ScalarValue::Integer(9)),
            _ => Ok(ScalarValue::Integer(2)),
        }
    }
    fn member(&self, _: &(), _: &str, operation: &Native) -> Result<(), PrivateError> {
        self.0.borrow_mut().push(operation.id);
        Ok(())
    }
}
fn native(id: &'static str, drops: &Rc<RefCell<Vec<&'static str>>>) -> Native {
    Native {
        id,
        buffer: vec![4; 19],
        drops: drops.clone(),
    }
}
fn integer(value: u64) -> Node {
    Node::Literal {
        value: ScalarValue::Integer(value),
    }
}
fn local(name: &str) -> Node {
    Node::Local { name: name.into() }
}
fn binding(name: &str, value: Node) -> HelperBinding<Node> {
    HelperBinding {
        name: name.into(),
        value,
    }
}
fn bind(name: &str, value: Node, body: Node) -> Node {
    Node::Bind {
        name: name.into(),
        value: Box::new(value),
        body: Box::new(body),
    }
}
fn choose(otherwise: Node) -> Node {
    Node::Choose {
        when: Box::new(Node::Literal {
            value: ScalarValue::Boolean(true),
        }),
        then: Box::new(integer(0)),
        otherwise: Box::new(otherwise),
    }
}
fn field(id: &'static str, drops: &Rc<RefCell<Vec<&'static str>>>) -> Node {
    Node::Field {
        value: Box::new(local("reply")),
        field: native(id, drops),
    }
}
fn rejected(result: Result<Node, HelperBindingError>) -> HelperBindingError {
    match result {
        Err(error) => error,
        Ok(_) => panic!("expected binding rejection"),
    }
}
fn eval(
    node: &Node,
    host: &Host,
    fuel: &mut Fuel,
) -> Result<PureValue<()>, PureEvaluationFailure<PrivateError>> {
    let mut values = vec![
        ("reply", PureValue::Result(())),
        ("caller", PureValue::Scalar(ScalarValue::Integer(7))),
    ];
    let mut scope = ScopeFrame::new(&mut values);
    let result = evaluate_pure_in_scope(node, &mut scope, host, fuel, PURE);
    assert_eq!(scope.len(), 2);
    assert!(scope.get("_first").is_none());
    assert!(scope.get("_second").is_none());
    assert!(matches!(
        scope.get("caller"),
        Some(PureValue::Scalar(ScalarValue::Integer(7)))
    ));
    result
}

#[test]
fn empty_binding_policy_and_zero_leaf_bounds_are_explicit() {
    assert!(matches!(
        bind_helper_arguments(
            integer(4),
            vec![],
            HelperBindingLimits {
                max_nodes: 1,
                max_depth: 0,
                max_parameters: 0
            }
        )
        .unwrap(),
        Node::Literal {
            value: ScalarValue::Integer(4)
        }
    ));
    assert_eq!(
        rejected(bind_helper_arguments(
            integer(0),
            vec![],
            HelperBindingLimits {
                max_nodes: 0,
                max_depth: 0,
                max_parameters: 0
            }
        )),
        HelperBindingError::Wrappers(StructureError::NodeLimit)
    );
    assert_eq!(
        rejected(bind_helper_arguments(
            integer(0),
            vec![binding("_p", integer(1))],
            HelperBindingLimits {
                max_parameters: 0,
                ..LIMITS
            }
        )),
        HelperBindingError::ParameterLimit
    );
}

#[test]
fn safety_ceilings_and_parameter_count_precede_all_body_work() {
    for limits in [
        HelperBindingLimits {
            max_nodes: 16_385,
            ..LIMITS
        },
        HelperBindingLimits {
            max_depth: 65,
            ..LIMITS
        },
        HelperBindingLimits {
            max_parameters: 9,
            ..LIMITS
        },
    ] {
        assert_eq!(
            rejected(bind_helper_arguments(integer(0), vec![], limits)),
            HelperBindingError::InvalidLimits
        );
    }
    assert_eq!(
        rejected(bind_helper_arguments(
            integer(0),
            (0..9).map(|_| binding("bad name", integer(0))).collect(),
            LIMITS
        )),
        HelperBindingError::ParameterLimit
    );
}

#[test]
fn wrapper_roots_and_each_operand_and_body_charge_one_aggregate_meter() {
    assert!(
        bind_helper_arguments(
            local("_p"),
            vec![binding("_p", integer(1))],
            HelperBindingLimits {
                max_nodes: 3,
                max_depth: 1,
                ..LIMITS
            }
        )
        .is_ok()
    );
    assert_eq!(
        rejected(bind_helper_arguments(
            local("_p"),
            vec![binding("_p", integer(1))],
            HelperBindingLimits {
                max_nodes: 2,
                ..LIMITS
            }
        )),
        HelperBindingError::Wrappers(StructureError::NodeLimit)
    );
    assert!(matches!(
        rejected(bind_helper_arguments(
            local("_p"),
            vec![binding("_p", integer(1))],
            HelperBindingLimits {
                max_depth: 0,
                ..LIMITS
            }
        )),
        HelperBindingError::Argument {
            index: 0,
            error: PureTypeError::Structure(StructureError::DepthLimit)
        }
    ));
}

#[test]
fn eight_parameters_fit_exactly_seventeen_nodes_and_depth_eight() {
    let bindings = || {
        (0..8)
            .map(|i| binding(&format!("_p{i}"), integer(i)))
            .collect()
    };
    assert!(
        bind_helper_arguments(
            local("_p7"),
            bindings(),
            HelperBindingLimits {
                max_nodes: 17,
                max_depth: 8,
                ..LIMITS
            }
        )
        .is_ok()
    );
    assert!(
        bind_helper_arguments(
            local("_p7"),
            bindings(),
            HelperBindingLimits {
                max_nodes: 16,
                max_depth: 8,
                ..LIMITS
            }
        )
        .is_err()
    );
    assert!(
        bind_helper_arguments(
            local("_p7"),
            bindings(),
            HelperBindingLimits {
                max_nodes: 17,
                max_depth: 7,
                ..LIMITS
            }
        )
        .is_err()
    );
}

#[test]
fn operand_positions_and_cold_body_depth_include_every_outer_wrapper() {
    let output = || vec![binding("_a", integer(0)), binding("_b", choose(integer(1)))];
    assert!(
        bind_helper_arguments(
            integer(0),
            output(),
            HelperBindingLimits {
                max_depth: 3,
                ..LIMITS
            }
        )
        .is_ok()
    );
    assert_eq!(
        rejected(bind_helper_arguments(
            integer(0),
            output(),
            HelperBindingLimits {
                max_depth: 2,
                ..LIMITS
            }
        )),
        HelperBindingError::Argument {
            index: 1,
            error: PureTypeError::Structure(StructureError::DepthLimit)
        }
    );
    assert_eq!(
        rejected(bind_helper_arguments(
            choose(choose(integer(0))),
            vec![binding("_a", integer(0))],
            HelperBindingLimits {
                max_depth: 2,
                ..LIMITS
            }
        )),
        HelperBindingError::Body(StructureError::DepthLimit)
    );
}

#[test]
fn operand_forests_do_not_receive_independent_node_budgets() {
    let values = || {
        vec![
            binding("_a", choose(integer(1))),
            binding("_b", choose(integer(2))),
        ]
    };
    assert!(
        bind_helper_arguments(
            integer(0),
            values(),
            HelperBindingLimits {
                max_nodes: 11,
                ..LIMITS
            }
        )
        .is_ok()
    );
    assert!(
        bind_helper_arguments(
            integer(0),
            values(),
            HelperBindingLimits {
                max_nodes: 10,
                ..LIMITS
            }
        )
        .is_err()
    );
}

#[test]
fn invalid_duplicate_aliases_preserve_indices_and_redact_payloads() {
    let error = rejected(bind_helper_arguments(
        integer(0),
        vec![
            binding("_first", integer(0)),
            binding("private bad alias", integer(0)),
        ],
        LIMITS,
    ));
    assert_eq!(error, HelperBindingError::InvalidName { index: 1 });
    assert!(!format!("{error:?} {error}").contains("private"));
    assert_eq!(
        rejected(bind_helper_arguments(
            integer(0),
            vec![binding("_first", integer(0)), binding("_first", integer(1))],
            LIMITS
        )),
        HelperBindingError::DuplicateName { index: 1 }
    );
}

#[test]
fn every_alias_is_fenced_from_all_cold_caller_operand_names() {
    for value in [
        local("_first"),
        choose(local("_first")),
        bind("_first", integer(0), integer(0)),
        Node::Loop {
            name: "_first".into(),
            initial: Box::new(integer(0)),
            condition: Box::new(Node::Literal {
                value: ScalarValue::Boolean(false),
            }),
            next: Box::new(integer(0)),
            limit: 0,
        },
        Node::Fold {
            name: "state".into(),
            item: "_first".into(),
            items: Box::new(Node::Strings { items: vec![] }),
            initial: Box::new(integer(0)),
            next: Box::new(integer(0)),
            limit: 0,
        },
    ] {
        assert_eq!(
            rejected(bind_helper_arguments(
                integer(0),
                vec![binding("_first", integer(1)), binding("_second", value)],
                LIMITS
            )),
            HelperBindingError::ArgumentCapture { index: 1 }
        );
    }
    let drops = Rc::new(RefCell::new(Vec::new()));
    let value = Node::Member {
        group: "_first".into(),
        name: "one".into(),
        operation: native("member", &drops),
    };
    assert_eq!(
        rejected(bind_helper_arguments(
            integer(0),
            vec![binding("_first", value)],
            LIMITS
        )),
        HelperBindingError::ArgumentCapture { index: 0 }
    );
    assert_eq!(*drops.borrow(), ["member"]);
}

#[test]
fn cold_body_bind_loop_and_fold_declarations_cannot_shadow_parameters() {
    for body in [
        choose(bind("_p", integer(0), integer(0))),
        Node::Loop {
            name: "_p".into(),
            initial: Box::new(integer(0)),
            condition: Box::new(Node::Literal {
                value: ScalarValue::Boolean(false),
            }),
            next: Box::new(integer(0)),
            limit: 0,
        },
        Node::Fold {
            name: "state".into(),
            item: "_p".into(),
            items: Box::new(Node::Strings { items: vec![] }),
            initial: Box::new(integer(0)),
            next: Box::new(integer(0)),
            limit: 0,
        },
    ] {
        assert_eq!(
            rejected(bind_helper_arguments(
                body,
                vec![binding("_p", integer(0))],
                LIMITS
            )),
            HelperBindingError::ShadowedParameter
        );
    }
}

#[test]
fn body_frontier_is_checked_before_growth_including_cold_group_children() {
    let drops = Rc::new(RefCell::new(Vec::new()));
    let body = Node::Group {
        group_kind: GroupKind::Parallel,
        branches: (0..64)
            .map(|i| ComputedBranch {
                name: format!("branch{i}"),
                value: integer(0),
                result_type: native("result", &drops),
            })
            .collect(),
    };
    assert_eq!(
        rejected(bind_helper_arguments(
            body,
            vec![],
            HelperBindingLimits {
                max_nodes: 4,
                ..LIMITS
            }
        )),
        HelperBindingError::Body(StructureError::NodeLimit)
    );
    assert_eq!(drops.borrow().len(), 64);
}

#[test]
fn native_projection_boxes_buffers_and_owned_alias_strings_move_unchanged() {
    let drops = Rc::new(RefCell::new(Vec::new()));
    let operation = native("member", &drops);
    let member_buffer = operation.buffer.as_ptr();
    let projection = native("field", &drops);
    let field_buffer = projection.buffer.as_ptr();
    let member = Box::new(Node::Member {
        group: "reply".into(),
        name: "one".into(),
        operation,
    });
    let member_box = member.as_ref() as *const Node;
    let operand = Node::Field {
        value: member,
        field: projection,
    };
    let alias = String::from("_p");
    let alias_buffer = alias.as_ptr();
    let body_value = Box::new(local("_p"));
    let body_box = body_value.as_ref() as *const Node;
    let output = bind_helper_arguments(
        Node::Unary {
            operator: UnaryOperator::ToString,
            value: body_value,
        },
        vec![HelperBinding {
            name: alias,
            value: operand,
        }],
        LIMITS,
    )
    .unwrap();
    let Node::Bind { name, value, body } = &output else {
        panic!()
    };
    assert_eq!(name.as_ptr(), alias_buffer);
    let Node::Field { value, field } = value.as_ref() else {
        panic!()
    };
    assert_eq!(value.as_ref() as *const Node, member_box);
    assert_eq!(field.buffer.as_ptr(), field_buffer);
    let Node::Member { operation, .. } = value.as_ref() else {
        panic!()
    };
    assert_eq!(operation.buffer.as_ptr(), member_buffer);
    let Node::Unary { value, .. } = body.as_ref() else {
        panic!()
    };
    assert_eq!(value.as_ref() as *const Node, body_box);
    assert!(drops.borrow().is_empty());
    drop(output);
    let mut recorded = drops.borrow().clone();
    recorded.sort();
    assert_eq!(recorded, ["field", "member"]);
}

#[test]
fn opaque_host_call_and_group_body_slots_are_not_rebuilt_or_dispatched() {
    let drops = Rc::new(RefCell::new(Vec::new()));
    let effect = Box::new(native("effect", &drops));
    let effect_box = effect.as_ref() as *const Native;
    let arguments = vec![ComputedArgument {
        name: "input".into(),
        value: local("_p"),
    }];
    let argument_buffer = arguments.as_ptr();
    let branches = vec![
        ComputedBranch {
            name: "first".into(),
            value: Node::Host { effect },
            result_type: native("result_a", &drops),
        },
        ComputedBranch {
            name: "second".into(),
            value: Node::Call {
                operation: native("operation", &drops),
                arguments,
            },
            result_type: native("result_b", &drops),
        },
    ];
    let branch_buffer = branches.as_ptr();
    let output = bind_helper_arguments(
        Node::Group {
            group_kind: GroupKind::Sequence,
            branches,
        },
        vec![binding("_p", integer(1))],
        LIMITS,
    )
    .unwrap();
    let Node::Bind { body, .. } = &output else {
        panic!()
    };
    let Node::Group { branches, .. } = body.as_ref() else {
        panic!()
    };
    assert_eq!(branches.as_ptr(), branch_buffer);
    let Node::Host { effect } = &branches[0].value else {
        panic!()
    };
    assert_eq!(effect.as_ref() as *const Native, effect_box);
    let Node::Call { arguments, .. } = &branches[1].value else {
        panic!()
    };
    assert_eq!(arguments.as_ptr(), argument_buffer);
    assert!(drops.borrow().is_empty());
    drop(output);
    assert_eq!(drops.borrow().len(), 4);
}

#[test]
fn cold_impure_operands_are_rejected_and_all_owned_slots_drop_once() {
    let drops = Rc::new(RefCell::new(Vec::new()));
    for effect in [
        Node::Host {
            effect: Box::new(native("host", &drops)),
        },
        Node::Call {
            operation: native("call", &drops),
            arguments: vec![],
        },
        Node::Group {
            group_kind: GroupKind::Parallel,
            branches: vec![ComputedBranch {
                name: "one".into(),
                value: integer(0),
                result_type: native("group", &drops),
            }],
        },
    ] {
        assert_eq!(
            rejected(bind_helper_arguments(
                Node::Host {
                    effect: Box::new(native("body", &drops))
                },
                vec![binding("_p", choose(effect))],
                LIMITS
            )),
            HelperBindingError::Argument {
                index: 0,
                error: PureTypeError::Impure
            }
        );
    }
    let mut recorded = drops.borrow().clone();
    recorded.sort();
    assert_eq!(recorded, ["body", "body", "body", "call", "group", "host"]);
}

#[test]
fn bounded_literals_names_and_control_limits_apply_even_to_unused_arguments() {
    for (value, error) in [
        (
            Node::Literal {
                value: ScalarValue::String("x".repeat(4097)),
            },
            PureTypeError::UnboundedLiteral,
        ),
        (local("bad name"), PureTypeError::InvalidName),
        (
            Node::Strings {
                items: (0..65).map(|_| integer(0)).collect(),
            },
            PureTypeError::StringListLimit,
        ),
        (
            Node::Loop {
                name: "state".into(),
                initial: Box::new(integer(0)),
                condition: Box::new(Node::Literal {
                    value: ScalarValue::Boolean(false),
                }),
                next: Box::new(integer(0)),
                limit: 1025,
            },
            PureTypeError::LoopLimit,
        ),
    ] {
        assert_eq!(
            rejected(bind_helper_arguments(
                integer(0),
                vec![binding("_p", value)],
                LIMITS
            )),
            HelperBindingError::Argument { index: 0, error }
        );
    }
}

#[test]
fn output_shape_is_not_complete_cold_lexical_or_scalar_type_acceptance() {
    let host = Host(RefCell::new(Vec::new()));
    let node = bind_helper_arguments(
        choose(local("unbound")),
        vec![binding("_p", integer(0))],
        LIMITS,
    )
    .unwrap();
    assert_eq!(
        infer_pure_type(&node, &[], &host, TYPES),
        Err(PureTypeError::UnknownLocal)
    );
    let node =
        bind_helper_arguments(local("_p"), vec![binding("_p", local("missing"))], LIMITS).unwrap();
    assert_eq!(
        infer_pure_type(&node, &[], &host, TYPES),
        Err(PureTypeError::UnknownLocal)
    );
    let node = bind_helper_arguments(
        choose(Node::Literal {
            value: ScalarValue::String("wrong branch".into()),
        }),
        vec![],
        LIMITS,
    )
    .unwrap();
    assert_eq!(
        infer_pure_type(&node, &[], &host, TYPES),
        Err(PureTypeError::BranchTypes)
    );
    let node = bind_helper_arguments(local("_p"), vec![binding("_p", integer(0))], LIMITS).unwrap();
    assert_eq!(
        infer_pure_type(
            &node,
            &[("_p", PureType::Scalar(ScalarType::Integer))],
            &host,
            TYPES
        ),
        Err(PureTypeError::ShadowedBinding)
    );
    let node = bind_helper_arguments(
        Node::Literal {
            value: ScalarValue::String("x".repeat(4097)),
        },
        vec![],
        LIMITS,
    )
    .unwrap();
    assert_eq!(
        infer_pure_type(&node, &[], &host, TYPES),
        Err(PureTypeError::UnboundedLiteral)
    );
    assert!(host.0.borrow().is_empty());
}

#[test]
fn arguments_evaluate_once_in_declaration_order_with_exact_fuel_and_prefix_cleanup() {
    let drops = Rc::new(RefCell::new(Vec::new()));
    let body = || Node::Binary {
        operator: BinaryOperator::Add,
        left: Box::new(local("_first")),
        right: Box::new(local("_first")),
    };
    let output = bind_helper_arguments(
        body(),
        vec![
            binding("_first", field("first", &drops)),
            binding("_second", field("second", &drops)),
        ],
        LIMITS,
    )
    .unwrap();
    let original = bind(
        "_first",
        field("first", &drops),
        bind("_second", field("second", &drops), body()),
    );
    let mut remaining = Vec::new();
    for node in [&original, &output] {
        let host = Host(RefCell::new(Vec::new()));
        assert_eq!(
            infer_pure_type(node, &[("reply", PureType::Result(()))], &host, TYPES),
            Ok(PureType::Scalar(ScalarType::Integer))
        );
        assert!(host.0.borrow().is_empty());
        let mut fuel = Fuel::new(100);
        assert!(matches!(
            eval(node, &host, &mut fuel).unwrap(),
            PureValue::Scalar(ScalarValue::Integer(18))
        ));
        assert_eq!(*host.0.borrow(), ["first", "second"]);
        remaining.push(fuel.remaining());
    }
    assert_eq!(remaining[0], remaining[1]);
    assert!(remaining[0] < 100);
}

#[test]
fn unused_argument_failure_and_native_unwind_are_not_skipped_or_retried() {
    for failing in ["fail", "panic"] {
        let drops = Rc::new(RefCell::new(Vec::new()));
        let host = Host(RefCell::new(Vec::new()));
        let node = bind_helper_arguments(
            integer(0),
            vec![
                binding("_first", field("first", &drops)),
                binding("_second", field(failing, &drops)),
            ],
            LIMITS,
        )
        .unwrap();
        assert!(host.0.borrow().is_empty());
        let mut values = vec![("reply", PureValue::Result(()))];
        let mut scope = ScopeFrame::new(&mut values);
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            evaluate_pure_in_scope(&node, &mut scope, &host, &mut Fuel::new(100), PURE)
        }));
        if failing == "panic" {
            assert!(outcome.is_err());
        } else {
            assert!(matches!(
                outcome.unwrap(),
                Err(CalculationFailure::External(PureEvaluationFault::Native(_)))
            ));
        }
        assert_eq!(scope.len(), 1);
        assert!(scope.get("_first").is_none());
        assert!(scope.get("_second").is_none());
        assert_eq!(*host.0.borrow(), ["first", failing]);
        assert!(drops.borrow().is_empty());
        drop(scope);
        drop(node);
        assert_eq!(drops.borrow().len(), 2);
    }
}

#[test]
fn original_caller_bindings_and_literal_buffers_are_not_renamed_or_copied() {
    let text = String::from("original text");
    let buffer = text.as_ptr();
    let output = bind_helper_arguments(
        local("_p"),
        vec![binding(
            "_p",
            bind(
                "caller_local",
                Node::Literal {
                    value: ScalarValue::String(text),
                },
                local("caller_local"),
            ),
        )],
        LIMITS,
    )
    .unwrap();
    let Node::Bind { value, .. } = &output else {
        panic!()
    };
    let Node::Bind { name, value, body } = value.as_ref() else {
        panic!()
    };
    assert_eq!(name, "caller_local");
    assert!(matches!(body.as_ref(), Node::Local { name } if name == "caller_local"));
    let Node::Literal {
        value: ScalarValue::String(text),
    } = value.as_ref()
    else {
        panic!()
    };
    assert_eq!(text.as_ptr(), buffer);
}

struct Device;
impl PureTypeEnvironment<u8, u32> for Device {
    type Result = ();
    fn field_type(&self, _: &(), _: &u8) -> Option<ScalarType> {
        None
    }
    fn member_result(&self, _: &(), _: &str, _: &u32) -> Option<()> {
        None
    }
}
impl PureEvaluationEnvironment<u8, u32> for Device {
    type Result = ();
    type Error = ();
    fn field(&self, _: &(), _: &u8) -> Result<ScalarValue, ()> {
        Err(())
    }
    fn member(&self, _: &(), _: &str, _: &u32) -> Result<(), ()> {
        Err(())
    }
}

#[test]
fn parsed_dependency_signature_arguments_hygiene_and_wrappers_prepare_an_unrelated_native_call() {
    type DeviceNode = Computation<u8, u32, (), ()>;
    let tree = leselang_syntax::parse(
        "fn work(n: integer) = add(left: n, right: 1)\nfn launch() = work(n: 41)",
    );
    assert!(tree.diagnostics.is_empty());
    let helper = tree.function.as_ref().unwrap();
    let mut order = helper_dependency_order(
        &[helper],
        "launch",
        HelperDependencyLimits {
            max_helpers: 1,
            max_source_nodes: 128,
            max_source_depth: 16,
        },
    )
    .unwrap();
    let helper = order.next().unwrap().unwrap();
    let parameters = helper_parameters(helper, 8).unwrap();
    let source = SourceCallLimits {
        max_source_nodes: 128,
        max_source_depth: 16,
        max_lowered_nodes: 128,
        max_lowered_depth: 16,
        max_arguments: 64,
    };
    let lowered = lower_helper_arguments(
        &tree.helpers[0].body,
        HelperSignature {
            name: &helper.name,
            parameters: &parameters,
        },
        HelperSourceLimits {
            source,
            max_parameters: 8,
        },
        |argument| {
            lower_scalar_source_with_scope(
                &argument.value,
                ScalarSourceLimits {
                    source,
                    max_bindings: 8,
                },
                &[],
                |_, _| -> Result<(DeviceNode, Option<ScalarType>), ()> { Err(()) },
            )
            .map(|(value, ty)| (value, Some(ty)))
        },
    )
    .unwrap();
    assert!(std::ptr::eq(lowered[0].parameter, &parameters[0]));
    let body = lower_scalar_source_with_scope(
        &helper.body,
        ScalarSourceLimits {
            source,
            max_bindings: 8,
        },
        &[(parameters[0].name, parameters[0].domain)],
        |_, _| -> Result<(DeviceNode, Option<ScalarType>), ()> { Err(()) },
    )
    .unwrap()
    .0;
    let body = hygienic_helper_body(
        body,
        &[("n", "_p")],
        &[],
        HelperHygieneLimits {
            max_nodes: 128,
            max_depth: 16,
            max_bindings: 8,
            max_reserved_names: 0,
        },
        || Err::<String, ()>(()),
    )
    .unwrap();
    let value = bind_helper_arguments(
        body,
        lowered
            .into_iter()
            .map(|argument| HelperBinding {
                name: "_p".into(),
                value: argument.value,
            })
            .collect(),
        LIMITS,
    )
    .unwrap();
    assert_eq!(
        infer_pure_type(&value, &[], &Device, TYPES),
        Ok(PureType::Scalar(ScalarType::Integer))
    );
    let parameters = [NamedParameter::required(
        "position",
        ScalarTypeSet::only(ScalarType::Integer),
    )];
    let schemas = [OperationSchema {
        key: 17u32,
        parameters: &parameters,
        result: (),
        required_capability: 31u8,
    }];
    let catalog = OperationCatalog::new(
        3,
        &schemas,
        OperationCatalogLimits {
            max_operations: 1,
            max_parameters_per_operation: 1,
        },
    )
    .unwrap();
    let call: DeviceNode = DeviceNode::Call {
        operation: 17,
        arguments: vec![ComputedArgument {
            name: "position".into(),
            value,
        }],
    };
    let host = CallEvaluationHost {
        catalog: &catalog,
        version: 3,
        granted: &[31],
        environment: &Device,
    };
    let mut values = Vec::new();
    let mut scope = ScopeFrame::new(&mut values);
    let mut fuel = Fuel::new(100);
    let prepared = prepare_call_in_scope(
        &call,
        &mut scope,
        &host,
        &mut fuel,
        CallEvaluationLimits {
            pure: PURE,
            max_arguments: 1,
        },
    )
    .unwrap();
    assert!(std::ptr::eq(prepared.schema(), &schemas[0]));
    assert!(matches!(
        prepared.arguments()[0].value,
        ScalarValue::Integer(42)
    ));
    assert!(scope.is_empty());
    assert!(order.next().is_none());
}

#[test]
fn reference_wrapper_wire_authority_and_legacy_expansion_bounds_are_preserved() {
    let source = "fn f(n: integer) = ui.focus(node_id: to_string(value: n))\nfn main() = f(n: 7)";
    let program = leselang_hir::lower(&leselang_syntax::parse(source)).unwrap();
    let canonical = leselang_hir::canonical_source(&program.function.effect).unwrap();
    let expected = leselang_syntax::format(&leselang_syntax::parse(
        "fn main() = bind(_lf0: 7, body: ui.focus(node_id: to_string(value: _lf0)))",
    ))
    .unwrap();
    assert_eq!(canonical, expected);
    assert_eq!(
        serde_json::to_vec(&program).unwrap(),
        serde_json::to_vec(&leselang_hir::lower(&leselang_syntax::parse(&canonical)).unwrap())
            .unwrap()
    );
    assert_eq!(program.function.required_capabilities, ["ui.presentation"]);
    assert!(
        leselang_hir::authorize(&program, &leselang_host_contract::CapabilitySet::default())
            .is_err()
    );
    let mut body = "n".to_owned();
    for i in 0..16 {
        body = format!("bind(v{i}: 0, body: {body})");
    }
    let source = format!("fn deep(n: integer) = {body}\nfn main() = deep(n: 0)");
    let errors = leselang_hir::lower(&leselang_syntax::parse(&source)).unwrap_err();
    assert!(
        errors
            .iter()
            .any(|error| error.code == "LSH1405" && error.span.is_some())
    );
}
