use std::cell::Cell;
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

// Native slots and errors have no Clone/Debug/serde/Send requirements.
struct Field {
    ty: ScalarType,
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
    queries: Cell<usize>,
    fail: bool,
    unwind: bool,
}
impl Editor {
    fn new() -> Self {
        Self {
            queries: Cell::new(0),
            fail: false,
            unwind: false,
        }
    }
}
impl PureTypeEnvironment<Field, Operation> for Editor {
    type Result = ();
    fn field_type(&self, _: &(), field: &Field) -> Option<ScalarType> {
        Some(field.ty)
    }
    fn member_result(&self, _: &(), _: &str, _: &Operation) -> Option<()> {
        None
    }
}
impl PureEvaluationEnvironment<Field, Operation> for Editor {
    type Result = Rc<Cell<u64>>;
    type Error = PrivateError;
    fn field(&self, _: &Self::Result, field: &Field) -> Result<ScalarValue, PrivateError> {
        self.queries.set(self.queries.get() + 1);
        if self.unwind {
            panic!("private native unwind")
        }
        if self.fail {
            return Err(PrivateError("private native fault"));
        }
        Ok(match field.ty {
            ScalarType::Boolean => ScalarValue::Boolean(true),
            ScalarType::String => ScalarValue::String("ready".into()),
            _ => ScalarValue::Integer(1),
        })
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
fn field(ty: ScalarType, drops: &Rc<Cell<usize>>) -> (Node, Option<ScalarType>) {
    (
        Node::Field {
            value: Box::new(Node::Local {
                name: "reply".into(),
            }),
            field: Field {
                ty,
                drops: drops.clone(),
            },
        },
        Some(ty),
    )
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
fn parsed_loops_cover_all_scalar_state_types_through_cold_typing_and_execution() {
    for (source, expected) in [
        (
            "loop(n: 0, while: lt(left: n, right: 3), next: add(left: n, right: 1), limit: 3)",
            ScalarValue::Integer(3),
        ),
        (
            "loop(next: false, limit: 1, ready: true, while: ready)",
            ScalarValue::Boolean(false),
        ),
        (
            "loop(text: \"\", while: lt(left: len(value: text), right: 3), next: concat(left: text, right: \"x\"), limit: 3)",
            ScalarValue::String("xxx".into()),
        ),
        (
            "loop(unit: none, while: false, next: unit, limit: 0)",
            ScalarValue::None,
        ),
        (
            "loop(text: optional_string(value: \"x\"), while: false, next: text, limit: 0)",
            ScalarValue::OptionalString(OptionalStringValue(Some("x".into()))),
        ),
        (
            "loop(rows: strings(a: \"x\"), while: false, next: rows, limit: 0)",
            ScalarValue::StringList(StringListValue(vec!["x".into()])),
        ),
        (
            "bind(target: 4, body: loop(n: 0, while: lt(left: n, right: target), next: add(left: n, right: 1), limit: 8))",
            ScalarValue::Integer(4),
        ),
    ] {
        let (node, ty) = lower(source).unwrap();
        assert_eq!(ty, expected.scalar_type());
        assert_eq!(
            infer_pure_type(&node, &[], &Editor::new(), TYPING),
            Ok(PureType::Scalar(ty))
        );
        assert!(
            matches!(run(&node, &Editor::new(), &mut Fuel::new(10_000)).unwrap(), PureValue::Scalar(value) if value == expected),
            "{source}"
        );
    }
}

#[test]
fn loop_extensions_observe_original_ast_in_initial_condition_next_order() {
    let source = expression("loop(next: advance(), limit: 1024, n: initial(), while: guard())");
    let Expression::Call { arguments, .. } = &source else {
        panic!()
    };
    let expected = [
        &arguments[2].value,
        &arguments[3].value,
        &arguments[0].value,
    ];
    let mut calls = 0;
    let (node, _): (Node, _) = lower_scalar_source_with_scope(
        &source,
        LIMITS,
        &[("seed", ScalarType::String)],
        |node, scope| -> Result<_, PrivateError> {
            assert!(std::ptr::eq(node, expected[calls]));
            assert_eq!(scope.get("seed"), Some(ScalarType::String));
            assert_eq!(scope.get("n"), (calls != 0).then_some(ScalarType::Integer));
            assert_eq!(scope.len(), if calls == 0 { 1 } else { 2 });
            let value = if calls == 1 {
                ScalarValue::Boolean(false)
            } else {
                ScalarValue::Integer(0)
            };
            calls += 1;
            let ty = value.scalar_type();
            Ok((Node::Literal { value }, Some(ty)))
        },
    )
    .unwrap();
    assert_eq!(calls, 3);
    assert!(matches!(node, Node::Loop { limit: 1024, .. }));
}

#[test]
fn cold_loop_shapes_and_literal_limits_reject_before_any_native_extension() {
    for (loop_source, shape) in [
        ("loop(while: false, next: 0, limit: 0)", true),
        ("loop(n: 0, while: false, next: n)", true),
        (
            "loop(n: 0, while: false, next: n, limit: 0, extra: 1)",
            true,
        ),
        ("loop(n: 0, while: false, next: n, next: n)", true),
        ("loop(n: 0, while: false, next: n, limit: true)", false),
        ("loop(n: 0, while: false, next: n, limit: 1025)", false),
        (
            "loop(n: 0, while: false, next: n, limit: 18446744073709551615)",
            false,
        ),
        (
            "loop(n: 0, while: false, next: n, limit: add(left: 1, right: 1))",
            false,
        ),
    ] {
        for source in [
            format!("choose(when: true, then: native(), otherwise: {loop_source})"),
            format!("native(value: {loop_source})"),
        ] {
            let result: ScalarSourceResult<Node, PrivateError> =
                lower_scalar_source_with_scope(&expression(&source), LIMITS, &[], |_, _| {
                    panic!("cold refusal reached callback")
                });
            assert!(
                if shape {
                    matches!(result, Err(ScalarSourceError::LoopShape { .. }))
                } else {
                    matches!(result, Err(ScalarSourceError::LoopLimit { .. }))
                },
                "{source}"
            );
        }
    }
    assert!(matches!(
        lower("loop(body: 0, while: false, next: 0, limit: 0)"),
        Err(ScalarSourceError::Bindings {
            error: PureTypeError::InvalidName,
            ..
        })
    ));
}

#[test]
fn loop_state_shadow_and_quota_fences_include_cold_and_nested_paths() {
    for (source, prefix, max_bindings, expected) in [
        (
            "loop(n: native(), while: false, next: n, limit: 0)",
            vec![("n", ScalarType::Integer)],
            8,
            PureTypeError::ShadowedBinding,
        ),
        (
            "loop(n: native(), while: false, next: n, limit: 0)",
            vec![],
            0,
            PureTypeError::BindingLimit,
        ),
        (
            "loop(n: native(), while: false, next: bind(n: 1, body: n), limit: 0)",
            vec![],
            8,
            PureTypeError::ShadowedBinding,
        ),
        (
            "loop(n: native(), while: false, next: loop(n: 1, while: false, next: n, limit: 0), limit: 0)",
            vec![],
            8,
            PureTypeError::ShadowedBinding,
        ),
        (
            "loop(n: native(), while: false, next: loop(m: 1, while: false, next: m, limit: 0), limit: 0)",
            vec![],
            1,
            PureTypeError::BindingLimit,
        ),
    ] {
        let result: ScalarSourceResult<Node, PrivateError> = lower_scalar_source_with_scope(
            &expression(source),
            ScalarSourceLimits {
                max_bindings,
                ..LIMITS
            },
            &prefix,
            |_, _| panic!(),
        );
        assert!(
            matches!(result, Err(ScalarSourceError::Bindings { error, .. }) if error == expected),
            "{source}"
        );
    }
}

#[test]
fn initializer_and_sibling_scopes_do_not_leak_loop_state() {
    for source in [
        "loop(n: loop(n: 3, while: false, next: n, limit: 0), while: false, next: n, limit: 0)",
        "add(left: loop(n: 1, while: false, next: n, limit: 0), right: loop(n: 2, while: false, next: n, limit: 0))",
        "loop(n: 3, while: bind(tmp: false, body: tmp), next: bind(tmp: n, body: tmp), limit: 0)",
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
            run(&node, &Editor::new(), &mut Fuel::new(100)).unwrap(),
            PureValue::Scalar(ScalarValue::Integer(3))
        ));
    }
    assert!(matches!(
        lower("loop(n: n, while: false, next: n, limit: 0)"),
        Err(ScalarSourceError::Native { .. })
    ));
    let source =
        expression("add(left: loop(n: 0, while: false, next: n, limit: 0), right: after())");
    let _: (Node, _) = lower_scalar_source_with_scope(
        &source,
        LIMITS,
        &[],
        |_, scope| -> Result<_, PrivateError> {
            assert!(scope.is_empty());
            Ok((
                Node::Literal {
                    value: ScalarValue::Integer(0),
                },
                Some(ScalarType::Integer),
            ))
        },
    )
    .unwrap();
}

#[test]
fn condition_type_precedes_next_lowering_and_zero_limit_does_not_skip_state_typing() {
    let source = expression("loop(n: 0, while: 1, next: native(), limit: 0)");
    let result: ScalarSourceResult<Node, PrivateError> =
        lower_scalar_source_with_scope(&source, LIMITS, &[], |_, _| {
            panic!("invalid condition lowered next")
        });
    let Expression::Call { arguments, .. } = &source else {
        panic!()
    };
    assert!(
        matches!(result, Err(ScalarSourceError::LoopCondition { span }) if span == at(&arguments[1].value))
    );
    assert!(matches!(
        lower("loop(n: 0, while: false, next: false, limit: 0)"),
        Err(ScalarSourceError::LoopState { .. })
    ));
    assert!(matches!(
        lower("loop(n: 0, while: false, next: missing, limit: 0)"),
        Err(ScalarSourceError::Native { .. })
    ));
    for part in [0, 1, 2] {
        let source = match part {
            0 => "loop(n: native(), while: false, next: n, limit: 0)",
            1 => "loop(n: 0, while: native(), next: n, limit: 0)",
            _ => "loop(n: 0, while: false, next: native(), limit: 0)",
        };
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
fn source_and_generated_ir_budgets_are_separate_and_limits_are_not_unrolled() {
    for limit in [0, 1024] {
        let source = expression(&format!(
            "loop(n: 0, while: false, next: n, limit: {limit})"
        ));
        let _: (Node, _) = lower_scalar_source_with_scope(
            &source,
            ScalarSourceLimits {
                source: SourceCallLimits {
                    max_source_nodes: 5,
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
    }
    let source = expression("loop(n: 0, while: false, next: native(), limit: 0)");
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
            |_, _| panic!("budget reached native child"),
        );
        assert!(matches!(result, Err(ScalarSourceError::Generation { .. })));
    }
}

#[test]
fn zero_and_exact_iteration_limits_check_condition_before_next() {
    let (node, _) = lower(
        "loop(n: 0, while: lt(left: n, right: 1024), next: add(left: n, right: 1), limit: 1024)",
    )
    .unwrap();
    assert!(matches!(
        run(&node, &Editor::new(), &mut Fuel::new(20_000)).unwrap(),
        PureValue::Scalar(ScalarValue::Integer(1024))
    ));
    let drops = Rc::new(Cell::new(0));
    for (source, ty) in [
        (
            "loop(text: \"done\", while: false, next: native(), limit: 0)",
            ScalarType::String,
        ),
        (
            "loop(n: 0, while: true, next: native(), limit: 0)",
            ScalarType::Integer,
        ),
    ] {
        let (node, _): (Node, _) = lower_scalar_source_with_scope(
            &expression(source),
            LIMITS,
            &[],
            |_, _| -> Result<_, PrivateError> { Ok(field(ty, &drops)) },
        )
        .unwrap();
        let host = Editor {
            fail: true,
            ..Editor::new()
        };
        assert!(infer_pure_type(&node, &[("reply", PureType::Result(()))], &host, TYPING).is_ok());
        let result = run(&node, &host, &mut Fuel::new(100));
        if ty == ScalarType::String {
            assert!(
                matches!(result.unwrap(), PureValue::Scalar(ScalarValue::String(value)) if value == "done")
            );
        } else {
            assert!(matches!(
                result,
                Err(CalculationFailure::External(PureEvaluationFault::Loop(
                    LoopError::IterationLimit
                )))
            ));
        }
        assert_eq!(host.queries.get(), 0);
    }
    assert_eq!(drops.get(), 2);
}

#[test]
fn loop_exhaustion_fuel_and_native_faults_do_not_become_calculation_fallbacks() {
    for (source, fuel, exhausted) in [
        (
            "recover(value: loop(n: 0, while: true, next: n, limit: 1), fallback: 7)",
            100,
            true,
        ),
        (
            "recover(value: loop(n: 0, while: true, next: n, limit: 1024), fallback: 7)",
            3,
            false,
        ),
    ] {
        let (node, _) = lower(source).unwrap();
        let result = run(&node, &Editor::new(), &mut Fuel::new(fuel));
        assert!(if exhausted {
            matches!(
                result,
                Err(CalculationFailure::External(PureEvaluationFault::Loop(
                    LoopError::IterationLimit
                )))
            )
        } else {
            matches!(
                result,
                Err(CalculationFailure::External(
                    PureEvaluationFault::FuelExhausted
                ))
            )
        });
    }
    let drops = Rc::new(Cell::new(0));
    let source =
        expression("recover(value: loop(n: 0, while: guard(), next: n, limit: 1), fallback: 7)");
    let (node, _): (Node, _) =
        lower_scalar_source_with_scope(&source, LIMITS, &[], |_, _| -> Result<_, PrivateError> {
            Ok(field(ScalarType::Boolean, &drops))
        })
        .unwrap();
    let host = Editor {
        fail: true,
        ..Editor::new()
    };
    assert!(matches!(
        run(&node, &host, &mut Fuel::new(100)),
        Err(CalculationFailure::External(PureEvaluationFault::Native(
            PrivateError("private native fault")
        )))
    ));
    assert_eq!(host.queries.get(), 1);
    let (node, _) = lower("recover(value: loop(n: 0, while: true, next: div(left: n, right: 0), limit: 1), fallback: 7)").unwrap();
    assert!(matches!(
        run(&node, &Editor::new(), &mut Fuel::new(100)).unwrap(),
        PureValue::Scalar(ScalarValue::Integer(7))
    ));
}

#[test]
fn lowering_errors_and_unwind_drop_partial_native_loop_ir_once() {
    for unwind in [false, true] {
        let source = expression("loop(text: initial(), while: guard(), next: fail(), limit: 0)");
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
                            Ok(field(ScalarType::String, &drops))
                        }
                        2 => {
                            assert_eq!(scope.get("text"), Some(ScalarType::String));
                            Ok(field(ScalarType::Boolean, &drops))
                        }
                        _ => {
                            assert_eq!(scope.len(), 1);
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
        assert!(lower("loop(text: \"clean\", while: false, next: text, limit: 0)").is_ok());
    }
}

#[test]
fn native_runtime_unwind_restores_prefix_without_refunding_fuel() {
    let drops = Rc::new(Cell::new(0));
    let (node, _): (Node, _) = lower_scalar_source_with_scope(
        &expression("loop(n: 0, while: guard(), next: n, limit: 1)"),
        LIMITS,
        &[],
        |_, _| -> Result<_, PrivateError> { Ok(field(ScalarType::Boolean, &drops)) },
    )
    .unwrap();
    let reply = Rc::new(Cell::new(9));
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
    assert!(scope.get("n").is_none());
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
fn native_type_observations_still_require_complete_cold_loop_ir_inference() {
    let (node, ty): (Node, _) = lower_scalar_source_with_scope(
        &expression("loop(n: 0, while: false, next: native(), limit: 0)"),
        LIMITS,
        &[],
        |_, scope| -> Result<_, PrivateError> {
            assert_eq!(scope.get("n"), Some(ScalarType::Integer));
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
    let source = expression("loop(n: 0, while: false, next: n, limit: 0)");
    let _: (Node, _) = lower_scalar_source(&source, SOURCE, |node| -> Result<_, PrivateError> {
        assert!(std::ptr::eq(node, &source));
        Ok((
            Node::Literal {
                value: ScalarValue::Integer(5),
            },
            Some(ScalarType::Integer),
        ))
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
fn parsed_native_call_prepares_loop_value_against_original_schema_and_prefix() {
    let source = expression(
        "panel.set(count: loop(n: 0, while: lt(left: n, right: target), next: add(left: n, right: 1), limit: 8))",
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
                &[("target", ScalarType::Integer)],
                |_, _| -> Result<_, PrivateError> { panic!() },
            )?;
            assert_eq!(
                infer_pure_type(
                    &node,
                    &[("target", PureType::Scalar(ScalarType::Integer))],
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
    let mut bindings = vec![("target", PureValue::Scalar(ScalarValue::Integer(3)))];
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
    assert_eq!(prepared.arguments()[0].value, ScalarValue::Integer(3));
    assert_eq!(scope.len(), 1);
    assert!(scope.get("n").is_none());
}

#[test]
fn reference_loop_diagnostics_spans_order_wire_and_authority_are_preserved() {
    for (source, code, message, location) in [
        (
            "loop(while: false, next: 0, limit: 0)",
            "LSH1410",
            "loop requires a named initial state",
            "root",
        ),
        (
            "loop(n: missing, while: false, next: n)",
            "LSH1401",
            "expected exactly the named arguments: n, while, next, limit",
            "root",
        ),
        (
            "loop(body: missing, while: false, next: 0, limit: true)",
            "LSH1403",
            "loop state name must be bounded and cannot shadow an active binding",
            "binding",
        ),
        (
            "loop(n: missing, while: false, next: n, limit: true)",
            "LSH1410",
            "loop limit must be an integer literal from 0 through 1024",
            "limit",
        ),
        (
            "loop(n: missing, while: false, next: n, limit: 1025)",
            "LSH1410",
            "loop limit exceeds 1024 iterations",
            "limit",
        ),
        (
            "loop(n: 0, while: 1, next: missing, limit: 0)",
            "LSH1402",
            "loop while requires a boolean",
            "while",
        ),
        (
            "loop(n: 0, while: false, next: false, limit: 0)",
            "LSH1402",
            "loop next must preserve the initial state's scalar type",
            "next",
        ),
        (
            "loop(n: runtime.list(), while: false, next: n, limit: 0)",
            "LSH1402",
            "expected a pure scalar expression, not a host operation",
            "n",
        ),
    ] {
        let tree = parse(&format!("fn main() = {source}"));
        let body = &tree.function.as_ref().unwrap().body;
        let Expression::Call { arguments, .. } = body else {
            panic!()
        };
        let expected = if location == "root" {
            at(body)
        } else if location == "binding" {
            arguments[0].span
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
    let tree = parse(
        "fn main() = bind(r: runtime.list(), body: loop(n: 1, while: lt(left: n, right: field(value: r, name: \"revision\")), next: mul(left: n, right: 2), limit: 64))",
    );
    let program = leselang_hir::lower(&tree).unwrap();
    assert_eq!(program.function.required_capabilities, ["runtime.read"]);
    assert!(
        leselang_hir::authorize(&program, &leselang_host_contract::CapabilitySet::default())
            .is_err()
    );
    let wire = serde_json::to_vec(&program.function.effect).unwrap();
    let canonical = leselang_hir::canonical_source(&program.function.effect).unwrap();
    let roundtrip = leselang_hir::lower(&parse(&canonical)).unwrap();
    assert_eq!(roundtrip, program);
    assert_eq!(
        serde_json::to_vec(&roundtrip.function.effect).unwrap(),
        wire
    );
}
