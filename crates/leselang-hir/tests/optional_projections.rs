use leselang_hir::computation::{Computation, OptionalStringValue, ScalarType, ScalarValue};
use leselang_hir::result_field::ResultField;
use leselang_hir::{Type, canonical_source, lower};
use leselang_syntax::parse;

#[test]
fn optional_constructors_helpers_and_defaults_round_trip_without_implicit_lifting() {
    for (source, ty) in [
        (
            "fn main() = optional_string(value: none)",
            ScalarType::OptionalString,
        ),
        (
            r#"fn main() = optional_string(value: "")"#,
            ScalarType::OptionalString,
        ),
        (
            r#"fn main() = has_value(value: optional_string(value: ""))"#,
            ScalarType::Boolean,
        ),
        (
            r#"fn main() = value_or(left: optional_string(value: none), right: "default")"#,
            ScalarType::String,
        ),
        (
            r#"fn defaulted(value: optional_string) = value_or(left: value, right: "default")
            fn main() = defaulted(value: optional_string(value: none))"#,
            ScalarType::String,
        ),
        (
            r#"fn wrap(value: string) = optional_string(value: value)
            fn main() = wrap(value: "value")"#,
            ScalarType::OptionalString,
        ),
        (
            r#"fn main() = loop(value: optional_string(value: none), while: not(value: has_value(value: value)), next: optional_string(value: "ready"), limit: 1)"#,
            ScalarType::OptionalString,
        ),
    ] {
        let program = lower(&parse(source)).unwrap();
        assert_eq!(program.function.result_type, Type::Scalar(ty));
        assert_eq!(
            lower(&parse(&canonical_source(&program.function.effect).unwrap())).unwrap(),
            program
        );
    }
    let literal = Computation::Literal {
        value: ScalarValue::OptionalString(OptionalStringValue(Some("".into()))),
    };
    assert_eq!(
        literal.validate_in_scope(&[]).unwrap(),
        Type::Scalar(ScalarType::OptionalString)
    );
}

#[test]
fn optional_expected_has_an_exact_four_operation_table_and_frozen_v5_vocabulary() {
    for (operation, extra, ty) in [
        (
            "assert_form_field_placeholder",
            r#"field: "name", "#,
            Type::UiAssertFormFieldPlaceholder,
        ),
        (
            "wait_form_field_placeholder",
            r#"field: "name", "#,
            Type::UiWaitFormFieldPlaceholder,
        ),
        (
            "assert_action_unavailable_reason",
            "",
            Type::UiAssertActionUnavailableReason,
        ),
        (
            "wait_action_unavailable_reason",
            "",
            Type::UiWaitActionUnavailableReason,
        ),
    ] {
        assert_eq!(
            ResultField::OptionalExpected.result_type(ty),
            Some(ScalarType::OptionalString)
        );
        assert_eq!(ResultField::Expected.result_type(ty), None);
        let source = format!(
            r#"fn main() = bind(r: ui.{operation}(node_id: "a", {extra}expected: none), body: field(value: r, name: "optional_expected"))"#
        );
        let program = lower(&parse(&source)).unwrap();
        assert_eq!(
            program.function.result_type,
            Type::Scalar(ScalarType::OptionalString)
        );
        assert_eq!(program.function.required_capabilities, ["ui.presentation"]);
        assert_eq!(
            lower(&parse(&canonical_source(&program.function.effect).unwrap())).unwrap(),
            program
        );
    }
    for ty in [
        Type::UiFocus,
        Type::UiAssertText,
        Type::UiAssertFormValue,
        Type::UiAssertNodeKind,
        Type::RuntimeInspect,
    ] {
        assert_eq!(ResultField::OptionalExpected.result_type(ty), None);
    }
    assert_eq!(ResultField::V5[..ResultField::V4.len()], ResultField::V4);
    assert_eq!(ResultField::ALL, ResultField::V5);
}

#[test]
fn only_optional_text_host_arguments_accept_explicit_optional_values() {
    for source in [
        r#"fn main() = ui.assert_form_field_placeholder(node_id: "form", field: "name", expected: optional_string(value: none))"#,
        r#"fn main() = bind(r: ui.assert_action_unavailable_reason(node_id: "a", expected: none), body: ui.wait_action_unavailable_reason(node_id: "a", expected: field(value: r, name: "optional_expected")))"#,
        r#"fn main() = bind(g: all(a: ui.assert_form_field_placeholder(node_id: "form", field: "name", expected: none), b: ui.assert_text(node_id: "a", expected: "ready")), body: bind(r: ui.wait_form_field_placeholder(node_id: "form", field: "name", expected: field(value: member(value: g, name: "a"), name: "optional_expected")), body: value_or(left: field(value: r, name: "optional_expected"), right: field(value: member(value: g, name: "b"), name: "expected"))))"#,
    ] {
        let program = lower(&parse(source)).unwrap();
        assert_eq!(
            lower(&parse(&canonical_source(&program.function.effect).unwrap())).unwrap(),
            program
        );
    }
    for source in [
        r#"fn main() = ui.focus(node_id: optional_string(value: "a"))"#,
        r#"fn main() = runtime.list(role: optional_string(value: none))"#,
        r#"fn main() = ui.assert_text(node_id: "a", expected: optional_string(value: "text"))"#,
        r#"fn main() = ui.set_form_value(node_id: "form", field: "name", value: optional_string(value: "text"))"#,
    ] {
        assert!(lower(&parse(source)).is_err(), "{source}");
    }
}

#[test]
fn defaults_and_presence_checks_reject_wrong_types_effects_and_cold_implicit_coercions() {
    for source in [
        "fn main() = optional_string(value: 1)",
        "fn main() = has_value(value: none)",
        r#"fn main() = value_or(left: "text", right: "default")"#,
        "fn main() = value_or(left: optional_string(value: none), right: 0)",
        "fn main() = value_or(left: optional_string(value: none), right: runtime.list())",
        r#"fn main() = to_string(value: optional_string(value: "text"))"#,
        "fn main() = eq(left: optional_string(value: none), right: none)",
        r#"fn f(value: optional_string) = value
            fn main() = f(value: "text")"#,
        r#"fn main() = choose(when: true, then: "text", otherwise: optional_string(value: none))"#,
        r#"fn main() = choose(when: true, then: "text", otherwise: value_or(left: optional_string(value: "text"), right: ui.focus(node_id: "a")))"#,
    ] {
        assert!(lower(&parse(source)).is_err(), "{source}");
    }
}

#[test]
fn trusted_host_resolution_accepts_optional_text_only_and_preserves_original_domains() {
    use leselang_hir::host_call::HostOperation;
    for text in [None, Some(""), Some("hint")] {
        let arguments = [
            ("node_id".into(), ScalarValue::String("a".into())),
            (
                "expected".into(),
                ScalarValue::OptionalString(OptionalStringValue(text.map(str::to_owned))),
            ),
        ];
        let effect = HostOperation::UiAssertActionUnavailableReason
            .resolve(&arguments)
            .unwrap();
        assert!(
            matches!(effect, leselang_hir::Effect::UiAssertActionUnavailableReason { expected, .. } if expected.as_deref() == text)
        );
    }
    assert!(
        HostOperation::UiFocus
            .resolve(&[(
                "node_id".into(),
                ScalarValue::OptionalString(OptionalStringValue(Some("a".into())))
            )])
            .is_err()
    );
    for text in ["x".repeat(1025), "bad\ntext".into()] {
        assert!(
            HostOperation::UiAssertActionUnavailableReason
                .resolve(&[
                    ("node_id".into(), ScalarValue::String("a".into())),
                    (
                        "expected".into(),
                        ScalarValue::OptionalString(OptionalStringValue(Some(text)))
                    ),
                ])
                .is_err()
        );
    }
}

#[test]
fn folded_optional_helper_literals_reserve_their_canonical_constructor_depth() {
    let mut body = "has_value(value: empty())".to_string();
    let mut accepted = 0;
    let mut rejected = 0;
    for _ in 0..=leselang_hir::MAX_EFFECT_NESTING_DEPTH {
        let source = format!("fn empty() = optional_string(value: none)\nfn main() = {body}");
        match lower(&parse(&source)) {
            Ok(program) => {
                accepted += 1;
                assert_eq!(
                    lower(&parse(&canonical_source(&program.function.effect).unwrap())).unwrap(),
                    program
                );
            }
            Err(errors) => {
                rejected += 1;
                assert!(
                    errors
                        .iter()
                        .any(|error| matches!(error.code.as_str(), "LSH1405" | "LSE1110")),
                    "{errors:?}"
                );
            }
        }
        body = format!("not(value: {body})");
    }
    assert!(accepted > 0 && rejected > 0);
}
