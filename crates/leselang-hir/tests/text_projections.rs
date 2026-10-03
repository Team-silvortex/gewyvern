use leselang_hir::computation::ScalarType;
use leselang_hir::result_field::ResultField;
use leselang_hir::{Type, canonical_source, lower};
use leselang_syntax::parse;

#[test]
fn confirmed_expected_text_is_a_string_only_for_nonnullable_text_receipts() {
    for operation in [
        "assert_text",
        "wait_text",
        "assert_automation_id",
        "wait_automation_id",
        "assert_action_label",
        "wait_action_label",
        "assert_form_value",
        "wait_form_value",
        "assert_form_field",
        "wait_form_field",
        "assert_accessible_name",
        "wait_accessible_name",
        "assert_accessible_description",
        "wait_accessible_description",
    ] {
        let extra = if operation.contains("form_") {
            r#"field: "name", "#
        } else {
            ""
        };
        let source = format!(
            r#"fn echo(text: string) = text
            fn main() = bind(r: ui.{operation}(node_id: "a", {extra}expected: "confirmed"), body: echo(text: field(value: r, name: "expected")))"#
        );
        let program = lower(&parse(&source)).unwrap();
        assert_eq!(
            program.function.result_type,
            Type::Scalar(ScalarType::String)
        );
        assert_eq!(program.function.required_capabilities, ["ui.presentation"]);
        assert_eq!(
            lower(&parse(&canonical_source(&program.function.effect).unwrap())).unwrap(),
            program
        );
    }
}

#[test]
fn field_names_and_set_values_are_closed_string_projections() {
    for (operation, argument) in [
        ("set_form_value", r#"value: "written""#),
        ("assert_form_value", r#"expected: "written""#),
        ("wait_form_value", r#"expected: "written""#),
        ("assert_form_field", r#"expected: "Name""#),
        ("wait_form_field", r#"expected: "Name""#),
        ("assert_form_field_input_kind", r#"kind: "trimmed_text""#),
        ("wait_form_field_input_kind", r#"kind: "trimmed_text""#),
        ("assert_form_field_required", r#"state: "required""#),
        ("wait_form_field_required", r#"state: "optional""#),
        ("assert_form_field_max_length", r#"max_length: "7""#),
        ("wait_form_field_max_length", r#"max_length: "7""#),
        ("assert_form_field_placeholder", "expected: none"),
        ("wait_form_field_placeholder", "expected: none"),
    ] {
        let source = format!(
            r#"fn main() = bind(r: ui.{operation}(node_id: "form", field: "name", {argument}), body: field(value: r, name: "field"))"#
        );
        let program = lower(&parse(&source)).unwrap();
        assert_eq!(
            program.function.result_type,
            Type::Scalar(ScalarType::String)
        );
        assert_eq!(
            lower(&parse(&canonical_source(&program.function.effect).unwrap())).unwrap(),
            program
        );
    }
    let program = lower(&parse(r#"fn main() = bind(r: ui.set_form_value(node_id: "form", field: "name", value: "written"), body: field(value: r, name: "value"))"#)).unwrap();
    assert_eq!(
        program.function.result_type,
        Type::Scalar(ScalarType::String)
    );
}

#[test]
fn text_fields_feed_group_members_aliases_conditions_and_computed_host_arguments() {
    for source in [
        r#"fn main() = bind(r: ui.set_form_value(node_id: "form", field: "name", value: "written"), body: bind(alias: r, body: ui.assert_form_value(node_id: field(value: alias, name: "node_id"), field: field(value: alias, name: "field"), expected: field(value: r, name: "value"))))"#,
        r#"fn main() = bind(g: all(a: ui.assert_text(node_id: "a", expected: "ready"), b: ui.wait_accessible_name(node_id: "b", expected: "Ready")), body: eq(left: field(value: member(value: g, name: "a"), name: "expected"), right: "ready"))"#,
        r#"fn main() = bind(g: seq(a: ui.set_form_value(node_id: "form", field: "name", value: "written")), body: choose(when: eq(left: field(value: member(value: g, name: "a"), name: "value"), right: "written"), then: true, otherwise: bind(r: ui.assert_form_value(node_id: "form", field: "name", expected: "written"), body: eq(left: field(value: r, name: "expected"), right: "written"))))"#,
    ] {
        let program = lower(&parse(source)).unwrap();
        assert_eq!(
            lower(&parse(&canonical_source(&program.function.effect).unwrap())).unwrap(),
            program
        );
    }
}

#[test]
fn unsupported_nullable_raw_and_mistyped_fields_are_rejected_even_on_cold_paths() {
    for (operation, arguments, field) in [
        ("focus", "", "expected"),
        ("assert_text", r#", expected: "ready""#, "value"),
        (
            "assert_form_value",
            r#", field: "name", expected: "ready""#,
            "value",
        ),
        (
            "set_form_value",
            r#", field: "name", value: "ready""#,
            "expected",
        ),
        (
            "assert_form_field_placeholder",
            r#", field: "name", expected: "hint""#,
            "expected",
        ),
        (
            "wait_action_unavailable_reason",
            ", expected: none",
            "expected",
        ),
        (
            "assert_form_field_input_kind",
            r#", field: "name", kind: "trimmed_text""#,
            "input_kind",
        ),
        ("assert_accessible_name", r#", expected: "ready""#, "text"),
        (
            "set_form_value",
            r#", field: "name", value: "ready""#,
            "password",
        ),
    ] {
        let source = format!(
            r#"fn main() = bind(r: ui.{operation}(node_id: "a"{arguments}), body: choose(when: true, then: "", otherwise: field(value: r, name: "{field}")))"#
        );
        assert!(lower(&parse(&source)).is_err(), "{source}");
    }
    for body in [
        r#"add(left: field(value: r, name: "value"), right: 1)"#,
        r#"choose(when: field(value: r, name: "value"), then: true, otherwise: false)"#,
    ] {
        assert!(lower(&parse(&format!(r#"fn main() = bind(r: ui.set_form_value(node_id: "form", field: "name", value: "1"), body: {body})"#))).is_err());
    }
}

#[test]
fn projection_vocabularies_are_frozen_prefixes_and_new_fields_have_exact_types() {
    assert_eq!(ResultField::V3[..ResultField::V2.len()], ResultField::V2);
    assert_eq!(ResultField::V2[..ResultField::V1.len()], ResultField::V1);
    assert_eq!(ResultField::ALL, ResultField::V5);
    for ty in [
        Type::UiSetFormValue,
        Type::UiAssertFormValue,
        Type::UiAssertText,
        Type::UiWaitFormFieldRequired,
        Type::UiAssertFormFieldPlaceholder,
        Type::UiFocus,
        Type::RuntimeList,
        Type::RuntimeInspect,
        Type::Scalar(ScalarType::Boolean),
    ] {
        for field in [
            ResultField::Expected,
            ResultField::Field,
            ResultField::Value,
        ] {
            assert!(matches!(
                field.result_type(ty),
                None | Some(ScalarType::String)
            ));
        }
    }
    assert_eq!(
        ResultField::Value.result_type(Type::UiSetFormValue),
        Some(ScalarType::String)
    );
    assert_eq!(
        ResultField::Expected.result_type(Type::UiAssertFormFieldPlaceholder),
        None
    );
}
