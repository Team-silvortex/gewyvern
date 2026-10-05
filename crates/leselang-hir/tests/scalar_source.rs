use std::cell::Cell;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;

use leselang_hir::ir::Computation;
use leselang_hir::pure_evaluation::{
    PureEvaluationEnvironment, PureEvaluationLimits, PureValue, evaluate_pure_in_scope,
};
use leselang_hir::pure_typing::{
    PureType, PureTypeEnvironment, PureTypeError, TypeInferenceLimits, infer_pure_type,
};
use leselang_hir::scalar_source::{ScalarSourceError, ScalarSourceResult, lower_scalar_source};
use leselang_hir::source_call::{SourceCallError, SourceCallLimits, SourceShapeError};
use leselang_runtime_core::*;
use leselang_syntax::{Expression, Span, parse};

// All native slots/errors deliberately lack Clone, Debug, serde and Send.
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
const LIMITS: SourceCallLimits = SourceCallLimits {
    max_source_nodes: 256,
    max_source_depth: 32,
    max_lowered_nodes: 256,
    max_lowered_depth: 32,
    max_arguments: 8,
};
fn expression(source: &str) -> Expression {
    let parsed = parse(&format!("fn main() = {source}"));
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    parsed.function.unwrap().body
}
fn lower(source: &str) -> ScalarSourceResult<Node, PrivateError> {
    lower_scalar_source(&expression(source), LIMITS, |_| {
        Err(PrivateError("private missing extension"))
    })
}
fn field(drops: &Rc<Cell<usize>>) -> Node {
    Node::Field {
        value: Box::new(Node::Local {
            name: "reply".into(),
        }),
        field: Field(drops.clone()),
    }
}

#[test]
fn all_literals_and_nested_operators_need_no_native_extension() {
    for (source, expected) in [
        ("7", ScalarValue::Integer(7)),
        ("true", ScalarValue::Boolean(true)),
        ("\"hello\"", ScalarValue::String("hello".into())),
        ("none", ScalarValue::None),
    ] {
        let (value, ty) = lower(source).unwrap();
        assert_eq!(ty, expected.scalar_type());
        assert!(matches!(value, Node::Literal { value } if value == expected));
    }
    let source = expression("add(right: 2, left: len(value: \"abc\"))");
    let (value, ty): (Node, _) =
        lower_scalar_source(&source, LIMITS, |_| -> Result<_, PrivateError> {
            panic!("pure builtins must not invoke extensions")
        })
        .unwrap();
    assert_eq!(ty, ScalarType::Integer);
    let Node::Binary {
        operator,
        left,
        right,
    } = value
    else {
        panic!()
    };
    assert_eq!(operator, BinaryOperator::Add);
    assert!(matches!(
        *left,
        Node::Unary {
            operator: UnaryOperator::Len,
            ..
        }
    ));
    assert!(matches!(
        *right,
        Node::Literal {
            value: ScalarValue::Integer(2)
        }
    ));
}

#[test]
fn every_closed_operator_uses_the_shared_signature_without_calculation() {
    let cases = [
        ("add", "4", "2", ScalarType::Integer),
        ("sub", "4", "2", ScalarType::Integer),
        ("mul", "4", "2", ScalarType::Integer),
        ("div", "1", "0", ScalarType::Integer),
        ("rem", "1", "0", ScalarType::Integer),
        ("eq", "none", "none", ScalarType::Boolean),
        ("ne", "4", "2", ScalarType::Boolean),
        ("lt", "4", "2", ScalarType::Boolean),
        ("le", "4", "2", ScalarType::Boolean),
        ("gt", "4", "2", ScalarType::Boolean),
        ("ge", "4", "2", ScalarType::Boolean),
        ("and", "false", "true", ScalarType::Boolean),
        ("or", "true", "false", ScalarType::Boolean),
        ("concat", "\"a\"", "\"b\"", ScalarType::String),
        (
            "value_or",
            "optional_string(value: none)",
            "\"b\"",
            ScalarType::String,
        ),
        ("contains", "\"a\"", "\"b\"", ScalarType::Boolean),
        ("starts_with", "\"a\"", "\"b\"", ScalarType::Boolean),
        ("ends_with", "\"a\"", "\"b\"", ScalarType::Boolean),
        ("char_at", "\"a\"", "1", ScalarType::OptionalString),
        ("split", "\"a\"", "\",\"", ScalarType::StringList),
        (
            "join",
            "split(left: \"a\", right: \",\")",
            "\",\"",
            ScalarType::String,
        ),
        (
            "append",
            "split(left: \"a\", right: \",\")",
            "\"b\"",
            ScalarType::StringList,
        ),
        (
            "item_at",
            "split(left: \"a\", right: \",\")",
            "0",
            ScalarType::OptionalString,
        ),
    ];
    assert_eq!(cases.len(), 23);
    for (name, left, right, expected) in cases {
        let (value, ty) = lower(&format!("{name}(right: {right}, left: {left})")).unwrap();
        assert_eq!(ty, expected, "{name}");
        let Node::Binary { operator, .. } = value else {
            panic!()
        };
        assert_eq!(operator, BinaryOperator::parse(name).unwrap());
    }
    for (name, input, expected) in [
        ("not", "false", ScalarType::Boolean),
        ("len", "\"abc\"", ScalarType::Integer),
        ("to_string", "7", ScalarType::String),
        ("parse_integer", "\"not a number\"", ScalarType::Integer),
        ("parse_boolean", "\"not a bool\"", ScalarType::Boolean),
        ("optional_string", "none", ScalarType::OptionalString),
        (
            "has_value",
            "optional_string(value: none)",
            ScalarType::Boolean,
        ),
    ] {
        let (_, ty) = lower(&format!("{name}(value: {input})")).unwrap();
        assert_eq!(ty, expected, "{name}");
    }
}

#[test]
fn both_short_circuit_operands_are_lowered_and_typed_in_signature_order() {
    let source = expression("and(right: second, left: first)");
    let Expression::Call { arguments, .. } = &source else {
        panic!()
    };
    let mut seen = Vec::new();
    let (value, ty): (Node, _) =
        lower_scalar_source(&source, LIMITS, |node| -> Result<_, PrivateError> {
            seen.push(node);
            Ok((
                Node::Literal {
                    value: ScalarValue::Boolean(false),
                },
                Some(ScalarType::Boolean),
            ))
        })
        .unwrap();
    assert_eq!(ty, ScalarType::Boolean);
    assert!(std::ptr::eq(seen[0], &arguments[1].value));
    assert!(std::ptr::eq(seen[1], &arguments[0].value));
    assert!(matches!(
        value,
        Node::Binary {
            operator: BinaryOperator::And,
            ..
        }
    ));
    assert!(matches!(
        lower("and(left: false, right: 3)"),
        Err(ScalarSourceError::Binary { .. })
    ));
    assert!(matches!(
        lower("not(value: 3)"),
        Err(ScalarSourceError::Unary { .. })
    ));
}

#[test]
fn all_cold_operator_names_and_physical_source_checks_precede_extensions() {
    for source in [
        "and(left: first, right: not(wrong: false))",
        "custom(value: add(left: first, left: second))",
        "add(left: first, right: 2, extra: 3)",
    ] {
        let result: ScalarSourceResult<Node, PrivateError> =
            lower_scalar_source(&expression(source), LIMITS, |_| {
                panic!("malformed cold operator must precede native code")
            });
        assert!(matches!(result, Err(ScalarSourceError::Names { .. })));
    }
    let source = expression("add(left: first, right: 2)");
    for limits in [
        SourceCallLimits {
            max_source_nodes: 2,
            ..LIMITS
        },
        SourceCallLimits {
            max_source_depth: 0,
            ..LIMITS
        },
        SourceCallLimits {
            max_arguments: 1,
            ..LIMITS
        },
        SourceCallLimits {
            max_source_depth: 65,
            ..LIMITS
        },
    ] {
        let result: ScalarSourceResult<Node, PrivateError> =
            lower_scalar_source(&source, limits, |_| {
                panic!("physical source limits must precede native code")
            });
        assert!(matches!(result, Err(ScalarSourceError::Source(_))));
    }
    let source = Expression::String {
        value: "x".repeat(MAX_SCALAR_STRING_BYTES + 1),
        span: Span { start: 0, end: 1 },
    };
    let result: ScalarSourceResult<Node, PrivateError> =
        lower_scalar_source(&source, LIMITS, |_| panic!());
    assert!(matches!(
        result,
        Err(ScalarSourceError::Source(SourceCallError::Shape {
            error: SourceShapeError::UnboundedText,
            ..
        }))
    ));
}

#[test]
fn optional_literal_buffer_moves_once_and_fold_cost_is_not_refunded() {
    let source = expression("optional_string(value: caption)");
    let text = String::from("private owned native caption");
    let pointer = text.as_ptr();
    let mut text = Some(text);
    let (value, ty): (Node, _) = lower_scalar_source(
        &source,
        SourceCallLimits {
            max_lowered_nodes: 2,
            ..LIMITS
        },
        |_| -> Result<_, PrivateError> {
            Ok((
                Node::Literal {
                    value: ScalarValue::String(text.take().unwrap()),
                },
                Some(ScalarType::String),
            ))
        },
    )
    .unwrap();
    assert_eq!(ty, ScalarType::OptionalString);
    let Node::Literal {
        value: ScalarValue::OptionalString(OptionalStringValue(Some(text))),
    } = value
    else {
        panic!()
    };
    assert_eq!(text.as_ptr(), pointer);
    for limits in [
        SourceCallLimits {
            max_lowered_nodes: 1,
            ..LIMITS
        },
        SourceCallLimits {
            max_lowered_depth: 0,
            ..LIMITS
        },
    ] {
        let result: ScalarSourceResult<Node, PrivateError> =
            lower_scalar_source(&source, limits, |_| {
                panic!("minimum constructor budget must precede native lowering")
            });
        assert!(matches!(result, Err(ScalarSourceError::Generation { .. })));
    }
    let source = expression("value_or(left: optional_string(value: none), right: \"fallback\")");
    let result: ScalarSourceResult<Node, PrivateError> = lower_scalar_source(
        &source,
        SourceCallLimits {
            max_lowered_nodes: 3,
            ..LIMITS
        },
        |_| panic!(),
    );
    assert!(matches!(
        result,
        Err(ScalarSourceError::Generation {
            error: StructureError::NodeLimit,
            ..
        })
    ));
}

#[test]
fn aggregate_native_expansion_is_bounded_and_partial_nodes_are_dropped() {
    let source = expression("add(left: first, right: second)");
    let drops = Rc::new(Cell::new(0));
    let calls = Cell::new(0);
    let result = lower_scalar_source(
        &source,
        SourceCallLimits {
            max_lowered_nodes: 3,
            ..LIMITS
        },
        |_| -> Result<_, PrivateError> {
            calls.set(calls.get() + 1);
            Ok((field(&drops), Some(ScalarType::Integer)))
        },
    );
    assert!(matches!(
        result,
        Err(ScalarSourceError::Generation {
            error: StructureError::NodeLimit,
            ..
        })
    ));
    assert_eq!(calls.get(), 1);
    assert_eq!(drops.get(), 1);
}

#[test]
fn malformed_impure_unbounded_and_inconsistent_native_outputs_are_rejected() {
    let source = expression("caption");
    for (output, ty, expected) in [
        (
            Node::Host {
                effect: Box::new(Effect),
            },
            Some(ScalarType::String),
            PureTypeError::Impure,
        ),
        (
            Node::Local {
                name: "bad name".into(),
            },
            Some(ScalarType::String),
            PureTypeError::InvalidName,
        ),
        (
            Node::Literal {
                value: ScalarValue::String("x".repeat(MAX_SCALAR_STRING_BYTES + 1)),
            },
            Some(ScalarType::String),
            PureTypeError::UnboundedLiteral,
        ),
    ] {
        let mut output = Some(output);
        let result = lower_scalar_source(&source, LIMITS, |_| -> Result<_, PrivateError> {
            Ok((output.take().unwrap(), ty))
        });
        assert!(
            matches!(result, Err(ScalarSourceError::Produced { error, .. }) if error == expected)
        );
    }
    let result: ScalarSourceResult<Node, PrivateError> =
        lower_scalar_source(&source, LIMITS, |_| {
            Ok((
                Node::Literal {
                    value: ScalarValue::Integer(7),
                },
                Some(ScalarType::String),
            ))
        });
    assert!(matches!(
        result,
        Err(ScalarSourceError::InconsistentLiteral { .. })
    ));
    let result: ScalarSourceResult<Node, PrivateError> =
        lower_scalar_source(&source, LIMITS, |_| {
            Ok((
                Node::Local {
                    name: "reply".into(),
                },
                None,
            ))
        });
    assert!(matches!(result, Err(ScalarSourceError::NonScalar { .. })));
}

#[test]
fn native_error_and_unwind_release_exact_partial_ir_without_formatting_payloads() {
    let source = expression("add(left: first, right: second)");
    let drops = Rc::new(Cell::new(0));
    let mut calls = 0;
    let error = lower_scalar_source(&source, LIMITS, |_| {
        calls += 1;
        if calls == 1 {
            Ok((field(&drops), Some(ScalarType::Integer)))
        } else {
            Err(PrivateError("private native payload"))
        }
    })
    .err()
    .unwrap();
    assert_eq!(drops.get(), 1);
    assert_eq!(calls, 2);
    assert!(!format!("{error:?} {error}").contains("private"));
    assert!(std::error::Error::source(&error).is_none());
    assert!(matches!(
        error,
        ScalarSourceError::Native {
            error: PrivateError("private native payload"),
            ..
        }
    ));
    calls = 0;
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let _: ScalarSourceResult<Node, PrivateError> =
                lower_scalar_source(&source, LIMITS, |_| {
                    calls += 1;
                    if calls == 1 {
                        Ok((field(&drops), Some(ScalarType::Integer)))
                    } else {
                        panic!("native extension unwind")
                    }
                });
        }))
        .is_err()
    );
    assert_eq!(drops.get(), 2);
    assert_eq!(calls, 2);
}

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
    fn field(&self, result: &Rc<String>, _: &Field) -> Result<ScalarValue, PrivateError> {
        Ok(ScalarValue::String(result.as_str().to_owned()))
    }
    fn member(&self, _: &Rc<String>, _: &str, _: &Operation) -> Result<Rc<String>, PrivateError> {
        Err(PrivateError("private unknown member"))
    }
}

#[test]
fn gui_local_native_field_source_can_be_cold_typed_and_evaluated_without_product_vm() {
    let drops = Rc::new(Cell::new(0));
    let source = expression("concat(left: caption, right: \"!\")");
    let (node, ty) = lower_scalar_source(&source, LIMITS, |source| -> Result<_, PrivateError> {
        assert!(matches!(source, Expression::Reference { name, .. } if name == "caption"));
        Ok((field(&drops), Some(ScalarType::String)))
    })
    .unwrap();
    assert_eq!(ty, ScalarType::String);
    assert_eq!(
        infer_pure_type(
            &node,
            &[("reply", PureType::Result(()))],
            &Editor,
            TypeInferenceLimits {
                max_nodes: 256,
                max_depth: 32,
                max_bindings: 8
            }
        ),
        Ok(PureType::Scalar(ty))
    );
    let reply = Rc::new(String::from("ready"));
    let mut bindings = vec![("reply", PureValue::Result(reply.clone()))];
    let mut scope = ScopeFrame::new(&mut bindings);
    let value = evaluate_pure_in_scope(
        &node,
        &mut scope,
        &Editor,
        &mut Fuel::new(100),
        PureEvaluationLimits {
            max_nodes: 256,
            max_depth: 32,
            max_bindings: 8,
        },
    )
    .unwrap();
    assert!(matches!(value, PureValue::Scalar(ScalarValue::String(text)) if text == "ready!"));
    assert_eq!(drops.get(), 0);
    drop(scope);
    drop(bindings);
    drop(node);
    assert_eq!(drops.get(), 1);
}

#[test]
fn extension_observations_do_not_replace_complete_lexical_cold_type_inference() {
    let source = expression("and(left: false, right: missing)");
    let (node, _): (Node, _) =
        lower_scalar_source(&source, LIMITS, |_| -> Result<_, PrivateError> {
            Ok((
                Node::Local {
                    name: "missing".into(),
                },
                Some(ScalarType::Boolean),
            ))
        })
        .unwrap();
    assert_eq!(
        infer_pure_type(
            &node,
            &[],
            &Editor,
            TypeInferenceLimits {
                max_nodes: 256,
                max_depth: 32,
                max_bindings: 8
            }
        ),
        Err(PureTypeError::UnknownLocal)
    );
}

fn scalar_samples() -> [ScalarValue; 6] {
    [
        ScalarValue::Integer(2),
        ScalarValue::Boolean(true),
        ScalarValue::String("2".into()),
        ScalarValue::None,
        ScalarValue::OptionalString(OptionalStringValue(Some("2".into()))),
        ScalarValue::StringList(StringListValue(vec!["2".into()])),
    ]
}

#[test]
fn all_scalar_operand_type_combinations_match_core_signatures() {
    for name in [
        "add",
        "sub",
        "mul",
        "div",
        "rem",
        "eq",
        "ne",
        "lt",
        "le",
        "gt",
        "ge",
        "and",
        "or",
        "concat",
        "value_or",
        "contains",
        "starts_with",
        "ends_with",
        "char_at",
        "split",
        "join",
        "append",
        "item_at",
    ] {
        let operator = BinaryOperator::parse(name).unwrap();
        let source = expression(&format!("{name}(left: first, right: second)"));
        for left in scalar_samples() {
            for right in scalar_samples() {
                let expected = operator.result_type(left.scalar_type(), right.scalar_type());
                let mut values = [left.clone(), right].into_iter();
                let output: ScalarSourceResult<Node, PrivateError> =
                    lower_scalar_source(&source, LIMITS, |_| {
                        let value = values.next().unwrap();
                        let ty = value.scalar_type();
                        Ok((Node::Literal { value }, Some(ty)))
                    });
                assert_eq!(output.map(|(_, ty)| ty).ok(), expected, "{name}");
            }
        }
    }
    for name in [
        "not",
        "len",
        "to_string",
        "parse_integer",
        "parse_boolean",
        "optional_string",
        "has_value",
    ] {
        let operator = UnaryOperator::parse(name).unwrap();
        let source = expression(&format!("{name}(value: input)"));
        for value in scalar_samples() {
            let ty = value.scalar_type();
            let mut value = Some(value);
            let output: ScalarSourceResult<Node, PrivateError> =
                lower_scalar_source(&source, LIMITS, |_| {
                    Ok((
                        Node::Literal {
                            value: value.take().unwrap(),
                        },
                        Some(ty),
                    ))
                });
            assert_eq!(
                output.map(|(_, ty)| ty).ok(),
                operator.result_type(ty),
                "{name}"
            );
        }
    }
}

#[test]
fn native_expansion_depth_rejection_drops_output_without_evaluation() {
    let source = expression("caption");
    let drops = Rc::new(Cell::new(0));
    let result = lower_scalar_source(
        &source,
        SourceCallLimits {
            max_lowered_depth: 0,
            ..LIMITS
        },
        |_| -> Result<_, PrivateError> { Ok((field(&drops), Some(ScalarType::String))) },
    );
    assert!(matches!(
        result,
        Err(ScalarSourceError::Produced {
            error: PureTypeError::Structure(StructureError::DepthLimit),
            ..
        })
    ));
    assert_eq!(drops.get(), 1);
}

#[test]
fn reference_adapter_keeps_exact_operator_diagnostics_spans_and_child_precedence() {
    for (source, code, message) in [
        (
            "add(left: 1)",
            "LSH1401",
            "expected exactly the named arguments: left, right",
        ),
        (
            "not(wrong: false)",
            "LSH1401",
            "expected exactly the named arguments: value",
        ),
        (
            "add(left: true, right: 1)",
            "LSH1402",
            "add does not accept Boolean and Integer",
        ),
        ("not(value: 1)", "LSH1402", "not requires a pure boolean"),
    ] {
        let parsed = parse(&format!("fn main() = {source}"));
        let Expression::Call { span, .. } = &parsed.function.as_ref().unwrap().body else {
            panic!()
        };
        let errors = leselang_hir::lower(&parsed).unwrap_err();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, code);
        assert_eq!(errors[0].message, message);
        assert_eq!(errors[0].span, Some(*span));
    }
    let errors = leselang_hir::lower(&parse(
        "fn main() = add(left: runtime.list(), right: missing)",
    ))
    .unwrap_err();
    assert_eq!(errors[0].code, "LSH1403");
    assert!(errors[0].message.contains("missing"));
}
