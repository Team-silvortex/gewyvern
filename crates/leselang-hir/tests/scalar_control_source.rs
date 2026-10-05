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
use leselang_hir::scalar_source::{ScalarSourceError, ScalarSourceResult, lower_scalar_source};
use leselang_hir::source_call::{
    SourceCallHost, SourceCallLimits, SourceSchema, lower_source_call,
};
use leselang_runtime_core::*;
use leselang_syntax::{Expression, parse};

// Native slots remain move-only and GUI-local, without Debug or wire requirements.
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
    max_arguments: 64,
};
const PURE: PureEvaluationLimits = PureEvaluationLimits {
    max_nodes: 256,
    max_depth: 32,
    max_bindings: 8,
};
const TYPING: TypeInferenceLimits = TypeInferenceLimits {
    max_nodes: 256,
    max_depth: 32,
    max_bindings: 8,
};
fn expression(source: &str) -> Expression {
    let parsed = parse(&format!("fn main() = {source}"));
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    parsed.function.unwrap().body
}
fn literal(value: ScalarValue) -> (Node, Option<ScalarType>) {
    let ty = value.scalar_type();
    (Node::Literal { value }, Some(ty))
}
fn field(drops: &Rc<Cell<usize>>) -> Node {
    Node::Field {
        value: Box::new(Node::Local {
            name: "reply".into(),
        }),
        field: Field(drops.clone()),
    }
}
fn lower(source: &str) -> ScalarSourceResult<Node, PrivateError> {
    lower_scalar_source(&expression(source), LIMITS, |_| {
        Err(PrivateError("private unknown form"))
    })
}
struct Editor {
    queries: Cell<usize>,
    fail: bool,
}
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
        self.queries.set(self.queries.get() + 1);
        if self.fail {
            Err(PrivateError("private field failure"))
        } else {
            Ok(ScalarValue::String(result.as_str().to_owned()))
        }
    }
    fn member(&self, _: &Rc<String>, _: &str, _: &Operation) -> Result<Rc<String>, PrivateError> {
        Err(PrivateError("private member failure"))
    }
}
fn editor() -> Editor {
    Editor {
        queries: Cell::new(0),
        fail: false,
    }
}
fn checked(node: &Node, host: &Editor) -> Result<PureType<()>, PureTypeError> {
    infer_pure_type(node, &[("reply", PureType::Result(()))], host, TYPING)
}
fn evaluate(
    node: &Node,
    host: &Editor,
) -> Result<PureValue<Rc<String>>, PureEvaluationFailure<PrivateError>> {
    let mut bindings = vec![("reply", PureValue::Result(Rc::new("ready".into())))];
    let mut scope = ScopeFrame::new(&mut bindings);
    evaluate_pure_in_scope(node, &mut scope, host, &mut Fuel::new(100), PURE)
}

#[test]
fn parsed_control_with_native_field_runs_through_shared_typing_and_execution() {
    let source = expression(
        "join(left: choose(when: true, then: strings(a: concat(left: caption, right: \"!\"), b: recover(value: to_string(value: parse_integer(value: \"bad\")), fallback: \"safe\")), otherwise: strings()), right: \",\")",
    );
    let drops = Rc::new(Cell::new(0));
    let (node, ty) = lower_scalar_source(&source, LIMITS, |source| -> Result<_, PrivateError> {
        assert!(matches!(source, Expression::Reference { name, .. } if name == "caption"));
        Ok((field(&drops), Some(ScalarType::String)))
    })
    .unwrap();
    assert_eq!(ty, ScalarType::String);
    let host = editor();
    assert_eq!(checked(&node, &host), Ok(PureType::Scalar(ty)));
    assert_eq!(host.queries.get(), 0);
    assert!(
        matches!(evaluate(&node, &host).unwrap(), PureValue::Scalar(ScalarValue::String(value)) if value == "ready!,safe")
    );
    assert_eq!(host.queries.get(), 1);
    drop(node);
    assert_eq!(drops.get(), 1);
}

#[test]
fn source_control_lowers_all_cold_children_but_execution_stays_lazy() {
    let source = expression("choose(otherwise: caption, then: \"selected\", when: true)");
    let drops = Rc::new(Cell::new(0));
    let calls = Cell::new(0);
    let (node, _): (Node, _) =
        lower_scalar_source(&source, LIMITS, |_| -> Result<_, PrivateError> {
            calls.set(calls.get() + 1);
            Ok((field(&drops), Some(ScalarType::String)))
        })
        .unwrap();
    assert_eq!(calls.get(), 1);
    let host = Editor {
        fail: true,
        ..editor()
    };
    assert_eq!(
        checked(&node, &host),
        Ok(PureType::Scalar(ScalarType::String))
    );
    assert!(
        matches!(evaluate(&node, &host).unwrap(), PureValue::Scalar(ScalarValue::String(value)) if value == "selected")
    );
    assert_eq!(host.queries.get(), 0);
    assert!(matches!(
        lower("choose(when: true, then: 1, otherwise: false)"),
        Err(ScalarSourceError::BranchTypes { .. })
    ));
    assert!(matches!(
        lower("choose(when: 1, then: 1, otherwise: 1)"),
        Err(ScalarSourceError::Condition { .. })
    ));
    assert!(matches!(
        lower("recover(value: 1, fallback: false)"),
        Err(ScalarSourceError::RecoveryTypes { .. })
    ));
}

#[test]
fn native_control_arguments_keep_original_ast_identity_and_signature_order() {
    for (source, order, ty) in [
        (
            "choose(otherwise: third, then: second, when: first)",
            vec![2, 1, 0],
            ScalarType::String,
        ),
        (
            "recover(fallback: second, value: first)",
            vec![1, 0],
            ScalarType::String,
        ),
        (
            "strings(z: first, a: second)",
            vec![0, 1],
            ScalarType::StringList,
        ),
    ] {
        let source = expression(source);
        let Expression::Call {
            arguments, callee, ..
        } = &source
        else {
            panic!()
        };
        let mut seen = Vec::new();
        let (_, actual): (Node, _) =
            lower_scalar_source(&source, LIMITS, |node| -> Result<_, PrivateError> {
                let boolean = callee == "choose" && seen.is_empty();
                seen.push(node);
                Ok(literal(if boolean {
                    ScalarValue::Boolean(true)
                } else {
                    ScalarValue::String("text".into())
                }))
            })
            .unwrap();
        assert_eq!(actual, ty);
        for (node, index) in seen.into_iter().zip(order) {
            assert!(std::ptr::eq(node, &arguments[index].value));
        }
    }
}

#[test]
fn every_reserved_control_signature_and_list_label_is_checked_before_extensions() {
    for source in [
        "choose(when: flag, then: first)",
        "choose(when: flag, then: first, then: second)",
        "recover(value: first, wrong: second)",
        "custom(value: choose(when: flag, then: first))",
        "choose(when: flag, then: first, otherwise: strings(a: second, a: third))",
        "strings(a: first, body: second)",
    ] {
        let result: ScalarSourceResult<Node, PrivateError> =
            lower_scalar_source(&expression(source), LIMITS, |_| {
                panic!("all cold names must precede native extensions")
            });
        assert!(matches!(
            result,
            Err(ScalarSourceError::Names { .. } | ScalarSourceError::StringLabels { .. })
        ));
    }
}

#[test]
fn optional_list_folding_moves_exact_owned_buffers_and_preserves_source_order() {
    let source = expression("strings(z: first, a: second)");
    let values = [
        String::from("first owned caption"),
        String::from("second owned caption"),
    ];
    let pointers = values.each_ref().map(|value| value.as_ptr());
    let mut values = values.into_iter();
    let (node, ty): (Node, _) = lower_scalar_source(
        &source,
        SourceCallLimits {
            max_lowered_nodes: 3,
            ..LIMITS
        },
        |_| -> Result<_, PrivateError> { Ok(literal(ScalarValue::String(values.next().unwrap()))) },
    )
    .unwrap();
    assert_eq!(ty, ScalarType::StringList);
    let Node::Literal {
        value: ScalarValue::StringList(StringListValue(values)),
    } = node
    else {
        panic!()
    };
    assert_eq!(
        values
            .iter()
            .map(|value| value.as_ptr())
            .collect::<Vec<_>>(),
        pointers
    );
    assert_eq!(values, ["first owned caption", "second owned caption"]);
    let (empty, ty) = lower("strings()").unwrap();
    assert_eq!(ty, ScalarType::StringList);
    assert!(
        matches!(empty, Node::Literal { value: ScalarValue::StringList(value) } if value.0.is_empty())
    );
}

#[test]
fn mixed_list_keeps_literal_buffer_and_native_node_without_speculative_copies() {
    let source = expression("strings(first: first, second: caption)");
    let text = String::from("owned prefix caption");
    let pointer = text.as_ptr();
    let mut text = Some(text);
    let drops = Rc::new(Cell::new(0));
    let (node, _): (Node, _) =
        lower_scalar_source(&source, LIMITS, |_| -> Result<_, PrivateError> {
            if let Some(text) = text.take() {
                Ok(literal(ScalarValue::String(text)))
            } else {
                Ok((field(&drops), Some(ScalarType::String)))
            }
        })
        .unwrap();
    let Node::Strings { items } = &node else {
        panic!()
    };
    assert!(
        matches!(&items[0], Node::Literal { value: ScalarValue::String(value) } if value.as_ptr() == pointer)
    );
    assert!(matches!(&items[1], Node::Field { .. }));
    drop(node);
    assert_eq!(drops.get(), 1);
}

#[test]
fn list_bytes_count_and_constructor_budgets_do_not_truncate_or_refund() {
    let source = expression("strings(first: first, second: second)");
    let calls = Cell::new(0);
    let result: ScalarSourceResult<Node, PrivateError> =
        lower_scalar_source(&source, LIMITS, |_| {
            calls.set(calls.get() + 1);
            Ok(literal(ScalarValue::String(
                "x".repeat(2048 + usize::from(calls.get() == 2)),
            )))
        });
    assert!(matches!(result, Err(ScalarSourceError::StringBytes { .. })));
    assert_eq!(calls.get(), 2);
    let source =
        expression("choose(when: true, then: strings(a: \"a\", b: \"b\"), otherwise: strings())");
    let result: ScalarSourceResult<Node, PrivateError> = lower_scalar_source(
        &source,
        SourceCallLimits {
            max_lowered_nodes: 5,
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
    let entries = (0..64)
        .map(|index| format!("item_{index}: caption"))
        .collect::<Vec<_>>()
        .join(", ");
    let source = expression(&format!("strings({entries})"));
    calls.set(0);
    let result: ScalarSourceResult<Node, PrivateError> = lower_scalar_source(
        &source,
        SourceCallLimits {
            max_lowered_nodes: 64,
            ..LIMITS
        },
        |_| {
            calls.set(calls.get() + 1);
            Ok(literal(ScalarValue::String("".into())))
        },
    );
    assert!(matches!(result, Err(ScalarSourceError::Generation { .. })));
    assert_eq!(calls.get(), 63);
    assert!(matches!(
        lower("strings(value: 1)"),
        Err(ScalarSourceError::StringItem { .. })
    ));
}

#[test]
fn control_depth_and_impure_native_output_fail_without_unsafe_execution() {
    let source = expression("choose(when: flag, then: \"a\", otherwise: \"b\")");
    let result: ScalarSourceResult<Node, PrivateError> = lower_scalar_source(
        &source,
        SourceCallLimits {
            max_lowered_depth: 0,
            ..LIMITS
        },
        |_| panic!(),
    );
    assert!(matches!(
        result,
        Err(ScalarSourceError::Generation {
            error: StructureError::DepthLimit,
            ..
        })
    ));
    let source = expression("choose(when: true, then: \"safe\", otherwise: forbidden)");
    let result: ScalarSourceResult<Node, PrivateError> =
        lower_scalar_source(&source, LIMITS, |_| {
            Ok((
                Node::Host {
                    effect: Box::new(Effect),
                },
                Some(ScalarType::String),
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

#[test]
fn source_recovery_does_not_swallow_native_lowering_or_evaluation_failures() {
    let source = expression("recover(value: caption, fallback: \"safe\")");
    let error: ScalarSourceError<PrivateError> =
        lower_scalar_source::<Field, Operation, Effect, ResultTag, _>(&source, LIMITS, |_| {
            Err(PrivateError("private lowering failure"))
        })
        .err()
        .unwrap();
    assert!(!format!("{error:?} {error}").contains("private"));
    assert!(std::error::Error::source(&error).is_none());
    assert!(matches!(
        error,
        ScalarSourceError::Native {
            error: PrivateError("private lowering failure"),
            ..
        }
    ));
    let drops = Rc::new(Cell::new(0));
    let (node, _): (Node, _) =
        lower_scalar_source(&source, LIMITS, |_| -> Result<_, PrivateError> {
            Ok((field(&drops), Some(ScalarType::String)))
        })
        .unwrap();
    let host = Editor {
        fail: true,
        ..editor()
    };
    assert!(matches!(
        evaluate(&node, &host),
        Err(CalculationFailure::External(PureEvaluationFault::Native(
            PrivateError("private field failure")
        )))
    ));
    assert_eq!(host.queries.get(), 1);
    let (node, _) = lower("recover(value: parse_integer(value: \"bad\"), fallback: 7)").unwrap();
    assert!(matches!(
        evaluate(&node, &editor()).unwrap(),
        PureValue::Scalar(ScalarValue::Integer(7))
    ));
}

#[test]
fn control_failure_and_unwind_drop_partial_native_branches_exactly_once() {
    for source in [
        "choose(when: true, then: first, otherwise: second)",
        "recover(value: first, fallback: second)",
        "strings(first: first, second: second)",
    ] {
        let source = expression(source);
        let drops = Rc::new(Cell::new(0));
        let mut calls = 0;
        let result = lower_scalar_source(&source, LIMITS, |_| {
            calls += 1;
            if calls == 1 {
                Ok((field(&drops), Some(ScalarType::String)))
            } else {
                Err(PrivateError("private second operand failure"))
            }
        });
        assert!(matches!(result, Err(ScalarSourceError::Native { .. })));
        assert_eq!(calls, 2);
        assert_eq!(drops.get(), 1);
        calls = 0;
        assert!(
            catch_unwind(AssertUnwindSafe(|| {
                let _: ScalarSourceResult<Node, PrivateError> =
                    lower_scalar_source(&source, LIMITS, |_| {
                        calls += 1;
                        if calls == 1 {
                            Ok((field(&drops), Some(ScalarType::String)))
                        } else {
                            panic!("native source unwind")
                        }
                    });
            }))
            .is_err()
        );
        assert_eq!(calls, 2);
        assert_eq!(drops.get(), 2);
    }
}

#[test]
fn observed_control_types_still_require_complete_cold_lexical_inference() {
    let source = expression("choose(when: true, then: \"safe\", otherwise: missing)");
    let (node, _): (Node, _) =
        lower_scalar_source(&source, LIMITS, |_| -> Result<_, PrivateError> {
            Ok((
                Node::Local {
                    name: "missing".into(),
                },
                Some(ScalarType::String),
            ))
        })
        .unwrap();
    assert_eq!(checked(&node, &editor()), Err(PureTypeError::UnknownLocal));
    let result: ScalarSourceResult<Node, PrivateError> = lower_scalar_source(
        &expression("choose(when: true, then: first, otherwise: second)"),
        LIMITS,
        |_| {
            Ok((
                Node::Local {
                    name: "reply".into(),
                },
                None,
            ))
        },
    );
    assert!(matches!(result, Err(ScalarSourceError::NonScalar { .. })));
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
fn parsed_panel_call_prepares_control_operand_through_original_native_schema() {
    let source = expression(
        "panel.set(text: join(left: strings(a: caption, b: to_string(value: recover(value: parse_integer(value: \"bad\"), fallback: 7))), right: \",\"))",
    );
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
    let drops = Rc::new(Cell::new(0));
    let lowered = lower_source_call(
        &source,
        &SourceCallHost {
            catalog: &catalog,
            version: 3,
            granted: &[7],
        },
        LIMITS,
        |argument| {
            let (node, ty) =
                lower_scalar_source(&argument.value, LIMITS, |_| -> Result<_, PrivateError> {
                    Ok((field(&drops), Some(ScalarType::String)))
                })?;
            assert_eq!(checked(&node, &editor()), Ok(PureType::Scalar(ty)));
            Ok::<_, ScalarSourceError<PrivateError>>((node, Some(ty)))
        },
    )
    .unwrap();
    assert!(std::ptr::eq(lowered.schema(), &schemas[0]));
    let arguments = lowered.into_arguments();
    let mut bindings = vec![("reply", PureValue::Result(Rc::new("ready".into())))];
    let mut scope = ScopeFrame::new(&mut bindings);
    let prepared = evaluate_call_arguments_in_scope(
        &arguments,
        &schemas[0],
        &mut scope,
        &editor(),
        &mut Fuel::new(100),
        CallEvaluationLimits {
            pure: PURE,
            max_arguments: 1,
        },
    )
    .unwrap();
    assert!(std::ptr::eq(prepared.schema(), &schemas[0]));
    assert_eq!(prepared.arguments().len(), 1);
    assert_eq!(
        prepared.arguments()[0].value,
        ScalarValue::String("ready,7".into())
    );
    assert_eq!(prepared.arguments()[0].argument_index, 0);
}

#[test]
fn reference_control_diagnostics_spans_order_and_effect_branches_are_preserved() {
    for (source, code, message, child) in [
        (
            "choose(when: 1, then: 2, otherwise: 3)",
            "LSH1402",
            "choose when requires a boolean",
            Some(0),
        ),
        (
            "choose(when: true, then: 1, otherwise: false)",
            "LSH1404",
            "choose branches must have the same result type",
            None,
        ),
        (
            "recover(value: 1, fallback: false)",
            "LSH1411",
            "recover value and fallback must be pure expressions of the same scalar type",
            None,
        ),
        (
            "strings(a: 1)",
            "LSH1402",
            "string list entries require strings",
            Some(0),
        ),
        (
            "strings(a: \"a\", a: \"b\")",
            "LSH1403",
            "string list labels must be bounded and unique",
            Some(1),
        ),
    ] {
        let parsed = parse(&format!("fn main() = {source}"));
        let Expression::Call {
            span, arguments, ..
        } = &parsed.function.as_ref().unwrap().body
        else {
            panic!()
        };
        let errors = leselang_hir::lower(&parsed).unwrap_err();
        assert_eq!(errors[0].code, code);
        assert_eq!(errors[0].message, message);
        let expected = match child {
            Some(0) if source.starts_with("choose") => match arguments[0].value {
                Expression::Integer { span, .. } => span,
                _ => panic!(),
            },
            Some(index) => arguments[index].span,
            None => *span,
        };
        assert_eq!(errors[0].span, Some(expected));
    }
    for source in [
        "strings(a: missing, a: \"duplicate\")",
        "choose(when: true, then: missing, otherwise: false)",
        "recover(value: missing, fallback: false)",
    ] {
        let errors = leselang_hir::lower(&parse(&format!("fn main() = {source}"))).unwrap_err();
        assert_eq!(errors[0].code, "LSH1403");
        assert!(errors[0].message.contains("undefined local"));
    }
    let parsed =
        parse("fn main() = choose(when: true, then: runtime.list(), otherwise: runtime.list())");
    let program = leselang_hir::lower(&parsed).unwrap();
    let source = leselang_hir::canonical_source(&program.function.effect).unwrap();
    assert_eq!(leselang_hir::lower(&parse(&source)).unwrap(), program);
}
