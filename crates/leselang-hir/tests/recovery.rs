use leselang_hir::computation::{Computation, ScalarType, ScalarValue};
use leselang_hir::{Effect, Type, authorize, canonical_source, lower};
use leselang_host_contract::CapabilitySet;
use leselang_syntax::{format, parse};

#[test]
fn recovery_is_typed_pure_and_canonical_with_order_independent_named_arguments() {
    for (expression, expected) in [
        (
            r#"recover(value: parse_integer(value: "bad"), fallback: 7)"#,
            ScalarType::Integer,
        ),
        (
            r#"recover(fallback: false, value: parse_boolean(value: "bad"))"#,
            ScalarType::Boolean,
        ),
        (
            r#"recover(value: to_string(value: div(left: 1, right: 0)), fallback: "missing")"#,
            ScalarType::String,
        ),
        ("recover(value: none, fallback: none)", ScalarType::None),
        (
            "bind(n: 7, body: recover(value: div(left: n, right: 0), fallback: n))",
            ScalarType::Integer,
        ),
    ] {
        let syntax = parse(&format!("fn main() = {expression}"));
        let program = lower(&syntax).unwrap();
        assert_eq!(program.function.result_type, Type::Scalar(expected));
        assert!(program.function.required_capabilities.is_empty());
        authorize(&program, &CapabilitySet::default()).unwrap();
        let formatted = format(&syntax).unwrap();
        assert_eq!(format(&parse(&formatted)).unwrap(), formatted);
        assert_eq!(
            lower(&parse(&canonical_source(&program.function.effect).unwrap())).unwrap(),
            program
        );
    }
}

#[test]
fn recovery_checks_cold_fallback_types_scope_shape_and_effects() {
    for expression in [
        "recover()",
        "recover(value: 1)",
        "recover(value: 1, value: 2)",
        "recover(value: 1, fallback: 2, extra: 3)",
        "recover(value: 1, fallback: false)",
        "recover(value: 1, fallback: missing)",
        "recover(value: bind(n: 1, body: n), fallback: n)",
        "recover(value: 1, fallback: bind(r: runtime.list(), body: 2))",
        "recover(value: bind(r: runtime.list(), body: 1), fallback: 2)",
        "recover(value: runtime.list(), fallback: runtime.list())",
        "bind(r: runtime.list(), body: recover(value: r, fallback: r))",
        "seq(first: recover(value: 1, fallback: 0))",
        "repeat(times: 2, body: recover(value: runtime.list(), fallback: runtime.list()))",
        "choose(when: true, then: 1, otherwise: recover(value: 2, fallback: false))",
    ] {
        let errors = lower(&parse(&format!("fn main() = {expression}"))).unwrap_err();
        assert!(
            errors.iter().all(|error| error.span.is_some()),
            "{expression}"
        );
    }
    for expression in [
        "recover(value: 1, fallback: false)",
        "recover(value: 1, fallback: bind(r: runtime.list(), body: 2))",
    ] {
        assert_eq!(
            lower(&parse(&format!("fn main() = {expression}"))).unwrap_err()[0].code,
            "LSH1411"
        );
    }
}

#[test]
fn recovery_after_host_results_preserves_capability_preflight_and_group_boundaries() {
    let program = lower(&parse(r#"fn main() = bind(r: runtime.list(), body:
        ui.focus(node_id: to_string(value: recover(value: div(left: 1, right: field(value: r, name: "count")), fallback: 7))))"#)).unwrap();
    assert_eq!(
        program.function.required_capabilities,
        ["runtime.read", "ui.presentation"]
    );
    assert!(authorize(&program, &CapabilitySet::new(["runtime.read"])).is_err());
    authorize(
        &program,
        &CapabilitySet::new(["runtime.read", "ui.presentation"]),
    )
    .unwrap();
    let cold = lower(&parse(
        r#"fn main() = choose(
        when: recover(value: parse_boolean(value: "bad"), fallback: true),
        then: runtime.list(), otherwise: ui.focus(node_id: "a"))"#,
    ));
    assert!(cold.is_err());
    let cold = lower(&parse(r#"fn main() = choose(
        when: recover(value: parse_boolean(value: "bad"), fallback: true),
        then: bind(r: runtime.list(), body: true), otherwise: bind(s: ui.focus(node_id: "a"), body: false))"#)).unwrap();
    assert!(authorize(&cold, &CapabilitySet::new(["runtime.read"])).is_err());
    for source in [
        r#"seq(a: ui.focus(node_id: to_string(value: recover(value: parse_integer(value: "bad"), fallback: 1))))"#,
        r#"all(a: ui.focus(node_id: to_string(value: recover(value: parse_integer(value: "bad"), fallback: 1))), b: ui.focus(node_id: "b"))"#,
        r#"repeat(times: 2, body: ui.focus(node_id: to_string(value: recover(value: parse_integer(value: "bad"), fallback: 1))))"#,
    ] {
        let program = lower(&parse(&format!("fn main() = {source}"))).unwrap();
        assert_eq!(
            lower(&parse(&canonical_source(&program.function.effect).unwrap())).unwrap(),
            program
        );
    }
}

#[test]
fn forged_recovery_hir_cannot_hide_oversized_fallbacks_or_host_operations() {
    let literal = || Computation::Literal {
        value: ScalarValue::Integer(1),
    };
    for fallback in [
        Computation::Literal {
            value: ScalarValue::String("x".repeat(4097)),
        },
        Computation::Literal {
            value: ScalarValue::Boolean(false),
        },
        Computation::Host {
            effect: Box::new(
                lower(&parse("fn main() = runtime.list()"))
                    .unwrap()
                    .function
                    .effect,
            ),
        },
    ] {
        let effect = Effect::Compute {
            expression: Box::new(Computation::Recover {
                value: Box::new(literal()),
                fallback: Box::new(fallback),
            }),
        };
        assert!(canonical_source(&effect).is_err());
    }
    let mut deep = literal();
    for _ in 0..64 {
        deep = Computation::Recover {
            value: Box::new(literal()),
            fallback: Box::new(deep),
        };
    }
    assert!(
        canonical_source(&Effect::Compute {
            expression: Box::new(deep)
        })
        .is_err()
    );
}
