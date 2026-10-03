use leselang_hir::computation::{Computation, ScalarType, ScalarValue};
use leselang_hir::{Effect, Type, authorize, canonical_source, lower};
use leselang_host_contract::CapabilitySet;
use leselang_syntax::parse;

#[test]
fn typed_computation_roundtrips_without_product_authority() {
    let legacy = lower(&parse(
        "fn true() = all(true: runtime.list(), false: runtime.list())",
    ))
    .unwrap();
    assert_eq!(legacy.function.name, "true");
    assert_eq!(
        lower(&parse(&canonical_source(&legacy.function.effect).unwrap()))
            .unwrap()
            .function
            .effect,
        legacy.function.effect
    );
    for (expression, ty) in [
        (
            "bind(count: add(left: 2, right: 3), body: choose(when: ge(left: count, right: 5), then: mul(left: count, right: 2), otherwise: 0))",
            ScalarType::Integer,
        ),
        ("eq(right: none, left: none)", ScalarType::Boolean),
        ("concat(right: \"b\", left: \"a\")", ScalarType::String),
        (
            "choose(when: false, then: none, otherwise: none)",
            ScalarType::None,
        ),
        (
            "bind(body: add(left: count, right: 1), count: 2)",
            ScalarType::Integer,
        ),
        (
            "bind(n: 2, body: bind(m: add(left: n, right: 1), body: m))",
            ScalarType::Integer,
        ),
    ] {
        let program = lower(&parse(&format!("fn main() = {expression}"))).unwrap();
        assert_eq!(program.function.result_type, Type::Scalar(ty));
        assert!(program.function.required_capabilities.is_empty());
        authorize(&program, &CapabilitySet::default()).unwrap();
        let source = canonical_source(&program.function.effect).unwrap();
        assert_eq!(lower(&parse(&source)).unwrap(), program);
    }
}

#[test]
fn scalar_types_scopes_and_call_shapes_are_checked_before_execution() {
    for expression in [
        "add(left: true, right: 2)",
        "eq(left: 1, right: \"1\")",
        "not(value: 1)",
        "len(value: none)",
        "choose(when: 1, then: 2, otherwise: 3)",
        "choose(when: true, then: 1, otherwise: false)",
        "choose(when: true, then: 1, otherwise: missing)",
        "bind(n: n, body: n)",
        "bind(n: 1, body: bind(n: 2, body: n))",
        "bind(body: 1, body: 2)",
        "bind(n: 1, m: 2, body: n)",
        "missing",
        "add(left: 1, left: 2)",
        "add(left: 1, right: 2, extra: 0)",
        "choose(when: true, then: 1)",
        "and(left: true, right: runtime.list())",
        "bind(n: runtime.list(), body: n)",
        "seq(a: add(left: 1, right: 2))",
        "all(a: true, b: false)",
        "repeat(times: 2, body: choose(when: true, then: ui.focus(node_id: \"a\"), otherwise: bind(r: ui.focus(node_id: \"b\"), body: ui.focus(node_id: \"c\"))))",
    ] {
        let error = lower(&parse(&format!("fn main() = {expression}"))).unwrap_err();
        assert!(
            error.iter().all(|error| error.span.is_some()),
            "{expression}"
        );
    }
}

#[test]
fn all_possible_host_branches_contribute_capabilities() {
    let program = lower(&parse(
        r#"fn main() = choose(when: true,
        then: seq(read: runtime.list()),
        otherwise: seq(mutate: runtime.refresh(runtime_id: "a")))"#,
    ))
    .unwrap();
    assert_eq!(
        program.function.required_capabilities,
        ["runtime.read", "runtime.refresh"]
    );
    assert_eq!(
        authorize(&program, &CapabilitySet::new(["runtime.read"]))
            .unwrap_err()
            .code,
        "LSH2001"
    );
    authorize(
        &program,
        &CapabilitySet::new(["runtime.read", "runtime.refresh"]),
    )
    .unwrap();
    let mut forged = program.clone();
    forged.function.required_capabilities = vec!["runtime.read".into()];
    assert_eq!(
        authorize(&forged, &CapabilitySet::new(["runtime.read"]))
            .unwrap_err()
            .code,
        "LSH2002"
    );
    let source = canonical_source(&program.function.effect).unwrap();
    assert_eq!(lower(&parse(&source)).unwrap(), program);
}

#[test]
fn malformed_computation_hir_and_result_metadata_are_rejected() {
    let mut program = lower(&parse("fn main() = 1")).unwrap();
    program.function.result_type = Type::Scalar(ScalarType::Boolean);
    assert!(authorize(&program, &CapabilitySet::default()).is_err());
    program.function.effect = Effect::Compute {
        expression: Box::new(Computation::Local {
            name: "unknown".into(),
        }),
    };
    assert!(canonical_source(&program.function.effect).is_err());
    program.function.effect = Effect::Compute {
        expression: Box::new(Computation::Host {
            effect: Box::new(Effect::Compute {
                expression: Box::new(Computation::Literal {
                    value: ScalarValue::Integer(1),
                }),
            }),
        }),
    };
    assert!(canonical_source(&program.function.effect).is_err());
}

#[test]
fn computation_node_name_string_and_depth_limits_apply_before_runtime() {
    let mut expression = "1".to_string();
    for _ in 0..10 {
        expression = format!("add(left: {expression}, right: {expression})");
    }
    assert!(lower(&parse(&format!("fn main() = {expression}"))).is_err());
    let steps = (0..64)
        .map(|index| format!("step_{index}: runtime.list()"))
        .collect::<Vec<_>>()
        .join(", ");
    let mut grouped = format!("seq({steps})");
    for _ in 0..4 {
        grouped = format!("choose(when: true, then: {grouped}, otherwise: {grouped})");
    }
    assert!(
        lower(&parse(&format!("fn main() = {grouped}")))
            .unwrap_err()
            .iter()
            .any(|error| error.code == "LSH1405")
    );
    for length in [4096, 4097] {
        assert_eq!(
            lower(&parse(&format!("fn main() = \"{}\"", "x".repeat(length)))).is_ok(),
            length == 4096
        );
    }
    assert!(
        lower(&parse(&format!(
            "fn main() = bind({}: 1, body: 0)",
            "x".repeat(65)
        )))
        .is_err()
    );
    let mut expression = Computation::Literal {
        value: ScalarValue::Boolean(true),
    };
    for _ in 0..32 {
        expression = Computation::Unary {
            operator: leselang_hir::computation::UnaryOperator::Not,
            value: Box::new(expression),
        };
    }
    assert!(
        canonical_source(&Effect::Compute {
            expression: Box::new(expression)
        })
        .is_err()
    );
}
