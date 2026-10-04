use leselang_hir::computation::{
    BinaryOperator, Computation, MAX_COMPUTATION_NODES, OptionalStringValue, ScalarValue,
    StringListValue, UnaryOperator,
};
use leselang_hir::host_call::HostOperation;
use leselang_hir::{CanonicalSourceError, Effect, MAX_EFFECT_NESTING_DEPTH};

fn integer() -> Computation {
    Computation::Literal {
        value: ScalarValue::Integer(1),
    }
}

fn tree(leaves: usize) -> Computation {
    if leaves == 1 {
        return integer();
    }
    Computation::Binary {
        operator: BinaryOperator::Add,
        left: Box::new(tree(leaves / 2)),
        right: Box::new(tree(leaves - leaves / 2)),
    }
}

fn wrap(mut value: Computation, depth: usize) -> Computation {
    for _ in 0..depth {
        value = Computation::Unary {
            operator: UnaryOperator::Len,
            value: Box::new(value),
        };
    }
    value
}

fn focus() -> Computation {
    Computation::Host {
        effect: Box::new(Effect::UiFocus {
            node_id: "target".into(),
        }),
    }
}

fn replace_leaf(mut value: &mut Computation, replacement: ScalarValue) {
    loop {
        match value {
            Computation::Unary { value: inner, .. } => value = inner,
            Computation::Binary { left, .. } => value = left,
            _ => break,
        }
    }
    *value = Computation::Literal { value: replacement };
}

fn error(value: &Computation) -> String {
    let CanonicalSourceError::InvalidEffect(errors) = value.validate_structure().unwrap_err()
    else {
        panic!("expected a structural diagnostic")
    };
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].code, "LSH1405");
    assert_eq!(
        errors[0].span,
        Some(leselang_syntax::Span { start: 0, end: 0 })
    );
    errors[0].message.clone()
}

#[test]
fn node_limit_is_inclusive_and_folded_constructors_still_spend_source_nodes() {
    let mut exact = wrap(tree(MAX_COMPUTATION_NODES / 2), 1);
    exact.validate_structure().unwrap();
    assert_eq!(
        error(&wrap(exact.clone(), 1)),
        "invalid or oversized computation"
    );
    // Replacing a physical leaf with one folded source child crosses the same limit.
    for value in [
        ScalarValue::OptionalString(OptionalStringValue(None)),
        ScalarValue::StringList(StringListValue(vec![String::new()])),
    ] {
        replace_leaf(&mut exact, value);
        assert_eq!(error(&exact), "invalid or oversized computation");
    }
    replace_leaf(&mut exact, ScalarValue::StringList(StringListValue(vec![])));
    exact.validate_structure().unwrap();
}

#[test]
fn depth_is_inclusive_and_folded_nonempty_values_keep_their_extra_level() {
    wrap(integer(), MAX_EFFECT_NESTING_DEPTH)
        .validate_structure()
        .unwrap();
    assert_eq!(
        error(&wrap(integer(), MAX_EFFECT_NESTING_DEPTH + 1)),
        "invalid or oversized computation"
    );
    for value in [
        ScalarValue::OptionalString(OptionalStringValue(None)),
        ScalarValue::StringList(StringListValue(vec![String::new()])),
    ] {
        let literal = Computation::Literal { value };
        wrap(literal.clone(), MAX_EFFECT_NESTING_DEPTH - 1)
            .validate_structure()
            .unwrap();
        assert_eq!(
            error(&wrap(literal, MAX_EFFECT_NESTING_DEPTH)),
            "invalid or oversized computation"
        );
    }
    wrap(
        Computation::Literal {
            value: ScalarValue::StringList(StringListValue(vec![])),
        },
        MAX_EFFECT_NESTING_DEPTH,
    )
    .validate_structure()
    .unwrap();
}

#[test]
fn prepared_inspection_counts_pending_pure_nodes_and_the_host_wrapper() {
    let prepared = |value| Computation::Bind {
        name: "n".into(),
        value: Box::new(value),
        body: Box::new(focus()),
    };
    let exact = prepared(tree((MAX_COMPUTATION_NODES - 2) / 2));
    exact.validate_structure().unwrap();
    assert_eq!(
        exact.prepared_atomic_operation(),
        Some(HostOperation::UiFocus)
    );
    let excessive = prepared(wrap(tree((MAX_COMPUTATION_NODES - 2) / 2), 1));
    assert_eq!(excessive.prepared_atomic_operation(), None);
    assert_eq!(error(&excessive), "invalid or oversized computation");

    let mut nested = focus();
    for _ in 0..MAX_EFFECT_NESTING_DEPTH - 1 {
        nested = prepared_with_body(nested);
    }
    assert_eq!(
        nested.prepared_atomic_operation(),
        Some(HostOperation::UiFocus)
    );
    nested.validate_structure().unwrap();
    nested = prepared_with_body(nested);
    assert_eq!(nested.prepared_atomic_operation(), None);
    assert_eq!(
        error(&nested),
        "invalid or oversized computation host graph"
    );
}

fn prepared_with_body(body: Computation) -> Computation {
    Computation::Bind {
        name: "n".into(),
        value: Box::new(integer()),
        body: Box::new(body),
    }
}

#[test]
fn cold_selection_branches_cannot_hide_bad_signatures_or_oversized_preparation() {
    for otherwise in [
        Computation::Host {
            effect: Box::new(Effect::UiAssertVisible {
                node_id: "target".into(),
            }),
        },
        Computation::Bind {
            name: "n".into(),
            value: Box::new(tree(MAX_COMPUTATION_NODES / 2)),
            body: Box::new(focus()),
        },
    ] {
        let selected = Computation::Choose {
            when: Box::new(Computation::Literal {
                value: ScalarValue::Boolean(true),
            }),
            then: Box::new(focus()),
            otherwise: Box::new(otherwise),
        };
        assert_eq!(selected.prepared_atomic_operation(), None);
    }
}

#[test]
fn host_shape_rejects_nested_computations_without_claiming_full_type_validation() {
    let hidden = Computation::Host {
        effect: Box::new(Effect::Compute {
            expression: Box::new(integer()),
        }),
    };
    assert_eq!(
        error(&hidden),
        "invalid or oversized computation host graph"
    );
    assert_eq!(hidden.prepared_atomic_operation(), None);
    let type_invalid = wrap(integer(), 1);
    type_invalid.validate_structure().unwrap();
    assert!(type_invalid.validate_in_scope(&[]).is_err());
}
