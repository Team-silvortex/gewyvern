use std::cell::{Cell, RefCell};
use std::convert::Infallible;
use std::panic::{AssertUnwindSafe, catch_unwind};

use leselang_hir::ir::Computation;
use leselang_hir::loop_source::*;
use leselang_hir::scalar_source::{
    ScalarSourceError, ScalarSourceLimits, lower_scalar_source_with_scope,
};
use leselang_hir::source_call::SourceCallLimits;
use leselang_runtime_core::{ScalarType, ScalarValue, ScopeFrame};
use leselang_syntax::{Expression, NamedArgument, Span, parse};

// Move-only borrowed host data deliberately has no Clone/Debug/serde/Send bound.
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

fn native(drops: &Cell<usize>) -> Native<'_> {
    Native {
        bytes: vec![9; 19],
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
fn plan(source: &Expression) -> BoundLoopSource<'_> {
    let (arguments, at) = parts(source);
    bind_loop_source::<Infallible>(arguments, at).unwrap()
}
fn pointer(node: &Node<'_>) -> *const u8 {
    let Node::Field { field, .. } = node else {
        panic!()
    };
    field.bytes.as_ptr()
}
fn loop_expression() -> Expression {
    expression("loop(next: step, limit: 0, state: start, while: check)")
}

#[test]
fn original_loop_parts_and_metadata_move_once_after_native_scope_cleanup() {
    let source = loop_expression();
    let (args, _) = parts(&source);
    let drops = Cell::new(0);
    let events = RefCell::new(vec![]);
    let bound = plan(&source);
    assert!(std::ptr::eq(bound.binding(), &args[2]));
    assert_eq!(format!("{bound:?}"), "BoundLoopSource");
    let initial = field(&drops);
    let initial_pointer = pointer(&initial);
    let metadata = native(&drops);
    let metadata_pointer = metadata.bytes.as_ptr();
    let prepared = bound
        .prepare(|original| {
            assert!(std::ptr::eq(original, &args[2].value));
            events.borrow_mut().push("initial");
            Ok::<_, PrivateError<'_>>((initial, Some(ScalarType::Integer), metadata))
        })
        .unwrap();
    assert!(std::ptr::eq(prepared.binding(), &args[2]));
    assert_eq!(prepared.metadata().bytes.as_ptr(), metadata_pointer);
    assert_eq!(format!("{prepared:?}"), "PreparedLoopSource");
    let condition = field(&drops);
    let condition_pointer = pointer(&condition);
    let next = field(&drops);
    let next_pointer = pointer(&next);
    let mut children = [condition, next].into_iter();
    let mut bindings = vec![("prefix", ScalarType::String)];
    let mut scope = ScopeFrame::new(&mut bindings);
    let lowered = {
        let mut local = scope.nested();
        local
            .push(&prepared.binding().name, ScalarType::Integer)
            .unwrap();
        let mut calls = 0;
        prepared
            .lower_body(|original| {
                assert_eq!(local.get("state"), Some(&ScalarType::Integer));
                let index = if calls == 0 { 3 } else { 0 };
                assert!(std::ptr::eq(original, &args[index].value));
                let ty = if calls == 0 {
                    ScalarType::Boolean
                } else {
                    ScalarType::Integer
                };
                events
                    .borrow_mut()
                    .push(if calls == 0 { "condition" } else { "next" });
                calls += 1;
                Ok::<_, PrivateError<'_>>((children.next().unwrap(), Some(ty)))
            })
            .unwrap()
    };
    assert_eq!(scope.len(), 1);
    assert!(scope.get("state").is_none());
    events.borrow_mut().push("scope-exit");
    assert_eq!(format!("{lowered:?}"), "LoweredLoopSource");
    let (node, metadata) = lowered.finish();
    assert_eq!(metadata.bytes.as_ptr(), metadata_pointer);
    assert_eq!(drops.get(), 0);
    let Node::Loop {
        name,
        initial,
        condition,
        next,
        limit,
    } = &node
    else {
        panic!()
    };
    assert_eq!((name.as_str(), *limit), ("state", 0));
    assert_eq!(pointer(initial), initial_pointer);
    assert_eq!(pointer(condition), condition_pointer);
    assert_eq!(pointer(next), next_pointer);
    assert_eq!(
        *events.borrow(),
        ["initial", "condition", "next", "scope-exit"]
    );
    drop((node, metadata));
    assert_eq!(drops.get(), 4);
}

#[test]
fn literal_loop_limits_precede_initial_lowering_without_executing_iterations() {
    for (limit, expected) in [
        ("missing", Some(false)),
        ("1025", Some(true)),
        ("0", None),
        ("1024", None),
    ] {
        let source = expression(&format!(
            "loop(state: 0, while: false, next: state, limit: {limit})"
        ));
        let calls = Cell::new(0);
        let result = plan(&source).prepare(|_| {
            calls.set(calls.get() + 1);
            Ok::<_, Infallible>((
                Node::Literal {
                    value: ScalarValue::Integer(0),
                },
                Some(ScalarType::Integer),
                (),
            ))
        });
        if let Some(expected) = expected {
            assert!(
                matches!(result, Err(ScalarSourceError::LoopLimit { exceeds_bound, .. }) if exceeds_bound == expected)
            );
            assert_eq!(calls.get(), 0);
        } else {
            let mut body_calls = 0;
            let (node, ()) = result
                .unwrap()
                .lower_body(|_| {
                    body_calls += 1;
                    let value = if body_calls == 1 {
                        ScalarValue::Boolean(false)
                    } else {
                        ScalarValue::Integer(0)
                    };
                    let ty = value.scalar_type();
                    Ok::<_, Infallible>((Node::Literal { value }, Some(ty)))
                })
                .unwrap()
                .finish();
            assert_eq!((calls.get(), body_calls), (1, 2));
            assert!(
                matches!(node, Node::Loop { limit: actual, .. } if actual.to_string() == limit)
            );
        }
    }
}

#[test]
fn generated_purity_and_state_types_are_checked_in_original_cold_order() {
    for phase in 0..3 {
        for impure in [false, true] {
            let source = loop_expression();
            let drops = Cell::new(0);
            let calls = Cell::new(0);
            let child = |index| {
                calls.set(calls.get() + 1);
                let value = if index == phase && impure {
                    Node::Choose {
                        when: Box::new(Node::Literal {
                            value: ScalarValue::Boolean(false),
                        }),
                        then: Box::new(Node::Host {
                            effect: Box::new(native(&drops)),
                        }),
                        otherwise: Box::new(field(&drops)),
                    }
                } else {
                    field(&drops)
                };
                let ty = if index == phase && !impure {
                    if phase == 0 {
                        None
                    } else {
                        Some(ScalarType::String)
                    }
                } else {
                    Some(if index == 1 {
                        ScalarType::Boolean
                    } else {
                        ScalarType::Integer
                    })
                };
                (value, ty)
            };
            let prepared = plan(&source).prepare(|_| {
                let (value, ty) = child(0);
                Ok::<_, Infallible>((value, ty, native(&drops)))
            });
            let result = prepared.and_then(|prepared| {
                let mut index = 0;
                prepared.lower_body(|_| {
                    index += 1;
                    Ok(child(index))
                })
            });
            match (phase, impure, result) {
                (_, true, Err(ScalarSourceError::NonScalar { .. }))
                | (0, false, Err(ScalarSourceError::NonScalar { .. }))
                | (1, false, Err(ScalarSourceError::LoopCondition { .. }))
                | (2, false, Err(ScalarSourceError::LoopState { .. })) => {}
                _ => panic!("wrong phase or priority"),
            }
            assert_eq!(calls.get(), phase + 1);
            assert_eq!(drops.get(), phase + 2 + usize::from(impure));
        }
    }
}

#[test]
fn native_refusal_retains_original_private_error_and_restores_scope() {
    for fail in 0..3 {
        let source = loop_expression();
        let (args, _) = parts(&source);
        let drops = Cell::new(0);
        let secret = String::from("native private diagnostic");
        let mut bindings = vec![("prefix", ScalarType::None)];
        let mut scope = ScopeFrame::new(&mut bindings);
        let prepared = plan(&source).prepare(|_| {
            if fail == 0 {
                return Err(PrivateError(&secret));
            }
            Ok((field(&drops), Some(ScalarType::Integer), native(&drops)))
        });
        let result = prepared.and_then(|prepared| {
            let mut local = scope.nested();
            local
                .push(&prepared.binding().name, ScalarType::Integer)
                .unwrap();
            let mut index = 0;
            prepared.lower_body(|_| {
                index += 1;
                if index == fail {
                    return Err(PrivateError(&secret));
                }
                Ok((field(&drops), Some(ScalarType::Boolean)))
            })
        });
        assert_eq!(scope.len(), 1);
        assert_eq!(drops.get(), if fail == 0 { 0 } else { fail + 1 });
        assert_eq!(
            format!("{:?}", result.as_ref().unwrap_err()),
            "native source extension lowering failed"
        );
        let Err(ScalarSourceError::Native { error, span }) = result else {
            panic!()
        };
        assert_eq!(error.0.as_ptr(), secret.as_ptr());
        let index = [2, 3, 0][fail];
        let Expression::Reference { span: original, .. } = &args[index].value else {
            panic!()
        };
        assert_eq!(span, *original);
    }
}

#[test]
fn native_unwind_releases_all_parts_without_retry_and_restores_scope() {
    for fail in 0..3 {
        let source = loop_expression();
        let drops = Cell::new(0);
        let calls = Cell::new(0);
        let mut bindings = vec![("prefix", ScalarType::None)];
        let mut scope = ScopeFrame::new(&mut bindings);
        let result = catch_unwind(AssertUnwindSafe(|| {
            let prepared = plan(&source)
                .prepare(|_| {
                    calls.set(calls.get() + 1);
                    assert_ne!(fail, 0, "native initial unwind");
                    Ok::<_, Infallible>((field(&drops), Some(ScalarType::Integer), native(&drops)))
                })
                .unwrap();
            let mut local = scope.nested();
            local
                .push(&prepared.binding().name, ScalarType::Integer)
                .unwrap();
            let mut index = 0;
            prepared
                .lower_body(|_| {
                    index += 1;
                    calls.set(calls.get() + 1);
                    assert_ne!(index, fail, "native body unwind");
                    Ok::<_, Infallible>((field(&drops), Some(ScalarType::Boolean)))
                })
                .unwrap()
        }));
        assert!(result.is_err());
        assert_eq!(scope.len(), 1);
        assert_eq!(calls.get(), fail + 1);
        assert_eq!(drops.get(), if fail == 0 { 0 } else { fail + 1 });
    }
}

#[test]
fn delayed_loop_preparation_rechecks_native_policy_in_cold_body_callbacks() {
    let source = loop_expression();
    let drops = Cell::new(0);
    let allowed = Cell::new(true);
    let prepared = plan(&source)
        .prepare(|_| {
            assert!(allowed.get());
            Ok::<_, PrivateError<'_>>((field(&drops), Some(ScalarType::Integer), native(&drops)))
        })
        .unwrap();
    allowed.set(false);
    let calls = Cell::new(0);
    let result = prepared.lower_body(|_| {
        calls.set(calls.get() + 1);
        if !allowed.get() {
            return Err(PrivateError("revoked"));
        }
        Ok((field(&drops), Some(ScalarType::Boolean)))
    });
    assert!(matches!(
        result,
        Err(ScalarSourceError::Native {
            error: PrivateError("revoked"),
            ..
        })
    ));
    assert_eq!((calls.get(), drops.get()), (1, 2));
}

#[test]
fn root_alignment_does_not_replace_whole_cold_source_admission() {
    let mut source = loop_expression();
    let Expression::Call { arguments, .. } = &mut source else {
        panic!()
    };
    arguments[0].value = Expression::Call {
        callee: "invalid name".into(),
        arguments: vec![],
        span: arguments[0].span,
    };
    let native = plan(&source).prepare(|_| {
        Err::<(Node<'_>, Option<ScalarType>, ()), _>(PrivateError("initial native policy"))
    });
    assert!(matches!(native, Err(ScalarSourceError::Native { .. })));
    let strict = lower_scalar_source_with_scope::<(), (), (), (), Infallible>(
        &source,
        ScalarSourceLimits {
            source: SourceCallLimits {
                max_source_nodes: 64,
                max_source_depth: 8,
                max_lowered_nodes: 64,
                max_lowered_depth: 8,
                max_arguments: 8,
            },
            max_bindings: 8,
        },
        &[],
        |_, _| panic!("strict source scan must reject before extensions"),
    );
    assert!(matches!(strict, Err(ScalarSourceError::Source(_))));
}

#[test]
fn discarding_each_loop_stage_drops_only_its_original_owned_parts() {
    for phase in 0..3 {
        let source = loop_expression();
        let drops = Cell::new(0);
        let bound = plan(&source);
        if phase == 0 {
            let _discarded = bound;
            assert_eq!(drops.get(), 0);
            continue;
        }
        let prepared = bound
            .prepare(|_| {
                Ok::<_, Infallible>((field(&drops), Some(ScalarType::Integer), native(&drops)))
            })
            .unwrap();
        if phase == 1 {
            drop(prepared);
            assert_eq!(drops.get(), 2);
            continue;
        }
        let mut calls = 0;
        let lowered = prepared
            .lower_body(|_| {
                calls += 1;
                Ok::<_, Infallible>((
                    field(&drops),
                    Some(if calls == 1 {
                        ScalarType::Boolean
                    } else {
                        ScalarType::Integer
                    }),
                ))
            })
            .unwrap();
        drop(lowered);
        assert_eq!(drops.get(), 4);
    }
}
