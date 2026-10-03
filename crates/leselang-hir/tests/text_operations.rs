use leselang_hir::computation::{BinaryOperator, Computation, ScalarType, ScalarValue};
use leselang_hir::{Effect, Type, authorize, canonical_source, lower};
use leselang_host_contract::CapabilitySet;
use leselang_syntax::{format, parse};

#[test]
fn text_operations_have_explicit_types_and_canonical_round_trips() {
    for (body, expected) in [
        (
            r#"contains(left: "ready", right: "ead")"#,
            ScalarType::Boolean,
        ),
        (
            r#"starts_with(right: "re", left: "ready")"#,
            ScalarType::Boolean,
        ),
        (
            r#"ends_with(left: "ready", right: "dy")"#,
            ScalarType::Boolean,
        ),
        (
            r#"char_at(left: "ready", right: 2)"#,
            ScalarType::OptionalString,
        ),
        (
            r#"char_at(left: "", right: 18446744073709551615)"#,
            ScalarType::OptionalString,
        ),
        (
            r#"value_or(left: char_at(left: "text", right: 1), right: "default")"#,
            ScalarType::String,
        ),
        (
            r#"loop(index: 0, while: has_value(value: char_at(left: "ab", right: index)), next: add(left: index, right: 1), limit: 2)"#,
            ScalarType::Integer,
        ),
    ] {
        let syntax = parse(&format!("fn main() = {body}"));
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
fn text_operands_are_pure_and_typed_even_in_cold_branches() {
    for operation in ["contains", "starts_with", "ends_with"] {
        for body in [
            format!(r#"{operation}(left: "text", right: 1)"#),
            format!(r#"{operation}(left: false, right: "text")"#),
            format!(r#"{operation}(left: optional_string(value: "text"), right: "t")"#),
            format!(r#"{operation}(left: "text", right: none)"#),
            format!(r#"{operation}(left: "text", right: runtime.list())"#),
            format!(
                r#"{operation}(left: "text", right: bind(r: ui.focus(node_id: "a"), body: "t"))"#
            ),
            format!(r#"{operation}(value: "text", right: "t")"#),
            format!(r#"{operation}(left: "text", right: "t", right: "e")"#),
            format!(r#"{operation}(left: "text", right: "t", extra: "e")"#),
        ] {
            let source = format!("fn main() = choose(when: true, then: false, otherwise: {body})");
            let diagnostics = lower(&parse(&source)).unwrap_err();
            assert!(
                diagnostics
                    .iter()
                    .all(|diagnostic| diagnostic.span.is_some()),
                "{source}"
            );
        }
    }
    for body in [
        r#"char_at(left: "text", right: "1")"#,
        r#"char_at(left: "text", right: false)"#,
        r#"char_at(left: none, right: 1)"#,
        r#"char_at(left: optional_string(value: "text"), right: 1)"#,
        r#"char_at(left: ui.focus(node_id: "a"), right: 1)"#,
        r#"char_at(left: "text", index: 1)"#,
        r#"contains(left: char_at(left: "text", right: 0), right: "t")"#,
        r#"ui.set_form_value(node_id: "form", field: "name", value: char_at(left: "text", right: 0))"#,
    ] {
        assert!(
            lower(&parse(&format!("fn main() = {body}"))).is_err(),
            "{body}"
        );
    }
}

#[test]
fn pure_helpers_and_host_decisions_share_the_existing_authority_boundary() {
    let source = r#"fn first(text: string) = value_or(left: char_at(left: text, right: 0), right: "default")
        fn ready(text: string) = and(left: starts_with(left: text, right: "ready"), right: ends_with(left: text, right: "!"))
        fn main() = bind(result: ui.assert_text(node_id: "status", expected: "ready!"), body:
            choose(when: ready(text: field(value: result, name: "expected")),
                then: ui.set_form_value(node_id: "form", field: "prefix", value: first(text: field(value: result, name: "expected"))),
                otherwise: ui.set_form_value(node_id: "form", field: "prefix", value: "fallback")))"#;
    let program = lower(&parse(source)).unwrap();
    assert_eq!(program.function.required_capabilities, ["ui.presentation"]);
    assert!(authorize(&program, &CapabilitySet::default()).is_err());
    authorize(&program, &CapabilitySet::new(["ui.presentation"])).unwrap();
    assert_eq!(
        lower(&parse(&canonical_source(&program.function.effect).unwrap())).unwrap(),
        program
    );
    for name in ["contains", "starts_with", "ends_with", "char_at"] {
        assert!(
            lower(&parse(&format!(
                "fn {name}(text: string) = text\nfn main() = 0"
            )))
            .is_err()
        );
    }
}

#[test]
fn source_and_deserialized_hir_limits_apply_before_text_shortening() {
    for operator in [
        BinaryOperator::Contains,
        BinaryOperator::StartsWith,
        BinaryOperator::EndsWith,
        BinaryOperator::CharAt,
    ] {
        let right = if operator == BinaryOperator::CharAt {
            ScalarValue::Integer(0)
        } else {
            ScalarValue::String("".into())
        };
        let effect = Effect::Compute {
            expression: Box::new(Computation::Binary {
                operator,
                left: Box::new(Computation::Literal {
                    value: ScalarValue::String("x".repeat(4097)),
                }),
                right: Box::new(Computation::Literal { value: right }),
            }),
        };
        assert!(canonical_source(&effect).is_err());
    }
    let text = format!("\"{}\"", "x".repeat(4096));
    let program = lower(&parse(&format!(
        "fn main() = char_at(left: {text}, right: 0)"
    )))
    .unwrap();
    assert_eq!(
        program.function.result_type,
        Type::Scalar(ScalarType::OptionalString)
    );
}
