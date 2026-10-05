use std::cell::Cell;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;

use leselang_hir::call_evaluation::{CallEvaluationLimits, evaluate_call_arguments_in_scope};
use leselang_hir::ir::Computation;
use leselang_hir::pure_evaluation::{
    PureEvaluationEnvironment, PureEvaluationFailure, PureEvaluationLimits, PureValue,
    evaluate_pure_in_scope,
};
use leselang_hir::pure_typing::{
    PureType, PureTypeEnvironment, PureTypeError, TypeInferenceLimits, infer_pure_type,
};
use leselang_hir::scalar_source::{
    ScalarSourceError, ScalarSourceLimits, ScalarSourceResult, lower_scalar_source,
    lower_scalar_source_with_scope,
};
use leselang_hir::source_call::{
    SourceCallError, SourceCallHost, SourceCallLimits, SourceSchema, lower_source_call,
};
use leselang_runtime_core::*;
use leselang_syntax::{Expression, Span, parse};

// Every native slot/error remains move-only, non-Debug and GUI-local.
struct Field(Rc<Cell<usize>>);
impl Drop for Field {
    fn drop(&mut self) {
        self.0.set(self.0.get() + 1);
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

struct Editor;
impl PureTypeEnvironment<Field, Operation> for Editor {
    type Result = ();
    fn field_type(&self, _: &(), _: &Field) -> Option<ScalarType> {
        Some(ScalarType::String)
    }
    fn member_result(&self, _: &(), _: &str, _: &Operation) -> Option<()> {
        None
    }
}
impl PureEvaluationEnvironment<Field, Operation> for Editor {
    type Result = Rc<String>;
    type Error = PrivateError;
    fn field(&self, _: &Rc<String>, _: &Field) -> Result<ScalarValue, PrivateError> {
        Err(PrivateError("private field failure"))
    }
    fn member(&self, _: &Rc<String>, _: &str, _: &Operation) -> Result<Rc<String>, PrivateError> {
        Err(PrivateError("private member failure"))
    }
}
fn expression(source: &str) -> Expression {
    let parsed = parse(&format!("fn main() = {source}"));
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    parsed.function.unwrap().body
}
fn literal(value: ScalarValue) -> (Node, Option<ScalarType>) {
    let ty = value.scalar_type();
    (Node::Literal { value }, Some(ty))
}
fn lower(source: &str) -> ScalarSourceResult<Node, PrivateError> {
    lower_scalar_source_with_scope(&expression(source), LIMITS, &[], |_, _| {
        Err(PrivateError("unknown native alias"))
    })
}
fn evaluate(node: &Node) -> Result<PureValue<Rc<String>>, PureEvaluationFailure<PrivateError>> {
    let mut bindings = Vec::new();
    let mut scope = ScopeFrame::new(&mut bindings);
    let result = evaluate_pure_in_scope(node, &mut scope, &Editor, &mut Fuel::new(100), PURE);
    assert!(scope.is_empty());
    result
}
fn source_span(expression: &Expression) -> Span {
    match expression {
        Expression::Call { span, .. }
        | Expression::Reference { span, .. }
        | Expression::Integer { span, .. }
        | Expression::Boolean { span, .. }
        | Expression::None { span }
        | Expression::String { span, .. } => *span,
    }
}
fn binding_span(expression: &Expression) -> Span {
    let Expression::Call { arguments, .. } = expression else {
        panic!()
    };
    arguments
        .iter()
        .find(|argument| argument.name != "body")
        .unwrap()
        .span
}

#[test]
fn nested_bindings_flow_from_parsed_source_through_cold_typing_and_execution() {
    let (node, ty) = lower(
        "bind(body: bind(total: add(left: count, right: 2), body: join(left: strings(a: to_string(value: total), b: recover(value: to_string(value: parse_integer(value: \"bad\")), fallback: \"safe\")), right: \",\")), count: 5)",
    ).unwrap();
    assert_eq!(ty, ScalarType::String);
    assert_eq!(
        infer_pure_type(&node, &[], &Editor, TYPING),
        Ok(PureType::Scalar(ty))
    );
    assert!(
        matches!(evaluate(&node).unwrap(), PureValue::Scalar(ScalarValue::String(value)) if value == "7,safe")
    );
}

#[test]
fn prefix_locals_are_lowered_without_native_callbacks_and_runtime_prefix_is_preserved() {
    let source =
        expression("bind(next: add(left: seed, right: 2), body: add(left: next, right: seed))");
    let (node, ty): (Node, _) = lower_scalar_source_with_scope(
        &source,
        LIMITS,
        &[("seed", ScalarType::Integer)],
        |_, _| -> Result<_, PrivateError> { panic!("bound local delegated") },
    )
    .unwrap();
    assert_eq!(
        infer_pure_type(
            &node,
            &[("seed", PureType::Scalar(ScalarType::Integer))],
            &Editor,
            TYPING
        ),
        Ok(PureType::Scalar(ty))
    );
    let mut bindings = vec![("seed", PureValue::Scalar(ScalarValue::Integer(5)))];
    let mut scope = ScopeFrame::new(&mut bindings);
    assert!(matches!(
        evaluate_pure_in_scope(&node, &mut scope, &Editor, &mut Fuel::new(100), PURE).unwrap(),
        PureValue::Scalar(ScalarValue::Integer(12))
    ));
    assert_eq!(scope.len(), 1);
    assert!(matches!(
        scope.get("seed"),
        Some(PureValue::Scalar(ScalarValue::Integer(5)))
    ));
    assert!(scope.get("next").is_none());
}

#[test]
fn initializer_sees_parent_and_body_sees_read_only_binding_types_at_original_ast_nodes() {
    let source = expression("bind(body: concat(left: text, right: body_text()), text: init())");
    let Expression::Call { arguments, .. } = &source else {
        panic!()
    };
    let body = &arguments[0].value;
    let Expression::Call {
        arguments: body_args,
        ..
    } = body
    else {
        panic!()
    };
    let expected = [&arguments[1].value, &body_args[1].value];
    let mut calls = 0;
    let (node, _): (Node, _) = lower_scalar_source_with_scope(
        &source,
        LIMITS,
        &[("seed", ScalarType::Integer)],
        |node, scope| -> Result<_, PrivateError> {
            assert!(std::ptr::eq(node, expected[calls]));
            assert_eq!(scope.get("seed"), Some(ScalarType::Integer));
            assert!(!scope.is_empty());
            if calls == 0 {
                assert_eq!(scope.len(), 1);
                assert_eq!(scope.get("text"), None);
            } else {
                assert_eq!(scope.len(), 2);
                assert_eq!(scope.get("text"), Some(ScalarType::String));
            }
            assert!(!format!("{scope:?}").contains("seed"));
            assert!(!format!("{scope:?}").contains("text"));
            calls += 1;
            Ok(literal(ScalarValue::String(
                if calls == 1 { "ready" } else { "!" }.into(),
            )))
        },
    )
    .unwrap();
    assert_eq!(calls, 2);
    assert!(
        matches!(evaluate(&node).unwrap(), PureValue::Scalar(ScalarValue::String(value)) if value == "ready!")
    );
}

#[test]
fn every_cold_binding_shape_shadow_and_active_quota_is_checked_before_extensions() {
    for (source, max_bindings, expected) in [
        (
            "choose(when: true, then: native(), otherwise: bind(x: 1, body: bind(x: 2, body: x)))",
            8,
            PureTypeError::ShadowedBinding,
        ),
        (
            "choose(when: true, then: native(), otherwise: bind(x: 1, body: bind(y: 2, body: y)))",
            1,
            PureTypeError::BindingLimit,
        ),
        ("bind(x: native(), body: x)", 0, PureTypeError::BindingLimit),
        (
            "bind(true: native(), body: 0)",
            8,
            PureTypeError::InvalidName,
        ),
        (
            "native(value: bind(x: 1, body: bind(x: 2, body: x)))",
            8,
            PureTypeError::ShadowedBinding,
        ),
    ] {
        let mut calls = 0;
        let result: ScalarSourceResult<Node, PrivateError> = lower_scalar_source_with_scope(
            &expression(source),
            ScalarSourceLimits {
                max_bindings,
                ..LIMITS
            },
            &[],
            |_, _| {
                calls += 1;
                Ok(literal(ScalarValue::Integer(0)))
            },
        );
        assert!(
            matches!(result, Err(ScalarSourceError::Bindings { error, .. }) if error == expected),
            "{source}"
        );
        assert_eq!(calls, 0);
    }
    for source in [
        "bind(x: native())",
        "bind(x: native(), y: 1)",
        "bind(body: native(), body: 1)",
        "bind(x: native(), body: 1, extra: 2)",
    ] {
        let source = expression(source);
        let result: ScalarSourceResult<Node, PrivateError> =
            lower_scalar_source_with_scope(&source, LIMITS, &[], |_, _| {
                panic!("bad shape reached callback")
            });
        assert!(
            matches!(result, Err(ScalarSourceError::BindingShape { span }) if span == source_span(&source))
        );
    }
    let source = expression("bind(x: 1, body: x)");
    let result: ScalarSourceResult<Node, PrivateError> = lower_scalar_source_with_scope(
        &source,
        LIMITS,
        &[("x", ScalarType::Integer)],
        |_, _| panic!(),
    );
    assert!(
        matches!(result, Err(ScalarSourceError::Bindings { span, error: PureTypeError::ShadowedBinding }) if span == binding_span(&source))
    );
}

#[test]
fn invalid_prefix_and_limit_policies_never_enter_extensions() {
    for (prefix, max_bindings) in [
        (
            vec![("a", ScalarType::Integer), ("a", ScalarType::String)],
            8,
        ),
        (vec![("body", ScalarType::Integer)], 8),
        (vec![("a", ScalarType::Integer)], 0),
        (vec![("invalid.name", ScalarType::Integer)], 8),
        (vec![("", ScalarType::Integer)], 8),
    ] {
        let result: ScalarSourceResult<Node, PrivateError> = lower_scalar_source_with_scope(
            &expression("native()"),
            ScalarSourceLimits {
                max_bindings,
                ..LIMITS
            },
            &prefix,
            |_, _| panic!(),
        );
        assert!(matches!(
            result,
            Err(ScalarSourceError::Bindings {
                error: PureTypeError::InvalidScope,
                ..
            })
        ));
    }
    for limits in [
        ScalarSourceLimits {
            max_bindings: 1_025,
            ..LIMITS
        },
        ScalarSourceLimits {
            source: SourceCallLimits {
                max_source_depth: 65,
                ..SOURCE
            },
            ..LIMITS
        },
    ] {
        let result: ScalarSourceResult<Node, PrivateError> =
            lower_scalar_source_with_scope(&expression("native()"), limits, &[], |_, _| panic!());
        assert!(matches!(
            result,
            Err(ScalarSourceError::Source(SourceCallError::InvalidLimits))
        ));
    }
    let (node, _): (Node, _) = lower_scalar_source_with_scope(
        &expression("1"),
        ScalarSourceLimits {
            max_bindings: 0,
            ..LIMITS
        },
        &[],
        |_, _| -> Result<_, PrivateError> { panic!() },
    )
    .unwrap();
    assert!(matches!(
        node,
        Node::Literal {
            value: ScalarValue::Integer(1)
        }
    ));
}

#[test]
fn binding_capacity_counts_active_frames_not_siblings_or_initializer_temporaries() {
    for source in [
        "add(left: bind(x: 1, body: x), right: bind(x: 2, body: x))",
        "bind(x: bind(x: 3, body: x), body: x)",
        "choose(when: true, then: bind(x: 3, body: x), otherwise: bind(x: 4, body: x))",
    ] {
        let (node, _): (Node, _) = lower_scalar_source_with_scope(
            &expression(source),
            ScalarSourceLimits {
                max_bindings: 1,
                ..LIMITS
            },
            &[],
            |_, _| -> Result<_, PrivateError> { panic!() },
        )
        .unwrap();
        assert!(matches!(
            evaluate(&node).unwrap(),
            PureValue::Scalar(ScalarValue::Integer(3))
        ));
    }
    let source =
        expression("add(left: bind(x: 1, body: first()), right: bind(y: 2, body: second()))");
    let mut calls = 0;
    let _: (Node, _) = lower_scalar_source_with_scope(
        &source,
        ScalarSourceLimits {
            max_bindings: 1,
            ..LIMITS
        },
        &[],
        |_, scope| -> Result<_, PrivateError> {
            assert_eq!(scope.len(), 1);
            assert_eq!(scope.get("x"), (calls == 0).then_some(ScalarType::Integer));
            assert_eq!(scope.get("y"), (calls == 1).then_some(ScalarType::Integer));
            calls += 1;
            Ok(literal(ScalarValue::Integer(0)))
        },
    )
    .unwrap();
    assert_eq!(calls, 2);
}

#[test]
fn unbound_native_aliases_remain_explicit_but_cannot_override_bound_locals() {
    let source = expression("bind(x: x, body: x)");
    let mut calls = 0;
    let (node, _): (Node, _) = lower_scalar_source_with_scope(
        &source,
        LIMITS,
        &[],
        |source, scope| -> Result<_, PrivateError> {
            assert!(matches!(source, Expression::Reference { name, .. } if name == "x"));
            assert!(scope.is_empty());
            calls += 1;
            Ok(literal(ScalarValue::Integer(9)))
        },
    )
    .unwrap();
    assert_eq!(calls, 1);
    assert!(matches!(
        evaluate(&node).unwrap(),
        PureValue::Scalar(ScalarValue::Integer(9))
    ));
    assert!(matches!(
        lower("bind(x: x, body: x)"),
        Err(ScalarSourceError::Native {
            error: PrivateError("unknown native alias"),
            ..
        })
    ));
    let _: (Node, _) = lower_scalar_source(&source, SOURCE, |node| -> Result<_, PrivateError> {
        assert!(std::ptr::eq(node, &source));
        Ok(literal(ScalarValue::Integer(3)))
    })
    .unwrap();
}

#[test]
fn bind_and_local_constructors_share_generation_budget_without_refunds() {
    let source = expression("bind(x: 1, body: add(left: x, right: x))");
    for nodes in [0, 1, 2, 3, 4] {
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
    let _: (Node, _) = lower_scalar_source_with_scope(
        &source,
        ScalarSourceLimits {
            source: SourceCallLimits {
                max_lowered_nodes: 5,
                max_lowered_depth: 2,
                ..SOURCE
            },
            ..LIMITS
        },
        &[],
        |_, _| -> Result<_, PrivateError> { panic!() },
    )
    .unwrap();
    let result: ScalarSourceResult<Node, PrivateError> = lower_scalar_source_with_scope(
        &source,
        ScalarSourceLimits {
            source: SourceCallLimits {
                max_lowered_depth: 1,
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
            error: StructureError::DepthLimit,
            ..
        })
    ));
    let source = expression("bind(x: first(), body: second())");
    let mut calls = 0;
    let result: ScalarSourceResult<Node, PrivateError> = lower_scalar_source_with_scope(
        &source,
        ScalarSourceLimits {
            source: SourceCallLimits {
                max_lowered_nodes: 2,
                ..SOURCE
            },
            ..LIMITS
        },
        &[],
        |_, _| {
            calls += 1;
            Ok(literal(ScalarValue::Integer(1)))
        },
    );
    assert!(matches!(
        result,
        Err(ScalarSourceError::Generation {
            error: StructureError::NodeLimit,
            ..
        })
    ));
    assert_eq!(calls, 1);
}

#[test]
fn shared_binding_requires_scalar_value_and_body_without_accepting_host_capture() {
    for source in ["bind(x: native(), body: 1)", "bind(x: 1, body: native())"] {
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
    }
}

#[test]
fn errors_and_unwind_release_partial_move_only_ir_once_without_polluting_next_run() {
    for unwind in [false, true] {
        let source = expression("bind(text: owned(), body: bind(n: 1, body: fail()))");
        let drops = Rc::new(Cell::new(0));
        let calls = Cell::new(0);
        let result = catch_unwind(AssertUnwindSafe(|| {
            lower_scalar_source_with_scope(
                &source,
                LIMITS,
                &[("seed", ScalarType::Boolean)],
                |_, scope| -> Result<_, PrivateError> {
                    calls.set(calls.get() + 1);
                    if calls.get() == 1 {
                        assert_eq!(scope.len(), 1);
                        Ok((
                            Node::Field {
                                value: Box::new(Node::Local {
                                    name: "reply".into(),
                                }),
                                field: Field(drops.clone()),
                            },
                            Some(ScalarType::String),
                        ))
                    } else {
                        assert_eq!(scope.get("text"), Some(ScalarType::String));
                        assert_eq!(scope.get("n"), Some(ScalarType::Integer));
                        assert_eq!(scope.len(), 3);
                        assert_eq!(format!("{scope:?}"), "ScalarSourceScope { bindings: 3 }");
                        if unwind {
                            panic!("native unwind")
                        }
                        Err(PrivateError("private failure"))
                    }
                },
            )
        }));
        assert_eq!(calls.get(), 2);
        assert_eq!(drops.get(), 1);
        if unwind {
            assert!(result.is_err());
        } else {
            let error = result.unwrap().err().unwrap();
            assert_eq!(error.to_string(), "native source extension lowering failed");
            assert!(!format!("{error:?}").contains("private failure"));
        }
        assert!(lower("bind(text: 1, body: text)").is_ok());
    }
}

#[test]
fn native_type_observations_do_not_replace_complete_cold_ir_inference() {
    let (node, ty): (Node, _) = lower_scalar_source_with_scope(
        &expression("bind(x: 1, body: choose(when: true, then: x, otherwise: native()))"),
        LIMITS,
        &[],
        |_, scope| -> Result<_, PrivateError> {
            assert_eq!(scope.get("x"), Some(ScalarType::Integer));
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
        infer_pure_type(&node, &[], &Editor, TYPING),
        Err(PureTypeError::UnknownLocal)
    );
    // Value execution alone would not inspect the cold branch, so cold typing is mandatory.
    assert!(matches!(
        evaluate(&node).unwrap(),
        PureValue::Scalar(ScalarValue::Integer(1))
    ));
}

struct TextDomain;
impl ScalarArgumentDomain for TextDomain {
    fn scalar_types(&self) -> ScalarTypeSet {
        ScalarTypeSet::only(ScalarType::String)
    }
    fn accepts_literal(&self, value: &ScalarValue) -> bool {
        matches!(value, ScalarValue::String(text) if text.len() <= 64)
    }
}

#[test]
fn parsed_native_call_prepares_bound_operand_against_original_catalog_and_prefix() {
    let source =
        expression("panel.set(text: bind(text: concat(left: seed, right: \"!\"), body: text))");
    let parameters = [NamedParameter::required("text", TextDomain)];
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
                &[("seed", ScalarType::String)],
                |_, _| -> Result<_, PrivateError> { panic!() },
            )?;
            assert_eq!(
                infer_pure_type(
                    &node,
                    &[("seed", PureType::Scalar(ScalarType::String))],
                    &Editor,
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
    let mut bindings = vec![(
        "seed",
        PureValue::Scalar(ScalarValue::String("ready".into())),
    )];
    let mut scope = ScopeFrame::new(&mut bindings);
    let prepared = evaluate_call_arguments_in_scope(
        &arguments,
        &schemas[0],
        &mut scope,
        &Editor,
        &mut Fuel::new(100),
        CallEvaluationLimits {
            pure: PURE,
            max_arguments: 1,
        },
    )
    .unwrap();
    assert!(std::ptr::eq(prepared.schema(), &schemas[0]));
    assert_eq!(
        prepared.arguments()[0].value,
        ScalarValue::String("ready!".into())
    );
    assert_eq!(scope.len(), 1);
    assert!(scope.get("text").is_none());
}

#[test]
fn reference_binding_diagnostics_and_host_group_function_capture_are_preserved() {
    for (source, code, message, at_binding) in [
        (
            "bind(x: missing)",
            "LSH1401",
            "bind requires one named value and 'body'",
            false,
        ),
        (
            "bind(true: missing, body: 1)",
            "LSH1403",
            "local name must be bounded and cannot shadow an active binding",
            true,
        ),
        (
            "bind(x: x, body: x)",
            "LSH1403",
            "undefined local 'x'",
            false,
        ),
    ] {
        let parsed = parse(&format!("fn main() = {source}"));
        let body = &parsed.function.as_ref().unwrap().body;
        let errors = leselang_hir::lower(&parsed).unwrap_err();
        assert_eq!(errors[0].code, code);
        assert_eq!(errors[0].message, message);
        let span = if at_binding {
            binding_span(body)
        } else if source.contains("x: x") {
            let Expression::Call { arguments, .. } = body else {
                panic!()
            };
            source_span(&arguments[0].value)
        } else {
            source_span(body)
        };
        assert_eq!(errors[0].span, Some(span));
    }
    for source in [
        "fn main() = bind(r: runtime.list(), body: field(value: r, name: \"count\"))",
        "fn main() = bind(r: choose(when: true, then: runtime.list(), otherwise: runtime.list()), body: 1)",
        "fn main() = bind(g: seq(a: ui.focus(node_id: \"a\")), body: field(value: member(value: g, name: \"a\"), name: \"node_id\"))",
        "fn read() = bind(r: ui.focus(node_id: \"a\"), body: field(value: r, name: \"node_id\"))\nfn main() = bind(answer: read(), body: answer)",
    ] {
        let program = leselang_hir::lower(&parse(source)).unwrap();
        let canonical = leselang_hir::canonical_source(&program.function.effect).unwrap();
        assert_eq!(leselang_hir::lower(&parse(&canonical)).unwrap(), program);
    }
    for source in [
        "bind(x: 1, body: bind(x: missing, body: x))",
        "bind(r: seq(a: runtime.list()), body: field(value: r, name: \"count\"))",
    ] {
        assert!(leselang_hir::lower(&parse(&format!("fn main() = {source}"))).is_err());
    }
}
