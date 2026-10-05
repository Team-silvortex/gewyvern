use std::cell::{Cell, RefCell};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;

use leselang_hir::helper_hygiene::{HelperHygieneError, HelperHygieneLimits, hygienic_helper_body};
use leselang_hir::ir::{Computation, ComputedArgument, ComputedBranch, GroupKind};
use leselang_hir::pure_evaluation::{
    PureEvaluationEnvironment, PureEvaluationLimits, PureValue, evaluate_pure_in_scope,
};
use leselang_hir::pure_typing::{
    PureType, PureTypeEnvironment, PureTypeError, TypeInferenceLimits, infer_pure_type,
};
use leselang_hir::scalar_source::{ScalarSourceLimits, lower_scalar_source_with_scope};
use leselang_hir::source_call::SourceCallLimits;
use leselang_runtime_core::*;
use leselang_syntax::parse;

// None of the four native slots implements Clone/Debug/serde/Send.
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
struct PrivateError(&'static str);
type Node = Computation<Native, Native, Native, Native>;
const LIMITS: HelperHygieneLimits = HelperHygieneLimits {
    max_nodes: 256,
    max_depth: 32,
    max_bindings: 8,
    max_reserved_names: 8,
};
const SOURCE: ScalarSourceLimits = ScalarSourceLimits {
    source: SourceCallLimits {
        max_source_nodes: 256,
        max_source_depth: 32,
        max_lowered_nodes: 256,
        max_lowered_depth: 32,
        max_arguments: 64,
    },
    max_bindings: 8,
};
const TYPING: TypeInferenceLimits = TypeInferenceLimits {
    max_nodes: 256,
    max_depth: 32,
    max_bindings: 8,
};
const PURE: PureEvaluationLimits = PureEvaluationLimits {
    max_nodes: 256,
    max_depth: 32,
    max_bindings: 8,
};
struct NoHost;
impl PureTypeEnvironment<Native, Native> for NoHost {
    type Result = ();
    fn field_type(&self, _: &(), _: &Native) -> Option<ScalarType> {
        None
    }
    fn member_result(&self, _: &(), _: &str, _: &Native) -> Option<()> {
        None
    }
}
impl PureEvaluationEnvironment<Native, Native> for NoHost {
    type Result = ();
    type Error = PrivateError;
    fn field(&self, _: &(), _: &Native) -> Result<ScalarValue, PrivateError> {
        Err(PrivateError("private native field"))
    }
    fn member(&self, _: &(), _: &str, _: &Native) -> Result<(), PrivateError> {
        Err(PrivateError("private native member"))
    }
}
fn native(id: &'static str, drops: &Rc<RefCell<Vec<&'static str>>>) -> Native {
    Native {
        id,
        buffer: vec![5; 19],
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
fn binding(name: &str, value: Node, body: Node) -> Node {
    Node::Bind {
        name: name.into(),
        value: Box::new(value),
        body: Box::new(body),
    }
}
fn parsed(source: &str, prefix: &[(&str, ScalarType)]) -> Node {
    let tree = parse(&format!("fn main() = {source}"));
    assert!(tree.diagnostics.is_empty(), "{:?}", tree.diagnostics);
    lower_scalar_source_with_scope(
        &tree.function.unwrap().body,
        SOURCE,
        prefix,
        |_, _| -> Result<_, PrivateError> { Err(PrivateError("private unadapted expression")) },
    )
    .unwrap()
    .0
}
fn fresh(counter: &Cell<usize>) -> Result<String, PrivateError> {
    let value = counter.get();
    counter.set(value + 1);
    Ok(format!("_h{value}"))
}
fn run(node: &Node, prefix: &[(&str, ScalarValue)], fuel: &mut Fuel) -> ScalarValue {
    let mut bindings = prefix
        .iter()
        .map(|(name, value)| (*name, PureValue::Scalar(value.clone())))
        .collect();
    let mut scope = ScopeFrame::new(&mut bindings);
    let result = evaluate_pure_in_scope(node, &mut scope, &NoHost, fuel, PURE).unwrap();
    assert_eq!(scope.len(), prefix.len());
    for (name, value) in prefix {
        assert!(matches!(scope.get(name), Some(PureValue::Scalar(actual)) if actual == value));
    }
    match result {
        PureValue::Scalar(value) => value,
        _ => unreachable!(),
    }
}

#[test]
fn all_six_scalar_parameters_keep_argument_caller_scope_and_actual_values() {
    for value in [
        ScalarValue::Integer(7),
        ScalarValue::Boolean(true),
        ScalarValue::String("ready".into()),
        ScalarValue::None,
        ScalarValue::OptionalString(OptionalStringValue(Some("hint".into()))),
        ScalarValue::StringList(StringListValue(vec!["a".into(), "b".into()])),
    ] {
        let body = parsed("x", &[("x", value.scalar_type())]);
        let counter = Cell::new(0);
        let body = hygienic_helper_body(body, &[("x", "_arg")], &["x"], LIMITS, || fresh(&counter))
            .unwrap();
        assert!(matches!(&body, Node::Local { name } if name == "_arg"));
        let body = binding("_arg", local("x"), body);
        assert_eq!(
            infer_pure_type(
                &body,
                &[("x", PureType::Scalar(value.scalar_type()))],
                &NoHost,
                TYPING
            ),
            Ok(PureType::Scalar(value.scalar_type()))
        );
        assert_eq!(
            run(&body, &[("x", value.clone())], &mut Fuel::new(100)),
            value
        );
        assert_eq!(counter.get(), 0);
    }
}

#[test]
fn parsed_loop_and_fold_keep_parent_state_item_scopes_and_exact_execution_fuel() {
    let source = "loop(acc: 0, while: lt(left: acc, right: x), next: fold(total: acc, items: strings(a: \"x\", b: \"yz\"), item: \"part\", next: add(left: total, right: len(value: part)), limit: 2), limit: 2)";
    let original = binding(
        "x",
        integer(4),
        parsed(source, &[("x", ScalarType::Integer)]),
    );
    let counter = Cell::new(0);
    let body = hygienic_helper_body(
        parsed(source, &[("x", ScalarType::Integer)]),
        &[("x", "_arg")],
        &["acc", "total", "part"],
        LIMITS,
        || fresh(&counter),
    )
    .unwrap();
    let Node::Loop { name, next, .. } = &body else {
        unreachable!()
    };
    assert_eq!(name, "_h0");
    let Node::Fold {
        name,
        item,
        initial,
        next,
        ..
    } = &**next
    else {
        unreachable!()
    };
    assert_eq!(name, "_h1");
    assert_eq!(item, "_h2");
    assert!(matches!(&**initial, Node::Local { name } if name == "_h0"));
    let Node::Binary { left, right, .. } = &**next else {
        unreachable!()
    };
    assert!(matches!(&**left, Node::Local { name } if name == "_h1"));
    assert!(
        matches!(&**right, Node::Unary { value, .. } if matches!(&**value, Node::Local { name } if name == "_h2"))
    );
    let body = binding("_arg", integer(4), body);
    assert_eq!(
        infer_pure_type(&body, &[], &NoHost, TYPING),
        Ok(PureType::Scalar(ScalarType::Integer))
    );
    let mut before = Fuel::new(1000);
    let mut after = Fuel::new(1000);
    assert_eq!(run(&original, &[], &mut before), ScalarValue::Integer(6));
    assert_eq!(run(&body, &[], &mut after), ScalarValue::Integer(6));
    assert_eq!(before.remaining(), after.remaining());
    assert_eq!(counter.get(), 3);
}

#[test]
fn initializer_and_sibling_reuse_never_leak_renames_or_capture_caller_names() {
    let source = "choose(when: true, then: bind(tmp: bind(tmp: x, body: tmp), body: tmp), otherwise: bind(tmp: x, body: tmp))";
    let counter = Cell::new(0);
    let body = hygienic_helper_body(
        parsed(source, &[("x", ScalarType::Integer)]),
        &[("x", "_arg")],
        &["tmp", "x"],
        LIMITS,
        || fresh(&counter),
    )
    .unwrap();
    assert_eq!(counter.get(), 3);
    let Node::Choose {
        then, otherwise, ..
    } = &body
    else {
        unreachable!()
    };
    assert!(
        matches!(&**then, Node::Bind { name, value, body } if name == "_h1" && matches!(&**value, Node::Bind { name, .. } if name == "_h0") && matches!(&**body, Node::Local { name } if name == "_h1"))
    );
    assert!(
        matches!(&**otherwise, Node::Bind { name, body, .. } if name == "_h2" && matches!(&**body, Node::Local { name } if name == "_h2"))
    );
    let node = binding("_arg", local("x"), body);
    assert_eq!(
        run(
            &node,
            &[
                ("x", ScalarValue::Integer(7)),
                ("tmp", ScalarValue::Integer(99))
            ],
            &mut Fuel::new(100)
        ),
        ScalarValue::Integer(7)
    );
}

#[test]
fn whole_cold_free_locals_groups_zero_bodies_and_recovery_reject_before_fresh_callbacks() {
    let drops = Rc::new(RefCell::new(vec![]));
    for (body, group) in [
        (
            Node::Choose {
                when: Box::new(Node::Literal {
                    value: ScalarValue::Boolean(true),
                }),
                then: Box::new(binding("ok", integer(0), local("ok"))),
                otherwise: Box::new(local("caller")),
            },
            false,
        ),
        (
            Node::Recover {
                value: Box::new(integer(0)),
                fallback: Box::new(local("caller")),
            },
            false,
        ),
        (
            Node::Loop {
                name: "state".into(),
                initial: Box::new(integer(0)),
                condition: Box::new(Node::Literal {
                    value: ScalarValue::Boolean(false),
                }),
                next: Box::new(local("caller")),
                limit: 0,
            },
            false,
        ),
        (
            Node::Fold {
                name: "state".into(),
                item: "entry".into(),
                items: Box::new(Node::Literal {
                    value: ScalarValue::StringList(StringListValue(vec![])),
                }),
                initial: Box::new(integer(0)),
                next: Box::new(local("caller")),
                limit: 0,
            },
            false,
        ),
        (
            Node::Member {
                group: "caller".into(),
                name: "step".into(),
                operation: native("operation", &drops),
            },
            true,
        ),
        (binding("future", local("future"), local("future")), false),
    ] {
        let counter = Cell::new(0);
        assert!(
            matches!(hygienic_helper_body(body, &[], &[], LIMITS, || fresh(&counter)), Err(HelperHygieneError::Capture { group: actual }) if actual == group)
        );
        assert_eq!(counter.get(), 0);
    }
    assert_eq!(*drops.borrow(), ["operation"]);
}

#[test]
fn aliases_and_caller_reservations_have_explicit_bounds_and_do_not_grant_captures() {
    for parameters in [
        vec![("x", "_arg"), ("x", "_other")],
        vec![("x", "_arg"), ("y", "_arg")],
        vec![("x", "x")],
        vec![("body", "_arg")],
        vec![("x", "bad-name")],
    ] {
        let counter = Cell::new(0);
        assert!(
            hygienic_helper_body(integer(0), &parameters, &[], LIMITS, || fresh(&counter)).is_err()
        );
        assert_eq!(counter.get(), 0);
    }
    for (reserved, limits) in [
        (vec!["_arg"], LIMITS),
        (vec!["body"], LIMITS),
        (
            vec!["caller"],
            HelperHygieneLimits {
                max_reserved_names: 0,
                ..LIMITS
            },
        ),
    ] {
        let counter = Cell::new(0);
        assert!(
            hygienic_helper_body(local("x"), &[("x", "_arg")], &reserved, limits, || fresh(
                &counter
            ))
            .is_err()
        );
        assert_eq!(counter.get(), 0);
    }
    let counter = Cell::new(0);
    assert!(matches!(
        hygienic_helper_body(local("caller"), &[], &["caller"], LIMITS, || fresh(
            &counter
        )),
        Err(HelperHygieneError::Capture { group: false })
    ));
    assert_eq!(counter.get(), 0);
}

#[test]
fn active_quota_counts_parameters_and_both_fold_locals_without_counting_siblings() {
    let body = parsed(
        "choose(when: true, then: bind(a: x, body: a), otherwise: bind(a: x, body: a))",
        &[("x", ScalarType::Integer)],
    );
    let counter = Cell::new(0);
    assert!(
        hygienic_helper_body(
            body,
            &[("x", "_arg")],
            &[],
            HelperHygieneLimits {
                max_bindings: 2,
                ..LIMITS
            },
            || fresh(&counter)
        )
        .is_ok()
    );
    assert_eq!(counter.get(), 2);
    let body = parsed(
        "fold(acc: x, items: strings(), item: \"entry\", next: acc, limit: 0)",
        &[("x", ScalarType::Integer)],
    );
    let counter = Cell::new(0);
    assert!(matches!(
        hygienic_helper_body(
            body,
            &[("x", "_arg")],
            &[],
            HelperHygieneLimits {
                max_bindings: 2,
                ..LIMITS
            },
            || fresh(&counter)
        ),
        Err(HelperHygieneError::BindingLimit)
    ));
    assert_eq!(counter.get(), 0);
    assert!(matches!(
        hygienic_helper_body(
            integer(0),
            &[("x", "_arg")],
            &[],
            HelperHygieneLimits {
                max_bindings: 0,
                ..LIMITS
            },
            || fresh(&counter)
        ),
        Err(HelperHygieneError::BindingLimit)
    ));
}

#[test]
fn active_shadow_and_invalid_cold_language_names_are_not_laundered_by_renaming() {
    let drops = Rc::new(RefCell::new(vec![]));
    for body in [
        binding("x", integer(0), local("x")),
        binding("bad-name", integer(0), integer(0)),
        Node::Member {
            group: "x".into(),
            name: "bad.name".into(),
            operation: native("operation", &drops),
        },
        Node::Loop {
            name: "while".into(),
            initial: Box::new(integer(0)),
            condition: Box::new(integer(0)),
            next: Box::new(integer(0)),
            limit: 0,
        },
        Node::Fold {
            name: "same".into(),
            item: "same".into(),
            items: Box::new(integer(0)),
            initial: Box::new(integer(0)),
            next: Box::new(integer(0)),
            limit: 0,
        },
    ] {
        let counter = Cell::new(0);
        assert!(
            hygienic_helper_body(body, &[("x", "_arg")], &[], LIMITS, || fresh(&counter)).is_err()
        );
        assert_eq!(counter.get(), 0);
    }
    assert_eq!(*drops.borrow(), ["operation"]);
}

#[test]
fn whole_physical_bounds_and_invalid_policy_precede_all_native_name_allocation() {
    for limits in [
        HelperHygieneLimits {
            max_nodes: 2,
            ..LIMITS
        },
        HelperHygieneLimits {
            max_depth: 0,
            ..LIMITS
        },
        HelperHygieneLimits {
            max_nodes: 16_385,
            ..LIMITS
        },
        HelperHygieneLimits {
            max_depth: 65,
            ..LIMITS
        },
        HelperHygieneLimits {
            max_bindings: 1_025,
            ..LIMITS
        },
        HelperHygieneLimits {
            max_reserved_names: 16_385,
            ..LIMITS
        },
    ] {
        let counter = Cell::new(0);
        assert!(
            hygienic_helper_body(
                binding("x", integer(0), local("x")),
                &[],
                &[],
                limits,
                || fresh(&counter)
            )
            .is_err()
        );
        assert_eq!(counter.get(), 0);
    }
    let counter = Cell::new(0);
    assert!(
        hygienic_helper_body(
            integer(0),
            &[],
            &[],
            HelperHygieneLimits {
                max_nodes: 1,
                max_depth: 0,
                max_bindings: 0,
                max_reserved_names: 0
            },
            || fresh(&counter)
        )
        .is_ok()
    );
    assert!(matches!(
        hygienic_helper_body(
            integer(0),
            &[],
            &[],
            HelperHygieneLimits {
                max_nodes: 0,
                ..LIMITS
            },
            || fresh(&counter)
        ),
        Err(HelperHygieneError::Structure(StructureError::NodeLimit))
    ));
    assert_eq!(counter.get(), 0);
}

#[test]
fn generated_names_cannot_collide_with_original_alias_reserved_or_previous_names() {
    for candidate in ["x", "_arg", "caller", "body", "bad-name"] {
        let counter = Cell::new(0);
        assert!(matches!(
            hygienic_helper_body(
                binding("x", local("p"), local("x")),
                &[("p", "_arg")],
                &["caller"],
                LIMITS,
                || -> Result<_, PrivateError> {
                    counter.set(counter.get() + 1);
                    Ok(candidate.into())
                }
            ),
            Err(HelperHygieneError::FreshName)
        ));
        assert_eq!(counter.get(), 1);
    }
    let counter = Cell::new(0);
    assert!(matches!(
        hygienic_helper_body(
            binding("a", integer(0), binding("b", integer(1), local("b"))),
            &[],
            &[],
            LIMITS,
            || -> Result<_, PrivateError> {
                counter.set(counter.get() + 1);
                Ok("_same".into())
            }
        ),
        Err(HelperHygieneError::FreshName)
    ));
    assert_eq!(counter.get(), 2);
}

#[test]
fn unused_quoted_fold_item_and_cold_local_labels_are_reserved_before_fresh_names() {
    let body = parsed(
        "choose(when: true, then: bind(temp: 0, body: temp), otherwise: fold(acc: 0, items: strings(), item: \"_h0\", next: acc, limit: 0))",
        &[],
    );
    let counter = Cell::new(0);
    assert!(matches!(
        hygienic_helper_body(body, &[], &[], LIMITS, || fresh(&counter)),
        Err(HelperHygieneError::FreshName)
    ));
    assert_eq!(counter.get(), 1);
}

fn native_body(drops: &Rc<RefCell<Vec<&'static str>>>) -> Node {
    binding(
        "captured",
        Node::Group {
            group_kind: GroupKind::Parallel,
            branches: vec![
                ComputedBranch {
                    name: "first-step".into(),
                    value: Node::Call {
                        operation: native("call", drops),
                        arguments: vec![ComputedArgument {
                            name: "payload".into(),
                            value: local("p"),
                        }],
                    },
                    result_type: native("first-result", drops),
                },
                ComputedBranch {
                    name: "second-step".into(),
                    value: Node::Host {
                        effect: Box::new(native("host", drops)),
                    },
                    result_type: native("second-result", drops),
                },
            ],
        },
        Node::Field {
            value: Box::new(Node::Member {
                group: "captured".into(),
                name: "first-step".into(),
                operation: native("member", drops),
            }),
            field: native("field", drops),
        },
    )
}

#[test]
fn all_four_native_slots_boxes_vectors_and_member_labels_move_without_cloning() {
    let drops = Rc::new(RefCell::new(vec![]));
    let body = native_body(&drops);
    let Node::Bind {
        value,
        body: projection,
        ..
    } = &body
    else {
        unreachable!()
    };
    let value_pointer = &**value as *const Node;
    let Node::Group { branches, .. } = &**value else {
        unreachable!()
    };
    let branches_pointer = branches.as_ptr();
    let first_buffer = branches[0].result_type.buffer.as_ptr();
    let Node::Field { field, .. } = &**projection else {
        unreachable!()
    };
    let field_buffer = field.buffer.as_ptr();
    let counter = Cell::new(0);
    let body =
        hygienic_helper_body(body, &[("p", "_arg")], &[], LIMITS, || fresh(&counter)).unwrap();
    assert!(drops.borrow().is_empty());
    let Node::Bind {
        name,
        value,
        body: projection,
    } = &body
    else {
        unreachable!()
    };
    assert_eq!(name, "_h0");
    assert!(std::ptr::eq(&**value, value_pointer));
    let Node::Group { branches, .. } = &**value else {
        unreachable!()
    };
    assert_eq!(branches.as_ptr(), branches_pointer);
    assert_eq!(branches[0].result_type.buffer.as_ptr(), first_buffer);
    assert_eq!(branches[0].name, "first-step");
    assert_eq!(branches[1].name, "second-step");
    assert!(
        matches!(&branches[0].value, Node::Call { arguments, operation } if operation.id == "call" && arguments[0].name == "payload" && matches!(&arguments[0].value, Node::Local { name } if name == "_arg"))
    );
    let Node::Field { value, field } = &**projection else {
        unreachable!()
    };
    assert_eq!(field.buffer.as_ptr(), field_buffer);
    assert!(
        matches!(&**value, Node::Member { group, name, operation } if group == "_h0" && name == "first-step" && operation.id == "member")
    );
    drop(body);
    let mut actual = drops.borrow().clone();
    actual.sort();
    assert_eq!(
        actual,
        [
            "call",
            "field",
            "first-result",
            "host",
            "member",
            "second-result"
        ]
    );
}

#[test]
fn native_allocator_error_and_unwind_drop_consumed_body_once_without_returning_partial_ir() {
    let drops = Rc::new(RefCell::new(vec![]));
    let counter = Cell::new(0);
    let Err(error) = hygienic_helper_body(
        native_body(&drops),
        &[("p", "_arg")],
        &[],
        LIMITS,
        || -> Result<_, PrivateError> {
            counter.set(counter.get() + 1);
            Err(PrivateError("private name allocator"))
        },
    ) else {
        panic!("expected native error")
    };
    assert!(!format!("{error} {error:?}").contains("private"));
    assert!(matches!(
        error,
        HelperHygieneError::Native(PrivateError("private name allocator"))
    ));
    assert_eq!(counter.get(), 1);
    assert_eq!(drops.borrow().len(), 6);
    assert!(
        catch_unwind(AssertUnwindSafe(|| hygienic_helper_body(
            native_body(&drops),
            &[("p", "_arg")],
            &[],
            LIMITS,
            || -> Result<_, PrivateError> {
                counter.set(counter.get() + 1);
                panic!("private allocator unwind")
            }
        )))
        .is_err()
    );
    assert_eq!(counter.get(), 2);
    assert_eq!(drops.borrow().len(), 12);
    assert!(
        hygienic_helper_body(
            native_body(&drops),
            &[("p", "_arg")],
            &[],
            LIMITS,
            || fresh(&counter)
        )
        .is_ok()
    );
    assert_eq!(drops.borrow().len(), 18);
}

#[test]
fn failure_after_one_successful_binding_keeps_external_reservations_consumed() {
    let drops = Rc::new(RefCell::new(vec![]));
    let body = binding(
        "outer",
        Node::Host {
            effect: Box::new(native("host", &drops)),
        },
        native_body(&drops),
    );
    let counter = Cell::new(0);
    let result = hygienic_helper_body(
        body,
        &[("p", "_arg")],
        &[],
        LIMITS,
        || -> Result<_, PrivateError> {
            let count = counter.get();
            counter.set(count + 1);
            if count == 0 {
                Ok("_first".into())
            } else {
                Err(PrivateError("private later failure"))
            }
        },
    );
    assert!(matches!(result, Err(HelperHygieneError::Native(_))));
    assert_eq!(counter.get(), 2);
    assert_eq!(drops.borrow().len(), 7);
}

#[test]
fn hygiene_does_not_replace_cold_type_checking_or_inspect_opaque_host_payloads() {
    let body = Node::Binary {
        operator: BinaryOperator::Add,
        left: Box::new(local("p")),
        right: Box::new(Node::Literal {
            value: ScalarValue::Boolean(true),
        }),
    };
    let body = hygienic_helper_body(
        body,
        &[("p", "_arg")],
        &[],
        LIMITS,
        || -> Result<_, PrivateError> { panic!() },
    )
    .unwrap();
    assert_eq!(
        infer_pure_type(
            &body,
            &[("_arg", PureType::Scalar(ScalarType::Integer))],
            &NoHost,
            TYPING
        ),
        Err(PureTypeError::BinaryOperands)
    );
    let drops = Rc::new(RefCell::new(vec![]));
    let node = Node::Host {
        effect: Box::new(native("host", &drops)),
    };
    let node = hygienic_helper_body(
        node,
        &[],
        &[],
        HelperHygieneLimits {
            max_nodes: 1,
            max_depth: 0,
            max_bindings: 0,
            max_reserved_names: 0,
        },
        || -> Result<_, PrivateError> { panic!() },
    )
    .unwrap();
    assert!(matches!(
        infer_pure_type(&node, &[], &NoHost, TYPING),
        Err(PureTypeError::Impure)
    ));
    drop(node);
    assert_eq!(*drops.borrow(), ["host"]);
}

#[test]
fn call_group_and_string_forests_do_not_export_sibling_temporary_bindings() {
    let drops = Rc::new(RefCell::new(vec![]));
    for body in [
        Node::Strings {
            items: vec![binding("temp", integer(0), local("temp")), local("temp")],
        },
        Node::Call {
            operation: native("call", &drops),
            arguments: vec![
                ComputedArgument {
                    name: "first".into(),
                    value: binding("temp", integer(0), local("temp")),
                },
                ComputedArgument {
                    name: "second".into(),
                    value: local("temp"),
                },
            ],
        },
        Node::Group {
            group_kind: GroupKind::Sequence,
            branches: vec![
                ComputedBranch {
                    name: "first".into(),
                    value: binding("temp", integer(0), local("temp")),
                    result_type: native("first-result", &drops),
                },
                ComputedBranch {
                    name: "second".into(),
                    value: local("temp"),
                    result_type: native("second-result", &drops),
                },
            ],
        },
    ] {
        let counter = Cell::new(0);
        assert!(matches!(
            hygienic_helper_body(body, &[], &[], LIMITS, || fresh(&counter)),
            Err(HelperHygieneError::Capture { group: false })
        ));
        assert_eq!(counter.get(), 0);
    }
    assert_eq!(drops.borrow().len(), 3);
}

#[test]
fn reference_helper_names_wire_authority_and_child_error_order_remain_compatible() {
    for source in [
        "fn id(n: integer) = n\nfn main() = id(n: 7)",
        "fn work(n: integer) = bind(tmp: n, body: tmp)\nfn main() = bind(_lf0: 9, body: work(n: _lf0))",
        "fn read() = bind(r: runtime.list(), body: field(value: r, name: \"count\"))\nfn main() = read()",
        "fn gather() = bind(g: all(first: runtime.list(), second: runtime.list()), body: field(value: member(value: g, name: \"first\"), name: \"count\"))\nfn main() = gather()",
    ] {
        let program = leselang_hir::lower(&parse(source)).unwrap();
        let canonical = leselang_hir::canonical_source(&program.function.effect).unwrap();
        assert_eq!(
            serde_json::to_vec(&leselang_hir::lower(&parse(&canonical)).unwrap()).unwrap(),
            serde_json::to_vec(&program).unwrap()
        );
        if source.contains("runtime.list") {
            assert_eq!(program.function.required_capabilities, ["runtime.read"]);
        }
    }
    let program =
        leselang_hir::lower(&parse("fn id(n: integer) = n\nfn main() = id(n: 7)")).unwrap();
    let leselang_hir::Effect::Compute { expression } = program.function.effect else {
        unreachable!()
    };
    assert!(
        matches!(&*expression, Computation::Bind { name, body, .. } if name == "_lf0" && matches!(&**body, Computation::Local { name } if name == "_lf0"))
    );
    let program = leselang_hir::lower(&parse("fn work(n: integer) = bind(tmp: n, body: tmp)\nfn main() = bind(_lf0: 9, body: work(n: _lf0))")).unwrap();
    let canonical = leselang_hir::canonical_source(&program.function.effect).unwrap();
    assert!(canonical.contains("_lf1"));
    assert!(canonical.contains("_lf2"));
    for (source, code) in [
        ("fn f(n: integer) = n\nfn main() = f(n: true)", "LSH1504"),
        (
            "fn f() = caller\nfn main() = bind(caller: 0, body: f())",
            "LSH1403",
        ),
        (
            "fn f() = choose(when: true, then: 0, otherwise: f())\nfn main() = 0",
            "LSH1502",
        ),
    ] {
        assert_eq!(
            leselang_hir::lower(&parse(source)).unwrap_err()[0].code,
            code
        );
    }
}
