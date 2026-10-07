use std::cell::{Cell, RefCell};
use std::convert::Infallible;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;

use leselang_hir::helper_returns::*;
use leselang_hir::ir::{Computation, ComputedArgument, ComputedBranch, GroupKind};
use leselang_hir::pure_evaluation::*;
use leselang_hir::pure_typing::*;
use leselang_hir::source_cost::{SourceCostLimits, measure_source_cost};
use leselang_runtime_core::*;

type Data = Computation<u8, u32, (), ()>;
const LIMITS: HelperReturnLimits = HelperReturnLimits {
    max_nodes: 128,
    max_depth: 16,
};
fn integer(value: u64) -> Data {
    Data::Literal {
        value: ScalarValue::Integer(value),
    }
}
fn local(name: &str) -> Data {
    Data::Local { name: name.into() }
}
fn call(operation: u32) -> Data {
    Data::Call {
        operation,
        arguments: vec![],
    }
}
fn bind(name: &str, value: Data, body: Data) -> Data {
    Data::Bind {
        name: name.into(),
        value: Box::new(value),
        body: Box::new(body),
    }
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
fn plan(value: Data, body: Data) -> HelperReturns<Data> {
    HelperReturns::new("answer".into(), value, body, LIMITS).unwrap()
}
fn copied(plan: HelperReturns<Data>) -> Data {
    plan.connect(|body, _| Ok::<_, Infallible>(body.clone()))
        .unwrap()
}
fn denied<Node, Error>(result: Result<Node, HelperReturnError<Error>>) -> HelperReturnError<Error> {
    match result {
        Err(error) => error,
        Ok(_) => panic!("expected helper return denial"),
    }
}

#[test]
fn owned_leaf_admits_exact_wrapper_shape_without_materialization() {
    let name = "answer".to_owned();
    let pointer = name.as_ptr();
    let plan = HelperReturns::new(
        name,
        integer(41),
        local("answer"),
        HelperReturnLimits {
            max_nodes: 3,
            max_depth: 1,
        },
    )
    .unwrap();
    assert_eq!(
        plan.shape(),
        HelperReturnShape {
            nodes: 3,
            depth: 1,
            returns: 1
        }
    );
    assert_eq!(plan.name().as_ptr(), pointer);
    assert!(matches!(plan.value(), Data::Literal { .. }));
    assert!(matches!(plan.continuation(), Data::Local { .. }));
    assert_eq!(plan.return_sites().count(), 1);
    let result = copied(plan);
    assert_eq!(result, bind("answer", integer(41), local("answer")));
}

#[test]
fn whole_pure_control_subtrees_are_single_returns_not_rewritten_inside() {
    let values = [
        bind("x", integer(1), local("x")),
        choose(integer(1), integer(2)),
        Data::Recover {
            value: Box::new(integer(1)),
            fallback: Box::new(integer(2)),
        },
        Data::Loop {
            name: "x".into(),
            initial: Box::new(integer(0)),
            condition: Box::new(Data::Literal {
                value: ScalarValue::Boolean(false),
            }),
            next: Box::new(local("x")),
            limit: 0,
        },
        Data::Fold {
            name: "x".into(),
            item: "part".into(),
            items: Box::new(Data::Strings { items: vec![] }),
            initial: Box::new(integer(0)),
            next: Box::new(local("x")),
            limit: 0,
        },
        Data::Strings {
            items: vec![integer(1)],
        },
        Data::Field {
            value: Box::new(local("external")),
            field: 0,
        },
        Data::Member {
            group: "external".into(),
            name: "member".into(),
            operation: 7,
        },
    ];
    for value in values {
        let expected = value.clone();
        let plan = plan(value, local("answer"));
        assert_eq!(plan.shape().returns, 1);
        assert_eq!(plan.return_sites().next().unwrap().1, 0);
        assert_eq!(copied(plan), bind("answer", expected, local("answer")));
    }
}

#[test]
fn every_cold_normal_return_is_admitted_before_left_to_right_factories() {
    let value = choose(
        bind("inner", call(1), integer(10)),
        choose(call(2), call(3)),
    );
    let plan = plan(value, local("answer"));
    assert_eq!(plan.shape().returns, 3);
    let sites = plan
        .return_sites()
        .map(|(node, depth)| match node {
            Data::Call { operation, .. } => (*operation, depth),
            Data::Literal {
                value: ScalarValue::Integer(value),
            } => (*value as u32, depth),
            _ => panic!(),
        })
        .collect::<Vec<_>>();
    assert_eq!(sites, [(3, 2), (2, 2), (10, 2)]);
    let Data::Local { name } = plan.continuation() else {
        panic!()
    };
    let original = name.as_ptr();
    let mut factories = vec![];
    let result = plan
        .connect(|body, index| {
            let Data::Local { name } = body else { panic!() };
            assert_eq!(name.as_ptr(), original);
            factories.push(index);
            Ok::<_, ()>(body.clone())
        })
        .unwrap();
    assert_eq!(factories, [0, 1, 2]);
    assert_eq!(
        result,
        choose(
            bind(
                "inner",
                call(1),
                bind("answer", integer(10), local("answer"))
            ),
            choose(
                bind("answer", call(2), local("answer")),
                bind("answer", call(3), local("answer"))
            )
        )
    );
}

#[test]
fn expanded_depth_keeps_initializers_and_guards_at_original_depth() {
    let initializer = bind("a", integer(0), bind("b", integer(0), integer(1)));
    let value = bind("inner", initializer, call(7));
    let plan = HelperReturns::new(
        "answer".into(),
        value,
        local("answer"),
        HelperReturnLimits {
            max_nodes: 9,
            max_depth: 3,
        },
    )
    .unwrap();
    assert_eq!(
        plan.shape(),
        HelperReturnShape {
            nodes: 9,
            depth: 3,
            returns: 1
        }
    );
    let result = copied(plan);
    let Data::Bind { value, .. } = result else {
        panic!()
    };
    assert_eq!(
        *value,
        bind("a", integer(0), bind("b", integer(0), integer(1)))
    );
}

#[test]
fn effectful_initializers_are_not_return_paths_or_implicit_type_certificates() {
    let value = bind(
        "inner",
        Data::Recover {
            value: Box::new(call(1)),
            fallback: Box::new(integer(0)),
        },
        call(2),
    );
    let plan = plan(value, local("answer"));
    assert_eq!(plan.shape().returns, 1);
    let Data::Bind { value, body, .. } = copied(plan) else {
        panic!()
    };
    assert!(matches!(*value, Data::Recover { .. }));
    assert!(matches!(*body, Data::Bind { .. }));
}

#[test]
fn unsupported_cold_recover_loop_fold_and_operand_effect_boundaries_are_rejected() {
    for value in [
        Data::Recover {
            value: Box::new(call(1)),
            fallback: Box::new(integer(0)),
        },
        Data::Loop {
            name: "n".into(),
            initial: Box::new(integer(0)),
            condition: Box::new(integer(0)),
            next: Box::new(call(1)),
            limit: 0,
        },
        Data::Fold {
            name: "n".into(),
            item: "part".into(),
            items: Box::new(Data::Strings { items: vec![] }),
            initial: Box::new(integer(0)),
            next: Box::new(call(1)),
            limit: 0,
        },
        Data::Binary {
            operator: BinaryOperator::Add,
            left: Box::new(integer(0)),
            right: Box::new(call(1)),
        },
        Data::Unary {
            operator: UnaryOperator::Not,
            value: Box::new(call(1)),
        },
        Data::Field {
            value: Box::new(call(1)),
            field: 0,
        },
        Data::Strings {
            items: vec![call(1)],
        },
    ] {
        let error = HelperReturns::new(
            "answer".into(),
            choose(integer(0), value),
            local("answer"),
            LIMITS,
        )
        .unwrap_err();
        assert!(matches!(error, HelperReturnError::UnsupportedBoundary));
    }
}

#[test]
fn independent_inclusive_output_node_and_depth_limits_are_checked() {
    for limits in [
        HelperReturnLimits {
            max_nodes: 2,
            max_depth: 1,
        },
        HelperReturnLimits {
            max_nodes: 3,
            max_depth: 0,
        },
    ] {
        let error =
            HelperReturns::new("answer".into(), integer(0), local("answer"), limits).unwrap_err();
        assert!(matches!(error, HelperReturnError::Output(_)));
    }
    let plan = plan(choose(call(1), call(2)), local("answer"));
    assert_eq!(
        plan.shape(),
        HelperReturnShape {
            nodes: 8,
            depth: 2,
            returns: 2
        }
    );
    let limits = HelperReturnLimits {
        max_nodes: 8,
        max_depth: 2,
    };
    assert!(
        HelperReturns::new(
            "answer".into(),
            choose(call(1), call(2)),
            local("answer"),
            limits
        )
        .is_ok()
    );
    assert!(matches!(
        HelperReturns::new(
            "answer".into(),
            choose(call(1), call(2)),
            local("answer"),
            HelperReturnLimits {
                max_nodes: 7,
                ..limits
            }
        ),
        Err(HelperReturnError::Output(StructureError::NodeLimit))
    ));
}

#[test]
fn fixed_safety_ceilings_and_zero_policies_fail_without_factory_work() {
    for limits in [
        HelperReturnLimits {
            max_nodes: 16_385,
            max_depth: 64,
        },
        HelperReturnLimits {
            max_nodes: 16_384,
            max_depth: 65,
        },
        HelperReturnLimits {
            max_nodes: usize::MAX,
            max_depth: usize::MAX,
        },
    ] {
        assert!(matches!(
            HelperReturns::new("answer".into(), integer(0), integer(0), limits),
            Err(HelperReturnError::InvalidLimits)
        ));
    }
    assert!(matches!(
        HelperReturns::new(
            "answer".into(),
            integer(0),
            integer(0),
            HelperReturnLimits {
                max_nodes: 0,
                max_depth: 0
            }
        ),
        Err(HelperReturnError::Input {
            continuation: false,
            error: StructureError::NodeLimit
        })
    ));
}

#[test]
fn complete_cold_input_frontiers_are_bounded_before_return_analysis() {
    let wide = || Data::Strings {
        items: (0..129).map(integer).collect(),
    };
    assert!(matches!(
        HelperReturns::new(
            "answer".into(),
            choose(call(1), wide()),
            local("answer"),
            LIMITS
        ),
        Err(HelperReturnError::Input {
            continuation: false,
            error: StructureError::NodeLimit
        })
    ));
    assert!(matches!(
        HelperReturns::new("answer".into(), call(1), wide(), LIMITS),
        Err(HelperReturnError::Input {
            continuation: true,
            error: StructureError::NodeLimit
        })
    ));
    let mut deep = call(1);
    for i in 0..17 {
        deep = bind(&format!("v{i}"), integer(0), deep);
    }
    assert!(matches!(
        HelperReturns::new("answer".into(), deep, local("answer"), LIMITS),
        Err(HelperReturnError::Input {
            continuation: false,
            error: StructureError::DepthLimit
        })
    ));
}

#[test]
fn all_cold_return_copies_are_reserved_not_only_the_selected_branch() {
    let mut value = call(1);
    for _ in 0..5 {
        value = choose(value.clone(), value);
    }
    assert!(matches!(
        HelperReturns::new("answer".into(), value, local("answer"), LIMITS),
        Err(HelperReturnError::Output(StructureError::NodeLimit))
    ));
}

#[test]
fn invalid_aliases_and_exposed_value_alias_binders_are_rejected() {
    for name in ["", "bad name", "1bad", "a.b"] {
        assert!(matches!(
            HelperReturns::new(name.into(), integer(0), integer(0), LIMITS),
            Err(HelperReturnError::InvalidName)
        ));
    }
    for value in [
        bind("answer", call(1), integer(0)),
        bind("answer", integer(0), call(1)),
    ] {
        assert!(matches!(
            HelperReturns::new(
                "answer".into(),
                choose(call(1), value),
                local("answer"),
                LIMITS
            ),
            Err(HelperReturnError::Capture)
        ));
    }
}

#[test]
fn pure_terminal_and_initializer_binders_do_not_capture_the_continuation() {
    for value in [
        bind("answer", integer(0), local("answer")),
        choose(call(1), bind("private", integer(0), local("private"))),
        bind(
            "inner",
            bind("private", integer(0), local("private")),
            call(1),
        ),
    ] {
        assert!(
            HelperReturns::new(
                "answer".into(),
                value,
                bind("private", integer(0), local("answer")),
                LIMITS
            )
            .is_ok()
        );
    }
    assert!(HelperReturns::new("answer".into(), local("answer"), local("answer"), LIMITS).is_ok());
}

#[test]
fn cold_lexical_names_are_bounded_before_route_name_copy_or_factory_capture_hashing() {
    let long = "x".repeat(100_000);
    for value in [
        bind(&long, call(1), integer(0)),
        local("bad name"),
        Data::Member {
            group: long.clone(),
            name: "native-export".into(),
            operation: 1,
        },
    ] {
        assert!(matches!(
            HelperReturns::new(
                "answer".into(),
                choose(call(1), value),
                local("answer"),
                LIMITS
            ),
            Err(HelperReturnError::InvalidLexicalName)
        ));
    }
    assert!(matches!(
        HelperReturns::new("answer".into(), call(1), local(&long), LIMITS),
        Err(HelperReturnError::InvalidLexicalName)
    ));
    let calls = Cell::new(0);
    let error = denied(
        plan(choose(call(1), call(2)), local("answer")).connect(|_, _| {
            calls.set(calls.get() + 1);
            Ok::<_, ()>(local(&long))
        }),
    );
    assert!(matches!(error, HelperReturnError::InvalidLexicalName));
    assert_eq!(calls.get(), 1);
}

#[test]
fn continuation_cold_binders_cannot_shadow_the_return_alias() {
    for body in [
        bind("answer", integer(0), integer(0)),
        Data::Loop {
            name: "answer".into(),
            initial: Box::new(integer(0)),
            condition: Box::new(integer(0)),
            next: Box::new(integer(0)),
            limit: 0,
        },
        Data::Fold {
            name: "n".into(),
            item: "answer".into(),
            items: Box::new(Data::Strings { items: vec![] }),
            initial: Box::new(integer(0)),
            next: Box::new(integer(0)),
            limit: 0,
        },
    ] {
        assert!(matches!(
            HelperReturns::new("answer".into(), call(1), choose(integer(0), body), LIMITS),
            Err(HelperReturnError::Capture)
        ));
    }
}

#[test]
fn value_binders_cannot_capture_caller_locals_groups_or_unused_cold_names() {
    let value = || choose(bind("private", call(1), integer(0)), call(2));
    for body in [
        local("private"),
        Data::Member {
            group: "private".into(),
            name: "item".into(),
            operation: 1,
        },
        bind("private", integer(0), integer(1)),
    ] {
        assert!(matches!(
            HelperReturns::new("answer".into(), value(), choose(integer(0), body), LIMITS),
            Err(HelperReturnError::Capture)
        ));
    }
    assert!(HelperReturns::new("answer".into(), value(), local("external"), LIMITS).is_ok());
}

#[test]
fn factory_shape_change_stops_before_the_next_cold_return() {
    let calls = Cell::new(0);
    let error = denied(
        plan(choose(call(1), call(2)), local("answer")).connect(|_, _| {
            calls.set(calls.get() + 1);
            Ok::<_, ()>(bind("x", integer(0), local("answer")))
        }),
    );
    assert_eq!(calls.get(), 1);
    assert!(matches!(
        error,
        HelperReturnError::ChangedContinuation { index: 0 }
    ));
}

#[test]
fn equal_node_count_but_changed_depth_is_not_a_reserved_continuation() {
    let original = choose(integer(0), integer(1));
    let replacement = Data::Unary {
        operator: UnaryOperator::Not,
        value: Box::new(Data::Unary {
            operator: UnaryOperator::Not,
            value: Box::new(Data::Unary {
                operator: UnaryOperator::Not,
                value: Box::new(integer(0)),
            }),
        }),
    };
    let error = denied(plan(call(1), original).connect(|_, _| Ok::<_, ()>(replacement.clone())));
    assert!(matches!(
        error,
        HelperReturnError::ChangedContinuation { index: 0 }
    ));
}

#[test]
fn factory_outputs_are_physically_bounded_before_name_scans() {
    let error = denied(plan(call(1), local("answer")).connect(|_, _| {
        Ok::<_, ()>(Data::Strings {
            items: (0..129).map(integer).collect(),
        })
    }));
    assert!(matches!(
        error,
        HelperReturnError::Input {
            continuation: true,
            error: StructureError::NodeLimit
        }
    ));
}

#[test]
fn same_shape_factory_output_is_still_checked_for_new_capture() {
    let value = bind("private", call(1), integer(0));
    let error = denied(plan(value, local("answer")).connect(|_, _| Ok::<_, ()>(local("private"))));
    assert!(matches!(error, HelperReturnError::Capture));
}

struct Native {
    id: &'static str,
    bytes: Vec<u8>,
    drops: Rc<RefCell<Vec<&'static str>>>,
}
impl Drop for Native {
    fn drop(&mut self) {
        self.drops.borrow_mut().push(self.id);
    }
}
struct PrivateError(&'static str);
type Owned = Computation<Native, Native, Native, Native>;
fn native(id: &'static str, drops: &Rc<RefCell<Vec<&'static str>>>) -> Native {
    Native {
        id,
        bytes: vec![7; 31],
        drops: drops.clone(),
    }
}
fn host(id: &'static str, drops: &Rc<RefCell<Vec<&'static str>>>) -> Owned {
    Owned::Host {
        effect: Box::new(native(id, drops)),
    }
}
fn owned_choose(drops: &Rc<RefCell<Vec<&'static str>>>) -> Owned {
    Owned::Choose {
        when: Box::new(Owned::Literal {
            value: ScalarValue::Boolean(true),
        }),
        then: Box::new(host("left", drops)),
        otherwise: Box::new(host("right", drops)),
    }
}

#[test]
fn move_only_all_four_native_slots_boxes_vectors_and_buffers_move_unchanged() {
    let drops = Rc::new(RefCell::new(vec![]));
    let effect = Box::new(native("effect", &drops));
    let effect_ptr = effect.as_ref() as *const Native;
    let argument = Owned::Field {
        value: Box::new(Owned::Literal {
            value: ScalarValue::Integer(1),
        }),
        field: native("field", &drops),
    };
    let field_ptr = match &argument {
        Owned::Field { field, .. } => field.bytes.as_ptr(),
        _ => panic!(),
    };
    let arguments = vec![ComputedArgument {
        name: "arg".into(),
        value: argument,
    }];
    let argument_ptr = arguments.as_ptr();
    let branches = vec![ComputedBranch {
        name: "work".into(),
        value: Owned::Call {
            operation: native("operation", &drops),
            arguments,
        },
        result_type: native("result", &drops),
    }];
    let branches_ptr = branches.as_ptr();
    let when = Box::new(Owned::Literal {
        value: ScalarValue::Boolean(true),
    });
    let guard_ptr = when.as_ref() as *const Owned;
    let value = Owned::Choose {
        when,
        then: Box::new(Owned::Host { effect }),
        otherwise: Box::new(Owned::Group {
            group_kind: GroupKind::Sequence,
            branches,
        }),
    };
    let continuation = Owned::Local {
        name: "answer".into(),
    };
    let plan = HelperReturns::new("answer".into(), value, continuation, LIMITS).unwrap();
    assert_eq!(plan.shape().returns, 2);
    let result = plan
        .connect(|_, _| {
            Ok::<_, PrivateError>(Owned::Local {
                name: "answer".into(),
            })
        })
        .unwrap();
    assert!(drops.borrow().is_empty());
    let Owned::Choose {
        when,
        then,
        otherwise,
    } = &result
    else {
        panic!()
    };
    assert_eq!(when.as_ref() as *const Owned, guard_ptr);
    let Owned::Bind { value, .. } = then.as_ref() else {
        panic!()
    };
    let Owned::Host { effect } = value.as_ref() else {
        panic!()
    };
    assert_eq!(effect.as_ref() as *const Native, effect_ptr);
    let Owned::Bind { value, .. } = otherwise.as_ref() else {
        panic!()
    };
    let Owned::Group { branches, .. } = value.as_ref() else {
        panic!()
    };
    assert_eq!(branches.as_ptr(), branches_ptr);
    let Owned::Call { arguments, .. } = &branches[0].value else {
        panic!()
    };
    assert_eq!(arguments.as_ptr(), argument_ptr);
    let Owned::Field { field, .. } = &arguments[0].value else {
        panic!()
    };
    assert_eq!(field.bytes.as_ptr(), field_ptr);
    drop(result);
    assert_eq!(drops.borrow().len(), 4);
}

#[test]
fn factory_error_moves_private_error_and_drops_original_and_partial_native_values_once() {
    let drops = Rc::new(RefCell::new(vec![]));
    let plan = HelperReturns::new(
        "answer".into(),
        owned_choose(&drops),
        host("template", &drops),
        LIMITS,
    )
    .unwrap();
    let calls = Cell::new(0);
    let error = denied(plan.connect(|_, index| {
        calls.set(calls.get() + 1);
        if index == 1 {
            Err(PrivateError("private factory payload"))
        } else {
            Ok(host("copy", &drops))
        }
    }));
    assert_eq!(calls.get(), 2);
    assert!(!format!("{error:?} {error}").contains("private factory payload"));
    assert!(std::error::Error::source(&error).is_none());
    let HelperReturnError::Factory { index, error } = error else {
        panic!()
    };
    assert_eq!(index, 1);
    assert_eq!(error.0, "private factory payload");
    let mut events = drops.borrow().clone();
    events.sort();
    assert_eq!(events, ["copy", "left", "right", "template"]);
}

#[test]
fn factory_unwind_drops_partial_owned_inputs_without_retry_or_reservation_refund() {
    let drops = Rc::new(RefCell::new(vec![]));
    let plan = HelperReturns::new(
        "answer".into(),
        owned_choose(&drops),
        host("template", &drops),
        LIMITS,
    )
    .unwrap();
    let calls = Cell::new(0);
    let reserved_nodes = Cell::new(plan.shape().nodes);
    let result = catch_unwind(AssertUnwindSafe(|| {
        let _ = plan.connect(|_, index| -> Result<Owned, PrivateError> {
            calls.set(calls.get() + 1);
            if index == 1 {
                panic!("trusted factory unwind")
            }
            Ok(host("copy", &drops))
        });
    }));
    assert!(result.is_err());
    assert_eq!(calls.get(), 2);
    assert_eq!(reserved_nodes.get(), 8);
    let mut events = drops.borrow().clone();
    events.sort();
    assert_eq!(events, ["copy", "left", "right", "template"]);
}

#[test]
fn debug_counts_and_errors_do_not_format_native_payloads_names_or_literals() {
    let drops = Rc::new(RefCell::new(vec![]));
    let plan = HelperReturns::new(
        "secret_alias".into(),
        host("secret_effect", &drops),
        Owned::Literal {
            value: ScalarValue::String("secret literal".into()),
        },
        LIMITS,
    )
    .unwrap();
    let text = format!("{plan:?}");
    assert!(text.contains("returns: 1"));
    assert!(!text.contains("secret"));
    let error: HelperReturnError<PrivateError> = HelperReturnError::Factory {
        index: 3,
        error: PrivateError("secret"),
    };
    assert!(!format!("{error:?} {error}").contains("secret"));
}

#[test]
fn physical_admission_does_not_certify_folded_source_weights_or_native_semantics() {
    let value = Data::Literal {
        value: ScalarValue::OptionalString(OptionalStringValue(None)),
    };
    let plan = HelperReturns::new(
        "answer".into(),
        value,
        local("answer"),
        HelperReturnLimits {
            max_nodes: 3,
            max_depth: 1,
        },
    )
    .unwrap();
    let result = copied(plan);
    assert!(
        measure_source_cost(
            &result,
            SourceCostLimits {
                max_nodes: 3,
                max_depth: 1
            },
            |_| Ok::<_, ()>(leselang_hir::source_cost::SourceCostExtra { nodes: 0, depth: 0 })
        )
        .is_err()
    );
    let value = Data::Literal {
        value: ScalarValue::String("x".repeat(100_000)),
    };
    assert!(HelperReturns::new("answer".into(), value, local("answer"), LIMITS).is_ok());
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
fn unrelated_device_host_preserves_explicit_bind_typing_value_and_exact_fuel() {
    let body = Data::Binary {
        operator: BinaryOperator::Add,
        left: Box::new(local("answer")),
        right: Box::new(integer(1)),
    };
    let original = bind("answer", integer(41), body.clone());
    let result = copied(plan(integer(41), body));
    let typing = TypeInferenceLimits {
        max_nodes: 128,
        max_depth: 16,
        max_bindings: 8,
    };
    assert_eq!(
        infer_pure_type(&result, &[], &Device, typing).unwrap(),
        PureType::Scalar(ScalarType::Integer)
    );
    let limits = PureEvaluationLimits {
        max_nodes: 128,
        max_depth: 16,
        max_bindings: 8,
    };
    let evaluate = |node: &Data, fuel: &mut Fuel| {
        let mut bindings = vec![];
        evaluate_pure_in_scope(
            node,
            &mut ScopeFrame::new(&mut bindings),
            &Device,
            fuel,
            limits,
        )
        .unwrap()
    };
    let mut before = Fuel::new(100);
    let mut after = Fuel::new(100);
    assert!(matches!(
        evaluate(&original, &mut before),
        PureValue::Scalar(ScalarValue::Integer(42))
    ));
    assert!(matches!(
        evaluate(&result, &mut after),
        PureValue::Scalar(ScalarValue::Integer(42))
    ));
    assert_eq!(before.remaining(), after.remaining());
    assert_eq!(
        serde_json::to_vec(&original).unwrap(),
        serde_json::to_vec(&result).unwrap()
    );
}

#[test]
fn reference_adapter_preserves_canonical_wire_capabilities_and_cold_return_diagnostics() {
    let source = r#"fn read(node: string) = choose(when: true, then: bind(r: ui.assert_text(node_id: node, expected: "ok"), body: field(value: r, name: "expected")), otherwise: "skip")
        fn main() = bind(answer: read(node: "status"), body: ui.set_form_value(node_id: "form", field: "value", value: answer))"#;
    let program = leselang_hir::lower(&leselang_syntax::parse(source)).unwrap();
    let bytes = serde_json::to_vec(&program).unwrap();
    let canonical = leselang_hir::canonical_source(&program.function.effect).unwrap();
    let restored = leselang_hir::lower(&leselang_syntax::parse(&canonical)).unwrap();
    assert_eq!(bytes, serde_json::to_vec(&restored).unwrap());
    assert_eq!(program.function.required_capabilities, ["ui.presentation"]);
    assert!(
        leselang_hir::authorize(&program, &leselang_host_contract::CapabilitySet::default())
            .is_err()
    );
    let oversized = format!(
        "fn f0() = bind(r: ui.focus(node_id: \"a\"), body: 0)\n{}fn main() = bind(answer: f6(), body: \"{}\")",
        (1..=6)
            .map(|i| format!(
                "fn f{i}() = choose(when: true, then: f{}(), otherwise: f{}())\n",
                i - 1,
                i - 1
            ))
            .collect::<String>(),
        "x".repeat(4096)
    );
    assert!(
        leselang_hir::lower(&leselang_syntax::parse(&oversized))
            .unwrap_err()
            .iter()
            .any(|error| error.code == "LSH1405")
    );
}

#[test]
fn bounded_normal_return_corpus_matches_legacy_desugaring_and_exact_shape() {
    fn legacy(value: Data, body: &Data) -> Data {
        if value.is_pure() {
            return bind("answer", value, body.clone());
        }
        match value {
            Data::Bind {
                name,
                value,
                body: tail,
            } => bind(&name, *value, legacy(*tail, body)),
            Data::Choose {
                when,
                then,
                otherwise,
            } => Data::Choose {
                when,
                then: Box::new(legacy(*then, body)),
                otherwise: Box::new(legacy(*otherwise, body)),
            },
            value => bind("answer", value, body.clone()),
        }
    }
    let mut corpus = vec![
        integer(0),
        call(1),
        Data::Host {
            effect: Box::new(()),
        },
        Data::Group {
            group_kind: GroupKind::Sequence,
            branches: vec![],
        },
    ];
    for depth in 0..3 {
        let previous = corpus.clone();
        for (index, value) in previous.into_iter().enumerate() {
            corpus.push(bind(
                &format!("v{depth}_{index}"),
                integer(0),
                value.clone(),
            ));
            corpus.push(choose(
                value.clone(),
                bind("pure", integer(0), local("pure")),
            ));
            corpus.push(choose(call(9), value));
        }
    }
    for value in corpus {
        let body = bind("caller", local("answer"), local("caller"));
        let expected = legacy(value.clone(), &body);
        let plan = HelperReturns::new("answer".into(), value, body, LIMITS).unwrap();
        let shape = plan.shape();
        assert_eq!(plan.return_sites().count(), shape.returns);
        let result = copied(plan);
        assert_eq!(result, expected);
        let actual = measure_source_cost(
            &result,
            SourceCostLimits {
                max_nodes: 128,
                max_depth: 16,
            },
            |_| Ok::<_, ()>(leselang_hir::source_cost::SourceCostExtra { nodes: 0, depth: 0 }),
        )
        .unwrap();
        assert_eq!((shape.nodes, shape.depth), (actual.nodes, actual.depth));
    }
}

struct FailingHost {
    prepares: Cell<usize>,
    captures: Cell<usize>,
}
impl PureEvaluationEnvironment<u8, u32> for FailingHost {
    type Result = ();
    type Error = PrivateError;
    fn field(&self, _: &(), _: &u8) -> Result<ScalarValue, PrivateError> {
        Err(PrivateError("field"))
    }
    fn member(&self, _: &(), _: &str, _: &u32) -> Result<(), PrivateError> {
        Err(PrivateError("member"))
    }
}
impl<'expression>
    leselang_hir::effect_evaluation::EffectEvaluationEnvironment<'expression, u8, u32, (), ()>
    for FailingHost
{
    type Request = ();
    type Capture = ();
    fn preflight_effect(&self, _: &'expression Data) -> Result<(), PrivateError> {
        Ok(())
    }
    fn prepare_effect(
        &self,
        _: &'expression Data,
        _: &mut ScopeFrame<'_, 'expression, PureValue<()>>,
        _: &mut Fuel,
    ) -> Result<(), CalculationFailure<PrivateError>> {
        self.prepares.set(self.prepares.get() + 1);
        Err(CalculationFailure::External(PrivateError("native failure")))
    }
    fn capture(
        &self,
        _: &'expression str,
        _: &'expression Data,
        _: &ScopeFrame<'_, 'expression, PureValue<()>>,
        _: &mut Fuel,
    ) -> Result<(), PrivateError> {
        self.captures.set(self.captures.get() + 1);
        Ok(())
    }
}
#[test]
fn native_execution_failure_never_enters_a_normal_return_or_scalar_recovery() {
    use leselang_hir::effect_evaluation::*;
    let body = Data::Recover {
        value: Box::new(local("answer")),
        fallback: Box::new(integer(99)),
    };
    let expression = copied(plan(choose(call(1), call(2)), body));
    let host = FailingHost {
        prepares: Cell::new(0),
        captures: Cell::new(0),
    };
    let mut bindings = vec![];
    let result = evaluate_effects_in_scope(
        &expression,
        &mut ScopeFrame::new(&mut bindings),
        &host,
        &mut Fuel::new(100),
        EffectEvaluationLimits {
            pure: PureEvaluationLimits {
                max_nodes: 128,
                max_depth: 16,
                max_bindings: 8,
            },
            max_arguments: 8,
            max_branches: 8,
        },
    );
    assert!(matches!(
        result,
        Err(CalculationFailure::External(EffectEvaluationFault::Native(
            _
        )))
    ));
    assert_eq!(host.prepares.get(), 1);
    assert_eq!(host.captures.get(), 0);
    assert!(bindings.is_empty());
}
