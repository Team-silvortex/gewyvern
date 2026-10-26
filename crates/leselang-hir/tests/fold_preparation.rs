use std::cell::{Cell, RefCell};
use std::convert::Infallible;
use std::panic::{AssertUnwindSafe, catch_unwind};

use leselang_hir::fold_source::*;
use leselang_hir::ir::Computation;
use leselang_hir::pure_typing::PureTypeError;
use leselang_hir::scalar_source::{
    ScalarSourceError, ScalarSourceLimits, lower_scalar_source_with_scope,
};
use leselang_hir::source_call::SourceCallLimits;
use leselang_runtime_core::{ScalarType, ScalarValue, ScopeFrame};
use leselang_syntax::{Expression, NamedArgument, Span, parse};

// Host data is borrowed and move-only, without Clone/Debug/serde/Send bounds.
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
        bytes: vec![5; 23],
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
fn plan(source: &Expression) -> BoundFoldSource<'_> {
    let (arguments, at) = parts(source);
    bind_fold_source::<Infallible>(arguments, at).unwrap()
}
fn fold_expression() -> Expression {
    expression("fold(next: step, limit: 0, total: start, item: \"entry\", items: source)")
}

#[test]
fn original_collection_state_next_and_metadata_survive_once_ordered_native_handoff() {
    let source = fold_expression();
    let (args, _) = parts(&source);
    let drops = Cell::new(0);
    let events = RefCell::new(vec![]);
    let mut bindings = vec![("prefix", ScalarType::None)];
    let mut scope = ScopeFrame::new(&mut bindings);
    let bound = plan(&source);
    assert_eq!(format!("{bound:?}"), "BoundFoldSource");
    bound.check_scope::<_, Infallible>(&scope, Some(3)).unwrap();
    let items = field(&drops);
    let items_pointer = pointer(&items);
    let collection = bound
        .prepare_items(|original| {
            assert!(std::ptr::eq(original, &args[4].value));
            assert_eq!(scope.len(), 1);
            events.borrow_mut().push("items");
            Ok::<_, PrivateError<'_>>((items, Some(ScalarType::StringList)))
        })
        .unwrap();
    assert_eq!(format!("{collection:?}"), "FoldItemsSource");
    let initial = field(&drops);
    let initial_pointer = pointer(&initial);
    let metadata = native(&drops);
    let metadata_pointer = metadata.bytes.as_ptr();
    let prepared = collection
        .prepare_initial(|original| {
            assert!(std::ptr::eq(original, &args[2].value));
            assert_eq!(scope.len(), 1);
            events.borrow_mut().push("initial");
            Ok::<_, PrivateError<'_>>((initial, Some(ScalarType::Integer), metadata))
        })
        .unwrap();
    assert_eq!(format!("{prepared:?}"), "PreparedFoldSource");
    assert!(std::ptr::eq(prepared.binding(), &args[2]));
    let Expression::String { value, .. } = &args[3].value else {
        panic!()
    };
    assert_eq!(prepared.item().as_ptr(), value.as_ptr());
    assert_eq!(prepared.metadata().bytes.as_ptr(), metadata_pointer);
    let next = field(&drops);
    let next_pointer = pointer(&next);
    let lowered = {
        let mut local = scope.nested();
        local
            .push(&prepared.binding().name, ScalarType::Integer)
            .unwrap();
        local.push(prepared.item(), ScalarType::String).unwrap();
        prepared
            .lower_next(|original| {
                assert!(std::ptr::eq(original, &args[0].value));
                assert_eq!(local.len(), 3);
                assert_eq!(local.get("entry"), Some(&ScalarType::String));
                events.borrow_mut().push("next");
                Ok::<_, PrivateError<'_>>((next, Some(ScalarType::Integer)))
            })
            .unwrap()
    };
    assert_eq!(scope.len(), 1);
    assert!(scope.get("entry").is_none());
    events.borrow_mut().push("scope-exit");
    assert_eq!(format!("{lowered:?}"), "LoweredFoldSource");
    let (node, metadata) = lowered.finish::<PrivateError<'_>>().unwrap();
    let Node::Fold {
        name,
        item,
        items,
        initial,
        next,
        limit,
    } = &node
    else {
        panic!()
    };
    assert_eq!(
        (name.as_str(), item.as_str(), *limit),
        ("total", "entry", 0)
    );
    assert_eq!(pointer(items), items_pointer);
    assert_eq!(pointer(initial), initial_pointer);
    assert_eq!(pointer(next), next_pointer);
    assert_eq!(metadata.bytes.as_ptr(), metadata_pointer);
    assert_eq!(*events.borrow(), ["items", "initial", "next", "scope-exit"]);
    assert_eq!(drops.get(), 0);
    drop((node, metadata));
    assert_eq!(drops.get(), 4);
}

#[test]
fn fold_metadata_and_two_local_scope_policy_precede_limits_and_callbacks() {
    for text in [
        "fold(items: source, item: \"entry\", next: step, limit: 0)",
        "fold(total: 0, items: source, item: 1, next: step, limit: 0)",
        "fold(total: 0, items: source, item: \"total\", next: step, limit: 0)",
        "fold(total: 0, items: source, item: \"bad name\", next: step, limit: 0)",
        "fold(total: 0, items: source, items: source, item: \"entry\", next: step, limit: 0)",
    ] {
        let source = expression(text);
        let (args, at) = parts(&source);
        assert!(bind_fold_source::<Infallible>(args, at).is_err());
    }
    let source =
        expression("fold(total: 0, items: source, item: \"entry\", next: step, limit: 65)");
    for name in ["total", "entry", "prefix"] {
        let mut bindings = vec![(name, ())];
        let scope = ScopeFrame::new(&mut bindings);
        let bound = plan(&source);
        let result = bound.check_scope::<_, Infallible>(&scope, Some(2));
        let expected = if name == "prefix" {
            PureTypeError::BindingLimit
        } else {
            PureTypeError::ShadowedBinding
        };
        assert!(
            matches!(result, Err(ScalarSourceError::FoldBindings { error, .. }) if error == expected)
        );
        if name == "prefix" {
            bound.check_scope::<_, Infallible>(&scope, Some(3)).unwrap();
            bound.check_scope::<_, Infallible>(&scope, None).unwrap();
            let limit =
                bound.prepare_items(|_| -> FoldSourceChild<Node<'_>, Infallible> { panic!() });
            assert!(matches!(
                limit,
                Err(ScalarSourceError::FoldLimit {
                    exceeds_bound: true,
                    ..
                })
            ));
        }
    }
}

#[test]
fn fold_iteration_limits_are_inclusive_and_zero_still_compiles_next() {
    for (limit, expected) in [
        ("missing", Some(false)),
        ("65", Some(true)),
        ("0", None),
        ("64", None),
    ] {
        let source = expression(&format!(
            "fold(total: 0, items: strings(), item: \"entry\", next: total, limit: {limit})"
        ));
        let calls = Cell::new(0);
        let collection = plan(&source).prepare_items(|_| {
            calls.set(calls.get() + 1);
            Ok::<_, Infallible>((
                Node::Strings { items: vec![] },
                Some(ScalarType::StringList),
            ))
        });
        if let Some(expected) = expected {
            assert!(
                matches!(collection, Err(ScalarSourceError::FoldLimit { exceeds_bound, .. }) if exceeds_bound == expected)
            );
            assert_eq!(calls.get(), 0);
            continue;
        }
        let (node, ()) = collection
            .unwrap()
            .prepare_initial(|_| {
                calls.set(calls.get() + 1);
                Ok::<_, Infallible>((
                    Node::Literal {
                        value: ScalarValue::Integer(0),
                    },
                    Some(ScalarType::Integer),
                    (),
                ))
            })
            .unwrap()
            .lower_next(|_| {
                calls.set(calls.get() + 1);
                Ok::<_, Infallible>((
                    Node::Local {
                        name: "total".into(),
                    },
                    Some(ScalarType::Integer),
                ))
            })
            .unwrap()
            .finish::<Infallible>()
            .unwrap();
        assert_eq!(calls.get(), 3);
        assert!(matches!(node, Node::Fold { limit: actual, .. } if actual.to_string() == limit));
    }
}

#[test]
fn collection_initial_and_next_keep_their_distinct_purity_and_type_gates() {
    for phase in 0..3 {
        for impure in [false, true] {
            let source = fold_expression();
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
                    if phase == 1 {
                        None
                    } else {
                        Some(ScalarType::String)
                    }
                } else {
                    Some(if index == 0 {
                        ScalarType::StringList
                    } else {
                        ScalarType::Integer
                    })
                };
                (value, ty)
            };
            let result = plan(&source)
                .prepare_items(|_| Ok::<_, Infallible>(child(0)))
                .and_then(|items| {
                    items.prepare_initial(|_| {
                        let (value, ty) = child(1);
                        Ok((value, ty, native(&drops)))
                    })
                })
                .and_then(|prepared| prepared.lower_next(|_| Ok(child(2))))
                .and_then(|lowered| lowered.finish());
            match (phase, impure, result) {
                (_, true, Err(ScalarSourceError::NonScalar { .. }))
                | (1, false, Err(ScalarSourceError::NonScalar { .. }))
                | (0, false, Err(ScalarSourceError::FoldItems { .. }))
                | (2, false, Err(ScalarSourceError::FoldState { .. })) => {}
                _ => panic!("wrong phase or diagnostic"),
            }
            assert_eq!(calls.get(), phase + 1);
            assert_eq!(
                drops.get(),
                phase + 1 + usize::from(phase > 0) + usize::from(impure)
            );
        }
    }
}

#[test]
fn next_admission_is_deferred_until_after_both_native_locals_are_released() {
    for impure in [false, true] {
        let source = fold_expression();
        let drops = Cell::new(0);
        let prepared = plan(&source)
            .prepare_items(|_| Ok::<_, Infallible>((field(&drops), Some(ScalarType::StringList))))
            .unwrap()
            .prepare_initial(|_| {
                Ok::<_, Infallible>((field(&drops), Some(ScalarType::Integer), native(&drops)))
            })
            .unwrap();
        let mut bindings = vec![("prefix", ScalarType::None)];
        let mut scope = ScopeFrame::new(&mut bindings);
        let lowered = {
            let mut local = scope.nested();
            local
                .push(&prepared.binding().name, ScalarType::Integer)
                .unwrap();
            local.push(prepared.item(), ScalarType::String).unwrap();
            let result = prepared.lower_next(|_| {
                assert_eq!(local.len(), 3);
                let node = if impure {
                    Node::Host {
                        effect: Box::new(native(&drops)),
                    }
                } else {
                    field(&drops)
                };
                Ok::<_, Infallible>((node, Some(ScalarType::String)))
            });
            assert!(
                result.is_ok(),
                "type/purity checks must wait for scope exit"
            );
            assert_eq!(drops.get(), 0);
            result.unwrap()
        };
        assert_eq!(scope.len(), 1);
        let result = lowered.finish::<Infallible>();
        assert!(matches!(
            result,
            Err(ScalarSourceError::NonScalar { .. }) | Err(ScalarSourceError::FoldState { .. })
        ));
        assert_eq!(drops.get(), 4);
    }
}

#[test]
fn native_fold_refusal_preserves_private_error_identity_span_and_owned_cleanup() {
    for fail in 0..3 {
        let source = fold_expression();
        let (args, _) = parts(&source);
        let drops = Cell::new(0);
        let secret = String::from("private host detail");
        let calls = Cell::new(0);
        let child = |phase| {
            calls.set(calls.get() + 1);
            if phase == fail {
                return Err(PrivateError(&secret));
            }
            Ok((
                field(&drops),
                Some(if phase == 0 {
                    ScalarType::StringList
                } else {
                    ScalarType::Integer
                }),
            ))
        };
        let result = plan(&source)
            .prepare_items(|_| child(0))
            .and_then(|items| {
                items.prepare_initial(|_| {
                    let (value, ty) = child(1)?;
                    Ok((value, ty, native(&drops)))
                })
            })
            .and_then(|prepared| prepared.lower_next(|_| child(2)));
        assert_eq!(
            format!("{:?}", result.as_ref().unwrap_err()),
            "native source extension lowering failed"
        );
        let Err(ScalarSourceError::Native { span, error }) = result else {
            panic!()
        };
        assert_eq!(error.0.as_ptr(), secret.as_ptr());
        let Expression::Reference { span: original, .. } = &args[[4, 2, 0][fail]].value else {
            panic!()
        };
        assert_eq!(span, *original);
        assert_eq!(calls.get(), fail + 1);
        assert_eq!(drops.get(), [0, 1, 3][fail]);
    }
}

#[test]
fn native_fold_unwind_restores_scope_and_never_retries_or_leaks_prior_parts() {
    for fail in 0..3 {
        let source = fold_expression();
        let drops = Cell::new(0);
        let calls = Cell::new(0);
        let mut bindings = vec![("prefix", ScalarType::None)];
        let mut scope = ScopeFrame::new(&mut bindings);
        let result = catch_unwind(AssertUnwindSafe(|| {
            let child = |phase| {
                calls.set(calls.get() + 1);
                assert_ne!(phase, fail, "native fold unwind");
                Ok::<_, Infallible>((
                    field(&drops),
                    Some(if phase == 0 {
                        ScalarType::StringList
                    } else {
                        ScalarType::Integer
                    }),
                ))
            };
            let prepared = plan(&source)
                .prepare_items(|_| child(0))
                .unwrap()
                .prepare_initial(|_| {
                    let (value, ty) = child(1)?;
                    Ok::<_, Infallible>((value, ty, native(&drops)))
                })
                .unwrap();
            let mut local = scope.nested();
            local
                .push(&prepared.binding().name, ScalarType::Integer)
                .unwrap();
            local.push(prepared.item(), ScalarType::String).unwrap();
            prepared.lower_next(|_| child(2)).unwrap()
        }));
        assert!(result.is_err());
        assert_eq!(scope.len(), 1);
        assert_eq!(calls.get(), fail + 1);
        assert_eq!(drops.get(), [0, 1, 3][fail]);
    }
}

#[test]
fn delayed_fold_stages_observe_current_native_policy_without_a_cached_grant() {
    let source = fold_expression();
    let drops = Cell::new(0);
    let allowed = Cell::new(true);
    let prepared = plan(&source)
        .prepare_items(|_| Ok::<_, PrivateError<'_>>((field(&drops), Some(ScalarType::StringList))))
        .unwrap()
        .prepare_initial(|_| {
            assert!(allowed.get());
            Ok::<_, PrivateError<'_>>((field(&drops), Some(ScalarType::Integer), native(&drops)))
        })
        .unwrap();
    allowed.set(false);
    let calls = Cell::new(0);
    let result = prepared.lower_next(|_| {
        calls.set(calls.get() + 1);
        if !allowed.get() {
            return Err(PrivateError("revoked"));
        }
        Ok((field(&drops), Some(ScalarType::Integer)))
    });
    assert!(matches!(
        result,
        Err(ScalarSourceError::Native {
            error: PrivateError("revoked"),
            ..
        })
    ));
    assert_eq!((calls.get(), drops.get()), (1, 3));
}

#[test]
fn fold_alignment_does_not_replace_strict_whole_cold_source_scanning() {
    let mut source = fold_expression();
    let Expression::Call { arguments, .. } = &mut source else {
        panic!()
    };
    arguments[0].value = Expression::Call {
        callee: "invalid name".into(),
        arguments: vec![],
        span: arguments[0].span,
    };
    let native = plan(&source).prepare_items(|_| {
        Err::<(Node<'_>, Option<ScalarType>), _>(PrivateError("collection policy"))
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
        |_, _| panic!("strict source scan must reject before callbacks"),
    );
    assert!(matches!(strict, Err(ScalarSourceError::Source(_))));
}

#[test]
fn discarding_fold_stages_releases_only_the_original_retained_parts() {
    for phase in 0..4 {
        let source = fold_expression();
        let drops = Cell::new(0);
        let bound = plan(&source);
        if phase == 0 {
            let _discarded = bound;
            assert_eq!(drops.get(), 0);
            continue;
        }
        let collection = bound
            .prepare_items(|_| Ok::<_, Infallible>((field(&drops), Some(ScalarType::StringList))))
            .unwrap();
        if phase == 1 {
            drop(collection);
            assert_eq!(drops.get(), 1);
            continue;
        }
        let prepared = collection
            .prepare_initial(|_| {
                Ok::<_, Infallible>((field(&drops), Some(ScalarType::Integer), native(&drops)))
            })
            .unwrap();
        if phase == 2 {
            drop(prepared);
            assert_eq!(drops.get(), 3);
            continue;
        }
        let lowered = prepared
            .lower_next(|_| Ok::<_, Infallible>((field(&drops), Some(ScalarType::Integer))))
            .unwrap();
        drop(lowered);
        assert_eq!(drops.get(), 4);
    }
}
