use leselang_hir::computation::{Computation, MAX_LOOP_ITERATIONS, ScalarType, ScalarValue};
use leselang_hir::{Effect, Type, authorize, canonical_source, lower};
use leselang_host_contract::CapabilitySet;
use leselang_syntax::{format, parse};

#[test]
fn loops_are_typed_pure_expressions_with_canonical_roundtrips() {
    for (expression, ty) in [
        (
            "loop(count: 0, while: lt(left: count, right: 3), next: add(left: count, right: 1), limit: 3)",
            ScalarType::Integer,
        ),
        (
            "loop(next: false, limit: 1, ready: true, while: ready)",
            ScalarType::Boolean,
        ),
        (
            "loop(text: \"\", while: lt(left: len(value: text), right: 3), next: concat(left: text, right: \"x\"), limit: 3)",
            ScalarType::String,
        ),
        (
            "loop(unit: none, while: false, next: unit, limit: 0)",
            ScalarType::None,
        ),
        (
            "bind(target: 4, body: loop(n: 0, while: lt(left: n, right: target), next: add(left: n, right: 1), limit: 8))",
            ScalarType::Integer,
        ),
    ] {
        let tree = parse(&format!("fn main() = {expression}"));
        assert!(tree.diagnostics.is_empty());
        let formatted = format(&tree).unwrap();
        assert_eq!(format(&parse(&formatted)).unwrap(), formatted);
        let program = lower(&tree).unwrap();
        assert_eq!(program.function.result_type, Type::Scalar(ty));
        assert!(program.function.required_capabilities.is_empty());
        authorize(&program, &CapabilitySet::default()).unwrap();
        assert_eq!(
            lower(&parse(&canonical_source(&program.function.effect).unwrap())).unwrap(),
            program
        );
    }
}

#[test]
fn loop_state_scope_and_types_are_checked_even_for_zero_or_cold_iterations() {
    for expression in [
        "loop(n: n, while: false, next: n, limit: 0)",
        "loop(n: 0, while: n, next: n, limit: 0)",
        "loop(n: 0, while: false, next: false, limit: 0)",
        "loop(n: 0, while: false, next: missing, limit: 0)",
        "bind(n: 0, body: loop(n: 1, while: false, next: n, limit: 0))",
        "loop(n: 0, while: true, next: bind(n: 1, body: n), limit: 1)",
        "loop(n: 0, while: true, next: loop(n: 1, while: false, next: n, limit: 0), limit: 1)",
        "add(left: loop(n: 0, while: false, next: n, limit: 0), right: n)",
        "choose(when: true, then: 1, otherwise: loop(n: 0, while: false, next: \"wrong\", limit: 0))",
    ] {
        let errors = lower(&parse(&format!("fn main() = {expression}"))).unwrap_err();
        assert!(
            errors.iter().all(|error| error.span.is_some()),
            "{expression}"
        );
    }
}

#[test]
fn loop_call_shape_and_literal_limits_are_explicit() {
    for expression in [
        "loop(n: 0, while: false, next: n)",
        "loop(n: 0, while: false, next: n, limit: 1, extra: 0)",
        "loop(n: 0, while: false, next: n, next: n)",
        "loop(n: 0, n: 1, next: n, limit: 1)",
        "loop(while: false, next: 1, limit: 1)",
        "loop(body: 0, while: false, next: 0, limit: 1)",
        "loop(n: 0, while: false, next: n, limit: 1025)",
        "loop(n: 0, while: false, next: n, limit: 18446744073709551615)",
        "loop(n: 0, while: false, next: n, limit: true)",
        "loop(n: 0, while: false, next: n, limit: add(left: 1, right: 1))",
        "bind(cap: 1, body: loop(n: 0, while: false, next: n, limit: cap))",
    ] {
        assert!(
            lower(&parse(&format!("fn main() = {expression}"))).is_err(),
            "{expression}"
        );
    }
    for limit in [0, MAX_LOOP_ITERATIONS] {
        let program = lower(&parse(&format!(
            "fn main() = loop(n: 0, while: false, next: n, limit: {limit})"
        )))
        .unwrap();
        let Effect::Compute { expression } = &program.function.effect else {
            panic!("expected computation")
        };
        assert!(
            matches!(expression.as_ref(), Computation::Loop { limit: actual, .. } if *actual == limit)
        );
        assert!(canonical_source(&program.function.effect).unwrap().len() < 128);
    }
}

#[test]
fn loops_cannot_hide_effects_in_any_component_or_become_effect_group_members() {
    for expression in [
        "loop(n: bind(r: runtime.list(), body: 0), while: false, next: n, limit: 0)",
        "loop(n: 0, while: bind(r: runtime.list(), body: false), next: n, limit: 0)",
        "loop(n: 0, while: false, next: bind(r: runtime.list(), body: 0), limit: 0)",
        "loop(n: 0, while: false, next: choose(when: false, then: bind(r: runtime.list(), body: 0), otherwise: n), limit: 0)",
        "loop(r: runtime.list(), while: false, next: r, limit: 0)",
        "seq(a: loop(n: 0, while: false, next: n, limit: 0))",
        "all(a: runtime.list(), b: loop(n: 0, while: false, next: n, limit: 0))",
    ] {
        assert!(
            lower(&parse(&format!("fn main() = {expression}"))).is_err(),
            "{expression}"
        );
    }
}

#[test]
fn result_bound_loops_preserve_declared_host_authority_and_scalar_result_type() {
    let program = lower(&parse(
        r#"fn main() = bind(r: runtime.list(), body:
        loop(n: 1, while: lt(left: n, right: field(value: r, name: "revision")),
            next: mul(left: n, right: 2), limit: 64))"#,
    ))
    .unwrap();
    assert_eq!(
        program.function.result_type,
        Type::Scalar(ScalarType::Integer)
    );
    assert_eq!(program.function.required_capabilities, ["runtime.read"]);
    assert!(authorize(&program, &CapabilitySet::default()).is_err());
    authorize(&program, &CapabilitySet::new(["runtime.read"])).unwrap();
    assert_eq!(
        lower(&parse(&canonical_source(&program.function.effect).unwrap())).unwrap(),
        program
    );
}

#[test]
fn loop_structure_bounds_cover_initial_condition_and_next_without_unrolling() {
    let mut large = Computation::Literal {
        value: ScalarValue::Integer(0),
    };
    for _ in 0..10 {
        large = Computation::Binary {
            operator: leselang_hir::computation::BinaryOperator::Add,
            left: Box::new(large.clone()),
            right: Box::new(large),
        };
    }
    for oversized_part in 0..3 {
        let mut expression = Computation::Loop {
            name: "n".into(),
            initial: Box::new(Computation::Literal {
                value: ScalarValue::Integer(0),
            }),
            condition: Box::new(Computation::Literal {
                value: ScalarValue::Boolean(false),
            }),
            next: Box::new(Computation::Local { name: "n".into() }),
            limit: 0,
        };
        let Computation::Loop {
            initial,
            condition,
            next,
            ..
        } = &mut expression
        else {
            unreachable!()
        };
        match oversized_part {
            0 => **initial = large.clone(),
            1 => **condition = large.clone(),
            _ => **next = large.clone(),
        }
        assert!(expression.validate_structure().is_err());
    }
    let mut deep = Computation::Literal {
        value: ScalarValue::Integer(0),
    };
    for index in 0..32 {
        deep = Computation::Loop {
            name: format!("n{index}"),
            initial: Box::new(deep),
            condition: Box::new(Computation::Literal {
                value: ScalarValue::Boolean(false),
            }),
            next: Box::new(Computation::Local {
                name: format!("n{index}"),
            }),
            limit: 0,
        };
    }
    assert!(deep.validate_structure().is_err());
    assert!(
        canonical_source(&Effect::Compute {
            expression: Box::new(deep)
        })
        .is_err()
    );
}

#[test]
fn malformed_native_loop_hir_is_rejected_before_evaluation_or_serialization() {
    let mut program = lower(&parse(
        "fn main() = loop(n: 0, while: false, next: n, limit: 1)",
    ))
    .unwrap();
    let original = program.function.effect.clone();
    for case in 0..6 {
        program.function.effect = original.clone();
        let Effect::Compute { expression } = &mut program.function.effect else {
            unreachable!()
        };
        let Computation::Loop {
            name, next, limit, ..
        } = expression.as_mut()
        else {
            unreachable!()
        };
        match case {
            0 => *limit = MAX_LOOP_ITERATIONS + 1,
            1 => *name = "while".into(),
            2 => *name = "x".repeat(65),
            3 => {
                **next = Computation::Literal {
                    value: ScalarValue::Boolean(false),
                }
            }
            4 => {
                **next = Computation::Local {
                    name: "missing".into(),
                }
            }
            _ => {
                **next = Computation::Host {
                    effect: Box::new(Effect::RuntimeList {
                        filter: Default::default(),
                    }),
                }
            }
        }
        assert!(
            canonical_source(&program.function.effect).is_err(),
            "case {case}"
        );
        assert!(authorize(&program, &CapabilitySet::default()).is_err());
    }
}
