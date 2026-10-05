use std::cell::{Cell, RefCell};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;

use leselang_hir::call_evaluation::{CallEvaluationLimits, evaluate_call_arguments_in_scope};
use leselang_hir::ir::Computation;
use leselang_hir::pure_evaluation::{
    PureEvaluationEnvironment, PureEvaluationFailure, PureEvaluationFault, PureEvaluationLimits,
    PureValue, evaluate_pure_in_scope,
};
use leselang_hir::pure_typing::{
    PureType, PureTypeEnvironment, PureTypeError, TypeInferenceLimits, infer_pure_type,
};
use leselang_hir::scalar_source::{
    ScalarSourceError, ScalarSourceLimits, ScalarSourceResult, lower_scalar_source,
    lower_scalar_source_with_scope,
};
use leselang_hir::source_call::{
    SourceCallHost, SourceCallLimits, SourceSchema, lower_source_call,
};
use leselang_runtime_core::*;
use leselang_syntax::{Expression, Span, parse};

// All native IR slots/errors remain move-only, non-Debug and GUI-local.
struct Field {
    value: ScalarValue,
    label: &'static str,
    drops: Rc<Cell<usize>>,
}
impl Drop for Field {
    fn drop(&mut self) {
        self.drops.set(self.drops.get() + 1);
    }
}
struct Operation;
struct Effect;
struct ResultTag;
struct PrivateError(&'static str);
type Node = Computation<Field, Operation, Effect, ResultTag>;
const SOURCE: SourceCallLimits = SourceCallLimits {
    max_source_nodes: 256,
    max_source_depth: 32,
    max_lowered_nodes: 256,
    max_lowered_depth: 32,
    max_arguments: 64,
};
const LIMITS: ScalarSourceLimits = ScalarSourceLimits {
    source: SOURCE,
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

struct Editor {
    queries: RefCell<Vec<&'static str>>,
    fail: &'static str,
    unwind: bool,
}
impl Editor {
    fn new() -> Self {
        Self {
            queries: RefCell::new(Vec::new()),
            fail: "",
            unwind: false,
        }
    }
}
impl PureTypeEnvironment<Field, Operation> for Editor {
    type Result = ();
    fn field_type(&self, _: &(), field: &Field) -> Option<ScalarType> {
        Some(field.value.scalar_type())
    }
    fn member_result(&self, _: &(), _: &str, _: &Operation) -> Option<()> {
        None
    }
}
impl PureEvaluationEnvironment<Field, Operation> for Editor {
    type Result = Rc<Cell<u64>>;
    type Error = PrivateError;
    fn field(&self, _: &Self::Result, field: &Field) -> Result<ScalarValue, PrivateError> {
        self.queries.borrow_mut().push(field.label);
        if self.unwind {
            panic!("private native unwind")
        }
        if self.fail == field.label {
            return Err(PrivateError("private native fault"));
        }
        Ok(field.value.clone())
    }
    fn member(
        &self,
        _: &Self::Result,
        _: &str,
        _: &Operation,
    ) -> Result<Self::Result, PrivateError> {
        Err(PrivateError("private member fault"))
    }
}
fn expression(source: &str) -> Expression {
    let tree = parse(&format!("fn main() = {source}"));
    assert!(tree.diagnostics.is_empty(), "{:?}", tree.diagnostics);
    tree.function.unwrap().body
}
fn at(expression: &Expression) -> Span {
    match expression {
        Expression::Call { span, .. }
        | Expression::Reference { span, .. }
        | Expression::Integer { span, .. }
        | Expression::Boolean { span, .. }
        | Expression::None { span }
        | Expression::String { span, .. } => *span,
    }
}
fn literal(value: ScalarValue) -> (Node, Option<ScalarType>) {
    let ty = value.scalar_type();
    (Node::Literal { value }, Some(ty))
}
fn field(
    value: ScalarValue,
    label: &'static str,
    drops: &Rc<Cell<usize>>,
) -> (Node, Option<ScalarType>) {
    let ty = value.scalar_type();
    (
        Node::Field {
            value: Box::new(Node::Local {
                name: "reply".into(),
            }),
            field: Field {
                value,
                label,
                drops: drops.clone(),
            },
        },
        Some(ty),
    )
}
fn list(items: &[&str]) -> ScalarValue {
    ScalarValue::StringList(StringListValue(
        items.iter().map(|item| (*item).into()).collect(),
    ))
}
fn lower(source: &str) -> ScalarSourceResult<Node, PrivateError> {
    lower_scalar_source_with_scope(&expression(source), LIMITS, &[], |_, _| {
        Err(PrivateError("unknown native alias"))
    })
}
fn run(
    node: &Node,
    host: &Editor,
    fuel: &mut Fuel,
) -> Result<PureValue<Rc<Cell<u64>>>, PureEvaluationFailure<PrivateError>> {
    let mut bindings = vec![("reply", PureValue::Result(Rc::new(Cell::new(0))))];
    let mut scope = ScopeFrame::new(&mut bindings);
    let result = evaluate_pure_in_scope(node, &mut scope, host, fuel, PURE);
    assert_eq!(scope.len(), 1);
    result
}

#[test]
fn parsed_folds_cover_every_scalar_accumulator_through_cold_typing_and_execution() {
    for (initial, expected) in [
        ("7", ScalarValue::Integer(7)),
        ("true", ScalarValue::Boolean(true)),
        ("\"seed\"", ScalarValue::String("seed".into())),
        ("none", ScalarValue::None),
        (
            "optional_string(value: none)",
            ScalarValue::OptionalString(OptionalStringValue(None)),
        ),
        ("strings(a: \"seed\")", list(&["seed"])),
    ] {
        let (node, ty) = lower(&format!("fold(acc: {initial}, items: strings(a: \"\", b: \"x\"), item: \"entry\", next: acc, limit: 2)")).unwrap();
        assert_eq!(ty, expected.scalar_type());
        assert_eq!(
            infer_pure_type(&node, &[], &Editor::new(), TYPING),
            Ok(PureType::Scalar(ty))
        );
        assert!(
            matches!(run(&node, &Editor::new(), &mut Fuel::new(1000)).unwrap(), PureValue::Scalar(value) if value == expected)
        );
    }
    let (node, _) = lower("fold(acc: strings(), items: strings(z: \"last\", a: \"\", b: \"first\", c: \"last\"), item: \"entry\", next: append(left: acc, right: entry), limit: 4)").unwrap();
    assert!(
        matches!(run(&node, &Editor::new(), &mut Fuel::new(1000)).unwrap(), PureValue::Scalar(value) if value == list(&["last", "", "first", "last"]))
    );
}

#[test]
fn native_extensions_see_original_items_initial_next_order_and_readonly_scope() {
    let source = expression(
        "fold(next: advance(), limit: 64, item: \"entry\", acc: initial(), items: collection())",
    );
    let Expression::Call { arguments, .. } = &source else {
        panic!()
    };
    let expected = [
        &arguments[4].value,
        &arguments[3].value,
        &arguments[0].value,
    ];
    let mut calls = 0;
    let text = String::from("owned");
    let pointer = text.as_ptr();
    let mut owned = Some(text);
    let (node, _): (Node, _) = lower_scalar_source_with_scope(
        &source,
        LIMITS,
        &[("seed", ScalarType::Boolean)],
        |node, scope| -> Result<_, PrivateError> {
            assert!(std::ptr::eq(node, expected[calls]));
            assert_eq!(scope.get("seed"), Some(ScalarType::Boolean));
            assert_eq!(
                scope.get("acc"),
                (calls == 2).then_some(ScalarType::Integer)
            );
            assert_eq!(
                scope.get("entry"),
                (calls == 2).then_some(ScalarType::String)
            );
            assert_eq!(scope.len(), if calls == 2 { 3 } else { 1 });
            let value = if calls == 0 {
                ScalarValue::StringList(StringListValue(vec![owned.take().unwrap()]))
            } else {
                ScalarValue::Integer(0)
            };
            calls += 1;
            Ok(literal(value))
        },
    )
    .unwrap();
    assert_eq!(calls, 3);
    let Node::Fold {
        items, item, limit, ..
    } = node
    else {
        panic!()
    };
    let Node::Literal {
        value: ScalarValue::StringList(items),
    } = *items
    else {
        panic!()
    };
    assert_eq!(items.0[0].as_ptr(), pointer);
    assert_eq!(item, "entry");
    assert_eq!(limit, 64);
}

#[test]
fn cold_fold_shapes_metadata_labels_and_limits_precede_all_native_extensions() {
    for (fold, kind) in [
        (
            "fold(items: strings(), item: \"entry\", next: 0, limit: 0)",
            0,
        ),
        ("fold(n: 0, items: strings(), item: \"entry\", next: n)", 0),
        (
            "fold(n: 0, items: strings(), item: \"entry\", next: n, limit: 0, extra: 1)",
            0,
        ),
        (
            "fold(n: 0, items: strings(), item: \"entry\", next: n, next: n)",
            0,
        ),
        (
            "fold(n: 0, items: strings(), item: false, next: n, limit: 0)",
            1,
        ),
        (
            "fold(n: 0, items: strings(), item: \"n\", next: n, limit: 0)",
            2,
        ),
        (
            "fold(n: 0, items: strings(), item: \"bad-name\", next: n, limit: 0)",
            2,
        ),
        (
            "fold(n: 0, items: strings(), item: \"entry\", next: n, limit: 65)",
            3,
        ),
        (
            "fold(n: 0, items: strings(), item: \"entry\", next: n, limit: 18446744073709551615)",
            3,
        ),
        (
            "fold(n: 0, items: strings(), item: \"entry\", next: n, limit: true)",
            3,
        ),
        (
            "fold(n: 0, items: strings(), item: \"entry\", next: n, limit: add(left: 1, right: 1))",
            3,
        ),
    ] {
        for source in [
            format!("choose(when: true, then: native(), otherwise: {fold})"),
            format!("native(value: {fold})"),
        ] {
            let result: ScalarSourceResult<Node, PrivateError> =
                lower_scalar_source_with_scope(&expression(&source), LIMITS, &[], |_, _| {
                    panic!("cold refusal reached native callback")
                });
            assert!(
                match kind {
                    0 => matches!(result, Err(ScalarSourceError::FoldShape { .. })),
                    1 => matches!(result, Err(ScalarSourceError::FoldItem { .. })),
                    2 => matches!(result, Err(ScalarSourceError::FoldBindings { .. })),
                    _ => matches!(result, Err(ScalarSourceError::FoldLimit { .. })),
                },
                "{source}"
            );
        }
    }
}

#[test]
fn both_fold_locals_count_against_active_quota_and_cannot_shadow_prefix_or_state() {
    let source =
        expression("fold(n: initial(), items: collection(), item: \"entry\", next: n, limit: 0)");
    for (prefix, quota, expected) in [
        (vec![], 0, PureTypeError::BindingLimit),
        (vec![], 1, PureTypeError::BindingLimit),
        (
            vec![("seed", ScalarType::String)],
            2,
            PureTypeError::BindingLimit,
        ),
        (
            vec![("n", ScalarType::Integer)],
            8,
            PureTypeError::ShadowedBinding,
        ),
        (
            vec![("entry", ScalarType::String)],
            8,
            PureTypeError::ShadowedBinding,
        ),
    ] {
        let result: ScalarSourceResult<Node, PrivateError> = lower_scalar_source_with_scope(
            &source,
            ScalarSourceLimits {
                max_bindings: quota,
                ..LIMITS
            },
            &prefix,
            |_, _| panic!(),
        );
        assert!(
            matches!(result, Err(ScalarSourceError::FoldBindings { span, error }) if span == at(&source) && error == expected)
        );
    }
    for source in [
        "fold(n: initial(), items: collection(), item: \"entry\", next: bind(entry: \"x\", body: n), limit: 0)",
        "fold(n: initial(), items: collection(), item: \"entry\", next: loop(n: 0, while: false, next: n, limit: 0), limit: 0)",
        "fold(n: initial(), items: collection(), item: \"entry\", next: fold(m: 0, items: strings(), item: \"entry\", next: m, limit: 0), limit: 0)",
    ] {
        let result: ScalarSourceResult<Node, PrivateError> =
            lower_scalar_source_with_scope(&expression(source), LIMITS, &[], |_, _| panic!());
        assert!(
            matches!(
                result,
                Err(ScalarSourceError::FoldBindings {
                    error: PureTypeError::ShadowedBinding,
                    ..
                }) | Err(ScalarSourceError::Bindings {
                    error: PureTypeError::ShadowedBinding,
                    ..
                })
            ),
            "{source}"
        );
    }
    let result: ScalarSourceResult<Node, PrivateError> = lower_scalar_source_with_scope(
        &expression(
            "fold(n: 0, items: strings(), item: \"entry\", next: fold(m: 0, items: strings(), item: \"part\", next: m, limit: 0), limit: 0)",
        ),
        ScalarSourceLimits {
            max_bindings: 3,
            ..LIMITS
        },
        &[],
        |_, _| panic!(),
    );
    assert!(matches!(
        result,
        Err(ScalarSourceError::FoldBindings {
            error: PureTypeError::BindingLimit,
            ..
        })
    ));
}

#[test]
fn parent_preparation_temporary_and_sibling_frames_do_not_leak_fold_locals() {
    for source in [
        "fold(n: fold(n: 3, items: strings(), item: \"entry\", next: n, limit: 0), items: strings(), item: \"entry\", next: n, limit: 0)",
        "add(left: fold(n: 1, items: strings(), item: \"entry\", next: n, limit: 0), right: fold(n: 2, items: strings(), item: \"entry\", next: n, limit: 0))",
        "fold(n: 3, items: fold(n: strings(), items: strings(), item: \"entry\", next: n, limit: 0), item: \"entry\", next: n, limit: 0)",
    ] {
        let (node, _): (Node, _) = lower_scalar_source_with_scope(
            &expression(source),
            ScalarSourceLimits {
                max_bindings: 2,
                ..LIMITS
            },
            &[],
            |_, _| -> Result<_, PrivateError> { panic!() },
        )
        .unwrap();
        assert!(matches!(
            run(&node, &Editor::new(), &mut Fuel::new(1000)).unwrap(),
            PureValue::Scalar(ScalarValue::Integer(3))
        ));
    }
    for source in [
        "fold(n: entry, items: strings(), item: \"entry\", next: n, limit: 0)",
        "fold(n: 0, items: strings(a: entry), item: \"entry\", next: n, limit: 0)",
    ] {
        assert!(matches!(
            lower(source),
            Err(ScalarSourceError::Native { .. })
        ));
    }
    let source = expression(
        "add(left: fold(n: 0, items: strings(), item: \"entry\", next: n, limit: 0), right: after())",
    );
    let _: (Node, _) = lower_scalar_source_with_scope(
        &source,
        LIMITS,
        &[],
        |_, scope| -> Result<_, PrivateError> {
            assert!(scope.is_empty());
            Ok(literal(ScalarValue::Integer(0)))
        },
    )
    .unwrap();
}

#[test]
fn collection_type_precedes_initial_and_empty_or_zero_folds_still_type_next() {
    let result: ScalarSourceResult<Node, PrivateError> = lower_scalar_source_with_scope(
        &expression("fold(n: native(), items: 1, item: \"entry\", next: n, limit: 0)"),
        LIMITS,
        &[],
        |_, _| panic!("invalid collection lowered initial"),
    );
    assert!(matches!(result, Err(ScalarSourceError::FoldItems { .. })));
    assert!(matches!(
        lower("fold(n: 0, items: strings(), item: \"entry\", next: false, limit: 0)"),
        Err(ScalarSourceError::FoldState { .. })
    ));
    assert!(matches!(
        lower("fold(n: 0, items: strings(), item: \"entry\", next: missing, limit: 0)"),
        Err(ScalarSourceError::Native { .. })
    ));
    for source in [
        "fold(n: native(), items: strings(), item: \"entry\", next: n, limit: 0)",
        "fold(n: 0, items: native(), item: \"entry\", next: n, limit: 0)",
        "fold(n: 0, items: strings(), item: \"entry\", next: native(), limit: 0)",
    ] {
        let result: ScalarSourceResult<Node, PrivateError> =
            lower_scalar_source_with_scope(&expression(source), LIMITS, &[], |_, _| {
                Ok((
                    Node::Host {
                        effect: Box::new(Effect),
                    },
                    Some(ScalarType::Integer),
                ))
            });
        assert!(matches!(
            result,
            Err(ScalarSourceError::Produced {
                error: PureTypeError::Impure,
                ..
            })
        ));
        let result: ScalarSourceResult<Node, PrivateError> =
            lower_scalar_source_with_scope(&expression(source), LIMITS, &[], |_, _| {
                Ok((
                    Node::Local {
                        name: "reply".into(),
                    },
                    None,
                ))
            });
        assert!(matches!(result, Err(ScalarSourceError::NonScalar { .. })));
    }
}

#[test]
fn item_and_limit_metadata_do_not_inflate_ir_or_refund_literal_constructor_work() {
    for limit in [0, 64] {
        let source = expression(&format!(
            "fold(n: 0, items: strings(), item: \"entry\", next: n, limit: {limit})"
        ));
        let _: (Node, _) = lower_scalar_source_with_scope(
            &source,
            ScalarSourceLimits {
                source: SourceCallLimits {
                    max_source_nodes: 6,
                    max_source_depth: 1,
                    max_lowered_nodes: 4,
                    max_lowered_depth: 1,
                    ..SOURCE
                },
                ..LIMITS
            },
            &[],
            |_, _| -> Result<_, PrivateError> { panic!() },
        )
        .unwrap();
        for nodes in [0, 1, 2, 3] {
            let result: ScalarSourceResult<Node, PrivateError> = lower_scalar_source_with_scope(
                &source,
                ScalarSourceLimits {
                    source: SourceCallLimits {
                        max_lowered_nodes: nodes,
                        ..SOURCE
                    },
                    ..LIMITS
                },
                &[],
                |_, _| panic!(),
            );
            assert!(matches!(
                result,
                Err(ScalarSourceError::Generation {
                    error: StructureError::NodeLimit,
                    ..
                })
            ));
        }
        let result: ScalarSourceResult<Node, PrivateError> = lower_scalar_source_with_scope(
            &source,
            ScalarSourceLimits {
                source: SourceCallLimits {
                    max_source_nodes: 5,
                    ..SOURCE
                },
                ..LIMITS
            },
            &[],
            |_, _| panic!(),
        );
        assert!(matches!(result, Err(ScalarSourceError::Source(_))));
    }
    let source = expression(
        "fold(n: 0, items: strings(a: \"x\", b: \"y\"), item: \"entry\", next: n, limit: 2)",
    );
    let result: ScalarSourceResult<Node, PrivateError> = lower_scalar_source_with_scope(
        &source,
        ScalarSourceLimits {
            source: SourceCallLimits {
                max_lowered_nodes: 5,
                ..SOURCE
            },
            ..LIMITS
        },
        &[],
        |_, _| panic!(),
    );
    assert!(matches!(
        result,
        Err(ScalarSourceError::Generation {
            error: StructureError::NodeLimit,
            ..
        })
    ));
    let _: (Node, _) = lower_scalar_source_with_scope(
        &source,
        ScalarSourceLimits {
            source: SourceCallLimits {
                max_lowered_nodes: 6,
                ..SOURCE
            },
            ..LIMITS
        },
        &[],
        |_, _| -> Result<_, PrivateError> { panic!() },
    )
    .unwrap();
    let source =
        expression("fold(n: 0, items: strings(), item: \"entry\", next: native(), limit: 0)");
    for source_limits in [
        SourceCallLimits {
            max_lowered_nodes: 3,
            ..SOURCE
        },
        SourceCallLimits {
            max_lowered_depth: 0,
            ..SOURCE
        },
    ] {
        let result: ScalarSourceResult<Node, PrivateError> = lower_scalar_source_with_scope(
            &source,
            ScalarSourceLimits {
                source: source_limits,
                ..LIMITS
            },
            &[],
            |_, _| panic!(),
        );
        assert!(matches!(result, Err(ScalarSourceError::Generation { .. })));
    }
}

#[test]
fn actual_empty_and_oversized_collections_do_not_execute_or_truncate_next() {
    let drops = Rc::new(Cell::new(0));
    for items in ["strings()", "strings(a: \"x\")"] {
        let source = expression(&format!(
            "fold(n: 7, items: {items}, item: \"entry\", next: native(), limit: 0)"
        ));
        let (node, _): (Node, _) = lower_scalar_source_with_scope(
            &source,
            LIMITS,
            &[],
            |_, _| -> Result<_, PrivateError> {
                Ok(field(ScalarValue::Integer(1), "next", &drops))
            },
        )
        .unwrap();
        let host = Editor {
            fail: "next",
            ..Editor::new()
        };
        assert_eq!(
            infer_pure_type(&node, &[("reply", PureType::Result(()))], &host, TYPING),
            Ok(PureType::Scalar(ScalarType::Integer))
        );
        let result = run(&node, &host, &mut Fuel::new(100));
        if items == "strings()" {
            assert!(matches!(
                result.unwrap(),
                PureValue::Scalar(ScalarValue::Integer(7))
            ));
        } else {
            assert!(matches!(
                result,
                Err(CalculationFailure::External(PureEvaluationFault::Fold(
                    FoldError::IterationLimit
                )))
            ));
        }
        assert!(host.queries.borrow().is_empty());
    }
    assert_eq!(drops.get(), 2);
}

#[test]
fn shared_source_folds_keep_exact_reference_copy_scan_and_iteration_fuel() {
    for count in [0, 1, 2, 17, 64] {
        for text in ["", "x"] {
            let items = (0..count)
                .map(|index| format!("i{index}: {}", serde_json::to_string(text).unwrap()))
                .collect::<Vec<_>>()
                .join(", ");
            let (node, _) = lower(&format!("fold(n: 0, items: strings({items}), item: \"entry\", next: add(left: n, right: 1), limit: {count})")).unwrap();
            let fuel = 3 + 6 * count + 3 * count * u64::from(!text.is_empty());
            let mut exact = Fuel::new(fuel);
            assert!(
                matches!(run(&node, &Editor::new(), &mut exact).unwrap(), PureValue::Scalar(ScalarValue::Integer(value)) if value == count)
            );
            assert_eq!(exact.remaining(), 0);
            assert!(matches!(
                run(&node, &Editor::new(), &mut Fuel::new(fuel - 1)),
                Err(CalculationFailure::External(
                    PureEvaluationFault::FuelExhausted
                ))
            ));
        }
    }
}

#[test]
fn runtime_collection_and_initial_faults_precede_count_limit_but_count_limit_precedes_next() {
    for (source, expected) in [
        (
            "fold(n: parse_integer(value: \"bad\"), items: strings(a: to_string(value: div(left: 1, right: 0))), item: \"entry\", next: n, limit: 0)",
            0,
        ),
        (
            "fold(n: parse_integer(value: \"bad\"), items: strings(a: \"x\"), item: \"entry\", next: n, limit: 0)",
            1,
        ),
        (
            "recover(value: fold(n: 0, items: strings(a: \"bad\"), item: \"entry\", next: parse_integer(value: entry), limit: 0), fallback: 9)",
            2,
        ),
    ] {
        let (node, _) = lower(source).unwrap();
        let result = run(&node, &Editor::new(), &mut Fuel::new(1000));
        assert!(match expected {
            0 => matches!(
                result,
                Err(CalculationFailure::Scalar(ScalarError::IntegerArithmetic))
            ),
            1 => matches!(
                result,
                Err(CalculationFailure::Scalar(ScalarError::InvalidIntegerText))
            ),
            _ => matches!(
                result,
                Err(CalculationFailure::External(PureEvaluationFault::Fold(
                    FoldError::IterationLimit
                )))
            ),
        });
    }
    let (node, _) = lower("recover(value: fold(n: 0, items: strings(a: \"bad\"), item: \"entry\", next: parse_integer(value: entry), limit: 1), fallback: 9)").unwrap();
    assert!(matches!(
        run(&node, &Editor::new(), &mut Fuel::new(1000)).unwrap(),
        PureValue::Scalar(ScalarValue::Integer(9))
    ));
}

#[test]
fn native_runtime_queries_follow_collection_initial_next_and_faults_do_not_fallback() {
    let source = expression(
        "recover(value: fold(n: initial(), items: collection(), item: \"entry\", next: advance(), limit: 1), fallback: 9)",
    );
    let drops = Rc::new(Cell::new(0));
    let (node, _): (Node, _) = lower_scalar_source_with_scope(
        &source,
        LIMITS,
        &[],
        |node, _| -> Result<_, PrivateError> {
            let Expression::Call { callee, .. } = node else {
                panic!()
            };
            Ok(match callee.as_str() {
                "collection" => field(list(&["x"]), "items", &drops),
                "initial" => field(ScalarValue::Integer(0), "initial", &drops),
                _ => field(ScalarValue::Integer(1), "next", &drops),
            })
        },
    )
    .unwrap();
    let host = Editor::new();
    assert!(matches!(
        run(&node, &host, &mut Fuel::new(100)).unwrap(),
        PureValue::Scalar(ScalarValue::Integer(1))
    ));
    assert_eq!(*host.queries.borrow(), ["items", "initial", "next"]);
    for (fail, queries) in [
        ("items", vec!["items"]),
        ("initial", vec!["items", "initial"]),
        ("next", vec!["items", "initial", "next"]),
    ] {
        let host = Editor {
            fail,
            ..Editor::new()
        };
        assert!(matches!(
            run(&node, &host, &mut Fuel::new(100)),
            Err(CalculationFailure::External(PureEvaluationFault::Native(
                PrivateError("private native fault")
            )))
        ));
        assert_eq!(*host.queries.borrow(), queries);
    }
    drop(node);
    assert_eq!(drops.get(), 3);
}

#[test]
fn lowering_errors_and_unwind_drop_both_prepared_native_operands_once() {
    for unwind in [false, true] {
        let source = expression(
            "fold(acc: initial(), items: collection(), item: \"entry\", next: fail(), limit: 0)",
        );
        let drops = Rc::new(Cell::new(0));
        let calls = Cell::new(0);
        let result = catch_unwind(AssertUnwindSafe(|| {
            lower_scalar_source_with_scope(
                &source,
                LIMITS,
                &[],
                |_, scope| -> Result<_, PrivateError> {
                    calls.set(calls.get() + 1);
                    match calls.get() {
                        1 => {
                            assert!(scope.is_empty());
                            Ok(field(list(&[]), "items", &drops))
                        }
                        2 => {
                            assert!(scope.is_empty());
                            Ok(field(
                                ScalarValue::String("ready".into()),
                                "initial",
                                &drops,
                            ))
                        }
                        _ => {
                            assert_eq!(scope.get("acc"), Some(ScalarType::String));
                            assert_eq!(scope.get("entry"), Some(ScalarType::String));
                            assert_eq!(scope.len(), 2);
                            if unwind {
                                panic!("private source unwind")
                            }
                            Err(PrivateError("private source fault"))
                        }
                    }
                },
            )
        }));
        assert_eq!(calls.get(), 3);
        assert_eq!(drops.get(), 2);
        if unwind {
            assert!(result.is_err());
        } else {
            let error = result.unwrap().err().unwrap();
            assert!(!format!("{error:?}").contains("private source fault"));
        }
        assert!(
            lower("fold(acc: 1, items: strings(), item: \"entry\", next: acc, limit: 0)").is_ok()
        );
    }
}

#[test]
fn native_next_unwind_restores_exact_prefix_and_consumed_fuel() {
    let drops = Rc::new(Cell::new(0));
    let (node, _): (Node, _) = lower_scalar_source_with_scope(
        &expression(
            "fold(acc: 0, items: strings(a: \"x\"), item: \"entry\", next: native(), limit: 1)",
        ),
        LIMITS,
        &[],
        |_, _| -> Result<_, PrivateError> { Ok(field(ScalarValue::Integer(1), "next", &drops)) },
    )
    .unwrap();
    let reply = Rc::new(Cell::new(7));
    let mut bindings = vec![("reply", PureValue::Result(reply.clone()))];
    let mut scope = ScopeFrame::new(&mut bindings);
    let mut fuel = Fuel::new(100);
    let host = Editor {
        unwind: true,
        ..Editor::new()
    };
    assert!(
        catch_unwind(AssertUnwindSafe(|| evaluate_pure_in_scope(
            &node, &mut scope, &host, &mut fuel, PURE
        )))
        .is_err()
    );
    assert_eq!(scope.len(), 1);
    assert!(scope.get("acc").is_none());
    assert!(scope.get("entry").is_none());
    assert!(
        matches!(scope.get("reply"), Some(PureValue::Result(value)) if Rc::ptr_eq(value, &reply))
    );
    assert!(fuel.remaining() < 100);
    drop(scope);
    drop(bindings);
    drop(node);
    assert_eq!(drops.get(), 1);
}

#[test]
fn dynamic_type_observations_do_not_replace_complete_cold_fold_typing_or_legacy_extensions() {
    let (node, ty): (Node, _) = lower_scalar_source_with_scope(
        &expression("fold(acc: 0, items: strings(), item: \"entry\", next: native(), limit: 0)"),
        LIMITS,
        &[],
        |_, scope| -> Result<_, PrivateError> {
            assert_eq!(scope.get("acc"), Some(ScalarType::Integer));
            assert_eq!(scope.get("entry"), Some(ScalarType::String));
            Ok((
                Node::Local {
                    name: "missing".into(),
                },
                Some(ScalarType::Integer),
            ))
        },
    )
    .unwrap();
    assert_eq!(ty, ScalarType::Integer);
    assert_eq!(
        infer_pure_type(&node, &[], &Editor::new(), TYPING),
        Err(PureTypeError::UnknownLocal)
    );
    let source = expression("fold(acc: 0, items: strings(), item: \"entry\", next: acc, limit: 0)");
    let _: (Node, _) = lower_scalar_source(&source, SOURCE, |node| -> Result<_, PrivateError> {
        assert!(std::ptr::eq(node, &source));
        Ok(literal(ScalarValue::Integer(5)))
    })
    .unwrap();
}

struct CountDomain;
impl ScalarArgumentDomain for CountDomain {
    fn scalar_types(&self) -> ScalarTypeSet {
        ScalarTypeSet::only(ScalarType::Integer)
    }
    fn accepts_literal(&self, value: &ScalarValue) -> bool {
        matches!(value, ScalarValue::Integer(value) if *value <= 8)
    }
}

#[test]
fn parsed_native_call_prepares_fold_value_against_original_schema_and_runtime_prefix() {
    let source = expression(
        "panel.set(count: fold(acc: seed, items: rows, item: \"entry\", next: add(left: acc, right: len(value: entry)), limit: 8))",
    );
    let parameters = [NamedParameter::required("count", CountDomain)];
    let schemas = [SourceSchema {
        key: "panel.set",
        parameters: &parameters,
        result: ResultTag,
        required_capability: 7u8,
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
    let lowered = lower_source_call(
        &source,
        &SourceCallHost {
            catalog: &catalog,
            version: 3,
            granted: &[7],
        },
        SOURCE,
        |argument| {
            let (node, ty): (Node, _) = lower_scalar_source_with_scope(
                &argument.value,
                LIMITS,
                &[
                    ("seed", ScalarType::Integer),
                    ("rows", ScalarType::StringList),
                ],
                |_, _| -> Result<_, PrivateError> { panic!() },
            )?;
            assert_eq!(
                infer_pure_type(
                    &node,
                    &[
                        ("seed", PureType::Scalar(ScalarType::Integer)),
                        ("rows", PureType::Scalar(ScalarType::StringList))
                    ],
                    &Editor::new(),
                    TYPING
                ),
                Ok(PureType::Scalar(ty))
            );
            Ok::<_, ScalarSourceError<PrivateError>>((node, Some(ty)))
        },
    )
    .unwrap();
    assert!(std::ptr::eq(lowered.schema(), &schemas[0]));
    let arguments = lowered.into_arguments();
    let mut bindings = vec![
        ("seed", PureValue::Scalar(ScalarValue::Integer(2))),
        ("rows", PureValue::Scalar(list(&["x", "yz"]))),
    ];
    let mut scope = ScopeFrame::new(&mut bindings);
    let prepared = evaluate_call_arguments_in_scope(
        &arguments,
        &schemas[0],
        &mut scope,
        &Editor::new(),
        &mut Fuel::new(100),
        CallEvaluationLimits {
            pure: PURE,
            max_arguments: 1,
        },
    )
    .unwrap();
    assert!(std::ptr::eq(prepared.schema(), &schemas[0]));
    assert_eq!(prepared.arguments()[0].value, ScalarValue::Integer(5));
    assert_eq!(scope.len(), 2);
    assert!(scope.get("acc").is_none());
    assert!(scope.get("entry").is_none());
}

#[test]
fn reference_fold_diagnostics_spans_order_wire_and_host_authority_are_preserved() {
    for (source, code, message, location) in [
        (
            "fold(items: strings(), item: \"entry\", next: 0, limit: 0)",
            "LSH1413",
            "fold requires a named initial state",
            "root",
        ),
        (
            "fold(n: missing, items: strings(), item: \"entry\", next: n)",
            "LSH1401",
            "expected exactly the named arguments: n, items, item, next, limit",
            "root",
        ),
        (
            "fold(body: missing, items: strings(), item: false, next: 0, limit: true)",
            "LSH1413",
            "fold item requires a literal local name",
            "item",
        ),
        (
            "fold(n: missing, items: strings(), item: \"n\", next: n, limit: true)",
            "LSH1403",
            "fold locals must be bounded, distinct and cannot shadow active bindings",
            "root",
        ),
        (
            "fold(n: missing, items: strings(), item: \"entry\", next: n, limit: true)",
            "LSH1413",
            "fold limit requires an integer literal from 0 through 64",
            "limit",
        ),
        (
            "fold(n: missing, items: strings(), item: \"entry\", next: n, limit: 65)",
            "LSH1413",
            "fold limit exceeds 64 entries",
            "limit",
        ),
        (
            "fold(n: missing, items: 1, item: \"entry\", next: n, limit: 0)",
            "LSH1402",
            "fold items requires a string_list",
            "items",
        ),
        (
            "fold(n: runtime.list(), items: strings(), item: \"entry\", next: n, limit: 0)",
            "LSH1402",
            "expected a pure scalar expression, not a host operation",
            "n",
        ),
        (
            "fold(n: 0, items: strings(), item: \"entry\", next: false, limit: 0)",
            "LSH1402",
            "fold next must preserve its initial state's type",
            "next",
        ),
    ] {
        let tree = parse(&format!("fn main() = {source}"));
        let body = &tree.function.as_ref().unwrap().body;
        let Expression::Call { arguments, .. } = body else {
            panic!()
        };
        let expected = if location == "root" {
            at(body)
        } else {
            at(&arguments
                .iter()
                .find(|argument| argument.name == location)
                .unwrap()
                .value)
        };
        let errors = leselang_hir::lower(&tree).unwrap_err();
        assert_eq!(errors[0].code, code, "{source}");
        assert_eq!(errors[0].message, message, "{source}");
        assert_eq!(errors[0].span, Some(expected), "{source}");
    }
    for source in [
        "fn main() = fold(n: 0, items: strings(a: \"x\"), item: \"items\", next: add(left: n, right: len(value: items)), limit: 1)",
        "fn main() = bind(r: runtime.list(), body: fold(n: field(value: r, name: \"count\"), items: strings(a: \"x\"), item: \"entry\", next: add(left: n, right: len(value: entry)), limit: 1))",
    ] {
        let program = leselang_hir::lower(&parse(source)).unwrap();
        if source.contains("runtime.list") {
            assert_eq!(program.function.required_capabilities, ["runtime.read"]);
            assert!(
                leselang_hir::authorize(
                    &program,
                    &leselang_host_contract::CapabilitySet::default()
                )
                .is_err()
            );
        }
        let wire = serde_json::to_vec(&program.function.effect).unwrap();
        let canonical = leselang_hir::canonical_source(&program.function.effect).unwrap();
        let roundtrip = leselang_hir::lower(&parse(&canonical)).unwrap();
        assert_eq!(roundtrip, program);
        assert_eq!(
            serde_json::to_vec(&roundtrip.function.effect).unwrap(),
            wire
        );
    }
}
