use std::cell::{Cell, RefCell};
use std::convert::Infallible;
use std::panic::{AssertUnwindSafe, catch_unwind};

use leselang_hir::ir::Computation;
use leselang_hir::scalar_form::*;
use leselang_hir::scalar_source::{ScalarSourceError, ScalarSourceResult, lower_scalar_source};
use leselang_hir::source_call::SourceCallLimits;
use leselang_runtime_core::{BinaryOperator, ScalarType, ScalarValue, UnaryOperator};
use leselang_syntax::{Expression, NamedArgument, Span, parse};

// GUI-local borrowed slots and errors intentionally have no Clone/Debug/serde/Send.
struct Native<'a> {
    bytes: Vec<u8>,
    drops: &'a Cell<usize>,
}
impl Drop for Native<'_> {
    fn drop(&mut self) {
        self.drops.set(self.drops.get() + 1);
    }
}
struct PrivateError<'a>(&'a str);
type Node<'a> = Computation<Native<'a>, (), Native<'a>, ()>;
const BINARY: [&str; 23] = [
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
];
const UNARY: [&str; 7] = [
    "not",
    "len",
    "to_string",
    "parse_integer",
    "parse_boolean",
    "optional_string",
    "has_value",
];
const TYPES: [ScalarType; 6] = [
    ScalarType::None,
    ScalarType::Integer,
    ScalarType::Boolean,
    ScalarType::String,
    ScalarType::OptionalString,
    ScalarType::StringList,
];
const LIMITS: SourceCallLimits = SourceCallLimits {
    max_source_nodes: 128,
    max_source_depth: 16,
    max_lowered_nodes: 128,
    max_lowered_depth: 16,
    max_arguments: 64,
};

fn native(drops: &Cell<usize>) -> Native<'_> {
    Native {
        bytes: vec![3; 17],
        drops,
    }
}
fn field(drops: &Cell<usize>) -> Node<'_> {
    Node::Field {
        value: Box::new(Node::Local {
            name: "original".into(),
        }),
        field: native(drops),
    }
}
fn pointer(node: &Node<'_>) -> *const u8 {
    let Node::Field { field, .. } = node else {
        panic!()
    };
    field.bytes.as_ptr()
}
fn expression(text: &str) -> Expression {
    let parsed = parse(&format!("fn main() = {text}"));
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    parsed.function.unwrap().body
}
fn parts(source: &Expression) -> (&[NamedArgument], Span) {
    let Expression::Call {
        arguments, span, ..
    } = source
    else {
        panic!()
    };
    (arguments, *span)
}
fn plan(source: &Expression) -> BoundScalarForm<'_> {
    bind_scalar_form(source).unwrap()
}

#[test]
fn scalar_root_classification_retains_closed_vocabulary_without_admitting_shape() {
    for text in [
        "0",
        "true",
        "false",
        "none",
        "\"text\"",
        "strings()",
        "recover()",
        "add()",
        "len()",
    ] {
        let source = expression(text);
        let bound = plan(&source);
        assert_eq!(format!("{bound:?}"), "BoundScalarForm");
    }
    for text in [
        "local", "native()", "choose()", "bind()", "loop()", "fold()", "field()", "member()",
        "seq()", "all()",
    ] {
        assert!(bind_scalar_form(&expression(text)).is_none(), "{text}");
    }
    for name in BINARY.into_iter().chain(UNARY) {
        let source = expression(&format!("{name}()"));
        let result: ScalarSourceResult<Node<'_>, Infallible> =
            plan(&source).lower(|_| panic!("shape must reject first"));
        assert!(matches!(result, Err(ScalarSourceError::Names { .. })));
    }
}

#[test]
fn all_scalar_operator_signatures_cover_870_native_type_combinations_without_execution() {
    let mut cases = 0;
    for name in BINARY {
        let operator = BinaryOperator::parse(name).unwrap();
        let source = expression(&format!("{name}(right: b, left: a)"));
        let (args, _) = parts(&source);
        for left in TYPES {
            for right in TYPES {
                let drops = Cell::new(0);
                let mut calls = 0;
                let result = plan(&source).lower(|original| {
                    assert!(std::ptr::eq(original, &args[1 - calls].value));
                    let ty = if calls == 0 { left } else { right };
                    calls += 1;
                    Ok::<_, Infallible>((field(&drops), Some(ty)))
                });
                assert_eq!(calls, 2);
                match (operator.result_type(left, right), result) {
                    (Some(expected), Ok((node, actual))) => {
                        assert_eq!(expected, actual);
                        assert!(
                            matches!(&node, Node::Binary { operator: actual, .. } if *actual == operator)
                        );
                        drop(node);
                    }
                    (
                        None,
                        Err(ScalarSourceError::Binary {
                            operator: actual,
                            left: l,
                            right: r,
                            ..
                        }),
                    ) => {
                        assert_eq!((actual, l, r), (operator, left, right));
                    }
                    _ => panic!("unexpected signature result"),
                }
                assert_eq!(drops.get(), 2);
                cases += 1;
            }
        }
    }
    for name in UNARY {
        let operator = UnaryOperator::parse(name).unwrap();
        let source = expression(&format!("{name}(value: a)"));
        for input in TYPES {
            let drops = Cell::new(0);
            let result = plan(&source).lower(|_| Ok::<_, Infallible>((field(&drops), Some(input))));
            match (operator.result_type(input), result) {
                (Some(expected), Ok((node, actual))) => {
                    assert_eq!(expected, actual);
                    assert!(
                        matches!(&node, Node::Unary { operator: actual, .. } if *actual == operator)
                    );
                    drop(node);
                }
                (
                    None,
                    Err(ScalarSourceError::Unary {
                        operator: actual, ..
                    }),
                ) => assert_eq!(actual, operator),
                _ => panic!("unexpected unary result"),
            }
            assert_eq!(drops.get(), 1);
            cases += 1;
        }
    }
    assert_eq!(cases, 870);
}

#[test]
fn original_native_fields_buffers_and_ast_references_survive_scalar_construction() {
    let source = expression("add(right: b, left: a)");
    let (args, _) = parts(&source);
    let drops = Cell::new(0);
    let left = field(&drops);
    let right = field(&drops);
    let left_pointer = pointer(&left);
    let right_pointer = pointer(&right);
    let mut nodes = [left, right].into_iter();
    let mut calls = 0;
    let (node, ty) = plan(&source)
        .lower(|original| {
            assert!(std::ptr::eq(original, &args[1 - calls].value));
            calls += 1;
            Ok::<_, PrivateError<'_>>((nodes.next().unwrap(), Some(ScalarType::Integer)))
        })
        .unwrap();
    assert_eq!(ty, ScalarType::Integer);
    let Node::Binary { left, right, .. } = &node else {
        panic!()
    };
    assert_eq!(pointer(left), left_pointer);
    assert_eq!(pointer(right), right_pointer);
    assert_eq!(drops.get(), 0);
    drop(node);
    assert_eq!(drops.get(), 2);
}

#[test]
fn list_and_optional_literal_construction_moves_original_owned_text_buffers() {
    let source = expression("strings(first: a, second: b)");
    let words = [
        String::from("first-original"),
        String::from("second-original"),
    ];
    let pointers = words.each_ref().map(|text| text.as_ptr());
    let mut words = words.into_iter();
    let (node, ty): (Node<'_>, _) = plan(&source)
        .lower(|_| {
            Ok::<_, Infallible>((
                Node::Literal {
                    value: ScalarValue::String(words.next().unwrap()),
                },
                Some(ScalarType::String),
            ))
        })
        .unwrap();
    let Node::Literal {
        value: ScalarValue::StringList(value),
    } = node
    else {
        panic!()
    };
    assert_eq!(ty, ScalarType::StringList);
    assert_eq!(value.0[0].as_ptr(), pointers[0]);
    assert_eq!(value.0[1].as_ptr(), pointers[1]);
    let source = expression("optional_string(value: a)");
    let text = String::from("owned-optional");
    let pointer = text.as_ptr();
    let mut text = Some(text);
    let (node, ty): (Node<'_>, _) = plan(&source)
        .lower(|_| {
            Ok::<_, Infallible>((
                Node::Literal {
                    value: ScalarValue::String(text.take().unwrap()),
                },
                Some(ScalarType::String),
            ))
        })
        .unwrap();
    let Node::Literal {
        value: ScalarValue::OptionalString(value),
    } = node
    else {
        panic!()
    };
    assert_eq!(ty, ScalarType::OptionalString);
    let text = value.0.unwrap();
    assert_eq!(text.as_ptr(), pointer);
}

#[test]
fn list_count_labels_and_entry_errors_keep_sequential_native_priority() {
    let text = format!(
        "strings({})",
        (0..65)
            .map(|i| format!("v{i}: a"))
            .collect::<Vec<_>>()
            .join(", ")
    );
    let source = expression(&text);
    let result: ScalarSourceResult<Node<'_>, Infallible> = plan(&source).lower(|_| panic!());
    assert!(matches!(result, Err(ScalarSourceError::StringCount { .. })));
    let source = expression("strings(first: a, first: b)");
    for mode in 0..3 {
        let drops = Cell::new(0);
        let calls = Cell::new(0);
        let result = plan(&source).lower(|_| {
            calls.set(calls.get() + 1);
            if mode == 0 {
                return Err(PrivateError("first-entry"));
            }
            Ok((
                field(&drops),
                Some(if mode == 1 {
                    ScalarType::Integer
                } else {
                    ScalarType::String
                }),
            ))
        });
        match (mode, result) {
            (0, Err(ScalarSourceError::Native { .. }))
            | (1, Err(ScalarSourceError::StringItem { .. }))
            | (2, Err(ScalarSourceError::StringLabels { .. })) => {}
            _ => panic!("changed entry/label precedence"),
        }
        assert_eq!(calls.get(), 1);
        assert_eq!(drops.get(), usize::from(mode != 0));
    }
}

#[test]
fn binary_and_recovery_compile_cold_children_before_joint_type_admission() {
    for text in [
        "and(left: a, right: b)",
        "or(left: a, right: b)",
        "value_or(left: a, right: b)",
        "recover(value: a, fallback: b)",
    ] {
        let source = expression(text);
        let drops = Cell::new(0);
        let mut calls = 0;
        let result = plan(&source).lower(|_| {
            calls += 1;
            if calls == 2 {
                return Err(PrivateError("cold-child"));
            }
            Ok((field(&drops), None))
        });
        assert!(matches!(
            result,
            Err(ScalarSourceError::Native {
                error: PrivateError("cold-child"),
                ..
            })
        ));
        assert_eq!((calls, drops.get()), (2, 1));
    }
    let source = expression("recover(value: a, fallback: b)");
    let calls = Cell::new(0);
    let result: ScalarSourceResult<Node<'_>, PrivateError<'_>> = plan(&source).lower(|_| {
        calls.set(calls.get() + 1);
        Err(PrivateError("compiler failure is not a runtime recovery"))
    });
    assert!(matches!(result, Err(ScalarSourceError::Native { .. })));
    assert_eq!(calls.get(), 1);
}

#[test]
fn cold_effects_cannot_be_certified_pure_by_native_scalar_observations() {
    for (kind, text, expected_calls) in [
        (0, "add(left: a, right: b)", 2),
        (1, "not(value: a)", 1),
        (2, "strings(first: a)", 1),
        (3, "recover(value: a, fallback: b)", 2),
    ] {
        let source = expression(text);
        let drops = Cell::new(0);
        let mut calls = 0;
        let result = plan(&source).lower(|_| {
            calls += 1;
            let value = Node::Choose {
                when: Box::new(Node::Literal {
                    value: ScalarValue::Boolean(false),
                }),
                then: Box::new(Node::Host {
                    effect: Box::new(native(&drops)),
                }),
                otherwise: Box::new(Node::Literal {
                    value: ScalarValue::Integer(0),
                }),
            };
            Ok::<_, Infallible>((value, Some(ScalarType::Integer)))
        });
        match (kind, result) {
            (0 | 2, Err(ScalarSourceError::NonScalar { .. }))
            | (1, Err(ScalarSourceError::Unary { .. }))
            | (3, Err(ScalarSourceError::RecoveryTypes { .. })) => {}
            _ => panic!("cold effect was admitted"),
        }
        assert_eq!(calls, expected_calls);
        assert_eq!(drops.get(), expected_calls);
    }
}

#[test]
fn native_refusal_retains_private_error_identity_and_original_source_span() {
    for text in [
        "add(left: a, right: b)",
        "recover(value: a, fallback: b)",
        "strings(first: a, second: b)",
    ] {
        for fail in 0..2 {
            let source = expression(text);
            let (args, _) = parts(&source);
            let drops = Cell::new(0);
            let secret = String::from("private-original");
            let mut calls = 0;
            let result = plan(&source).lower(|_| {
                let current = calls;
                calls += 1;
                if current == fail {
                    return Err(PrivateError(&secret));
                }
                Ok((
                    field(&drops),
                    Some(if text.starts_with("strings") {
                        ScalarType::String
                    } else {
                        ScalarType::Integer
                    }),
                ))
            });
            let Err(error) = result else { panic!() };
            assert_eq!(
                format!("{error:?}"),
                "native source extension lowering failed"
            );
            let ScalarSourceError::Native { span, error } = error else {
                panic!()
            };
            assert_eq!(error.0.as_ptr(), secret.as_ptr());
            let Expression::Reference { span: original, .. } = &args[fail].value else {
                panic!()
            };
            assert_eq!(span, *original);
            assert_eq!(calls, fail + 1);
            assert_eq!(drops.get(), fail);
        }
    }
}

#[test]
fn native_unwind_drops_prior_operands_once_without_retry_or_fallback() {
    for text in [
        "add(left: a, right: b)",
        "recover(value: a, fallback: b)",
        "strings(first: a, second: b)",
    ] {
        for fail in 0..2 {
            let source = expression(text);
            let drops = Cell::new(0);
            let calls = Cell::new(0);
            let result = catch_unwind(AssertUnwindSafe(|| {
                plan(&source).lower(|_| {
                    let current = calls.get();
                    calls.set(current + 1);
                    assert_ne!(current, fail, "native scalar unwind");
                    Ok::<_, Infallible>((
                        field(&drops),
                        Some(if text.starts_with("strings") {
                            ScalarType::String
                        } else {
                            ScalarType::Integer
                        }),
                    ))
                })
            }));
            assert!(result.is_err());
            assert_eq!(calls.get(), fail + 1);
            assert_eq!(drops.get(), fail);
        }
    }
}

#[test]
fn delayed_scalar_classification_does_not_cache_native_policy_or_authority() {
    let source = expression("add(left: a, right: b)");
    let allowed = Cell::new(true);
    let form = plan(&source);
    allowed.set(false);
    let events = RefCell::new(vec![]);
    let result: ScalarSourceResult<Node<'_>, PrivateError<'_>> = form.lower(|_| {
        events.borrow_mut().push("current-policy");
        assert!(!allowed.get());
        Err(PrivateError("revoked"))
    });
    assert!(matches!(
        result,
        Err(ScalarSourceError::Native {
            error: PrivateError("revoked"),
            ..
        })
    ));
    assert_eq!(*events.borrow(), ["current-policy"]);
}

#[test]
fn scalar_form_classification_does_not_replace_strict_cold_source_scanning() {
    let mut source = expression("add(left: a, right: b)");
    let Expression::Call { arguments, .. } = &mut source else {
        panic!()
    };
    arguments[1].value = Expression::Call {
        callee: "invalid name".into(),
        arguments: vec![],
        span: arguments[1].span,
    };
    let result: ScalarSourceResult<Node<'_>, PrivateError<'_>> =
        plan(&source).lower(|_| Err(PrivateError("first-child")));
    assert!(matches!(result, Err(ScalarSourceError::Native { .. })));
    let strict: ScalarSourceResult<Node<'_>, Infallible> =
        lower_scalar_source(&source, LIMITS, |_| panic!("cold scan must reject first"));
    assert!(matches!(strict, Err(ScalarSourceError::Source(_))));
}

#[test]
fn literal_and_aggregate_text_bounds_keep_exact_inclusive_limits() {
    for bytes in [4096, 4097] {
        let source = expression(&format!("\"{}\"", "a".repeat(bytes)));
        let result: ScalarSourceResult<Node<'_>, Infallible> = plan(&source).lower(|_| panic!());
        if bytes == 4096 {
            assert!(matches!(
                result,
                Ok((Node::Literal { .. }, ScalarType::String))
            ));
        } else {
            assert!(matches!(
                result,
                Err(ScalarSourceError::LiteralLimit { .. })
            ));
        }
    }
    let source = expression("strings(first: a, second: b)");
    for bytes in [2048, 2049] {
        let mut calls = 0;
        let result: ScalarSourceResult<Node<'_>, Infallible> = plan(&source).lower(|_| {
            calls += 1;
            let size = if calls == 1 { bytes } else { 2048 };
            Ok((
                Node::Literal {
                    value: ScalarValue::String("a".repeat(size)),
                },
                Some(ScalarType::String),
            ))
        });
        assert_eq!(calls, 2);
        if bytes == 2048 {
            assert!(result.is_ok());
        } else {
            assert!(matches!(result, Err(ScalarSourceError::StringBytes { .. })));
        }
    }
}
