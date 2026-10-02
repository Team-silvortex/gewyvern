use leselang_hir::computation::{Computation, ScalarType, ScalarValue, UnaryOperator};
use leselang_hir::{Effect, Type, authorize, canonical_source, lower};
use leselang_host_contract::CapabilitySet;
use leselang_syntax::{format, parse};

#[test]
fn explicit_conversions_have_canonical_types_without_host_authority() {
    for (expression, expected) in [
        ("to_string(value: 42)", ScalarType::String),
        ("to_string(value: true)", ScalarType::String),
        (r#"to_string(value: "same")"#, ScalarType::String),
        (r#"parse_integer(value: "00042")"#, ScalarType::Integer),
        (r#"parse_boolean(value: "false")"#, ScalarType::Boolean),
        (
            r#"bind(text: "42", body: to_string(value: add(left: parse_integer(value: text), right: 1)))"#,
            ScalarType::String,
        ),
        (
            r#"loop(n: 0, while: lt(left: n, right: parse_integer(value: "2")), next: add(left: n, right: 1), limit: 2)"#,
            ScalarType::Integer,
        ),
    ] {
        let syntax = parse(&format!("fn main() = {expression}"));
        let program = lower(&syntax).unwrap();
        assert_eq!(program.function.result_type, Type::Scalar(expected));
        assert!(program.function.required_capabilities.is_empty());
        authorize(&program, &CapabilitySet::default()).unwrap();
        assert_eq!(
            lower(&parse(&canonical_source(&program.function.effect).unwrap())).unwrap(),
            program
        );
        let formatted = format(&syntax).unwrap();
        assert_eq!(format(&parse(&formatted)).unwrap(), formatted);
    }
}

#[test]
fn conversion_types_shapes_and_effectful_operands_fail_before_dispatch() {
    for expression in [
        "to_string(value: none)",
        "parse_integer(value: 42)",
        "parse_boolean(value: true)",
        "parse_integer(value: none)",
        "parse_boolean(value: none)",
        "to_string()",
        "to_string(text: 42)",
        "to_string(value: 1, value: 2)",
        "parse_integer(value: \"2\", extra: 1)",
        "to_string(value: runtime.list())",
        "parse_integer(value: bind(r: ui.focus(node_id: \"a\"), body: \"2\"))",
        "to_string(value: bind(r: runtime.list(), body: 2))",
        "bind(r: runtime.list(), body: to_string(value: r))",
        "choose(when: true, then: \"ok\", otherwise: to_string(value: none))",
        "eq(left: 1, right: \"1\")",
        "concat(left: \"n=\", right: 1)",
        "ui.focus(node_id: parse_integer(value: \"42\"))",
    ] {
        let diagnostics = lower(&parse(&format!("fn main() = {expression}"))).unwrap_err();
        assert!(
            diagnostics
                .iter()
                .all(|diagnostic| diagnostic.span.is_some()),
            "{expression}"
        );
    }
}

#[test]
fn conversions_connect_typed_projections_to_host_arguments_without_skipping_preflight() {
    let program = lower(&parse(r#"fn main() = bind(r: runtime.list(), body:
        ui.set_form_value(node_id: "form", field: "count", value: to_string(value: field(value: r, name: "count"))))"#)).unwrap();
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
        r#"fn main() = choose(when: parse_boolean(value: "true"),
        then: runtime.list(role: to_string(value: 7)), otherwise: runtime.list())"#,
    ))
    .unwrap();
    assert!(authorize(&cold, &CapabilitySet::default()).is_err());
}

#[test]
fn serialized_conversion_hir_cannot_bypass_types_or_existing_bounds() {
    for (operator, value) in [
        (UnaryOperator::ToString, ScalarValue::None),
        (UnaryOperator::ParseInteger, ScalarValue::Boolean(true)),
        (UnaryOperator::ParseBoolean, ScalarValue::Integer(1)),
        (
            UnaryOperator::ToString,
            ScalarValue::String("x".repeat(4097)),
        ),
    ] {
        let effect = Effect::Compute {
            expression: Box::new(Computation::Unary {
                operator,
                value: Box::new(Computation::Literal { value }),
            }),
        };
        assert!(canonical_source(&effect).is_err());
    }
}
