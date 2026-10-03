use leselang_hir::computation::ScalarType;
use leselang_hir::{Type, canonical_source, lower};
use leselang_syntax::parse;

#[test]
fn selection_and_requirement_results_export_boolean_fields_without_coercion() {
    for (operation, arguments, field) in [
        ("set_selection", r#"state: "selected""#, "selected"),
        ("assert_selection", r#"state: "unselected""#, "selected"),
        ("wait_selection", r#"state: "selected""#, "selected"),
        (
            "assert_form_field_required",
            r#"field: "name", state: "required""#,
            "required",
        ),
        (
            "wait_form_field_required",
            r#"field: "name", state: "optional""#,
            "required",
        ),
    ] {
        let source = format!(
            r#"fn invert(value: boolean) = not(value: value)
            fn main() = bind(r: ui.{operation}(node_id: "a", {arguments}),
                body: invert(value: field(value: r, name: "{field}")))"#
        );
        let program = lower(&parse(&source)).unwrap();
        assert_eq!(
            program.function.result_type,
            Type::Scalar(ScalarType::Boolean)
        );
        assert_eq!(program.function.required_capabilities, ["ui.presentation"]);
        assert_eq!(
            lower(&parse(&canonical_source(&program.function.effect).unwrap())).unwrap(),
            program
        );
    }
}

#[test]
fn boolean_projections_feed_conditions_named_members_and_successor_arguments() {
    for source in [
        r#"fn main() = bind(r: ui.set_selection(node_id: "a", state: "selected"),
            body: choose(when: field(value: r, name: "selected"), then: 1, otherwise: 0))"#,
        r#"fn main() = bind(r: ui.assert_form_field_required(node_id: "form", field: "name", state: "required"),
            body: ui.focus(node_id: choose(when: field(value: r, name: "required"), then: "name", otherwise: "next")))"#,
        r#"fn main() = bind(g: all(
            selection: ui.wait_selection(node_id: "row", state: "selected"),
            requirement: ui.wait_form_field_required(node_id: "form", field: "name", state: "optional")),
            body: and(left: field(value: member(value: g, name: "selection"), name: "selected"),
                right: not(value: field(value: member(value: g, name: "requirement"), name: "required"))))"#,
    ] {
        let program = lower(&parse(source)).unwrap();
        assert_eq!(
            lower(&parse(&canonical_source(&program.function.effect).unwrap())).unwrap(),
            program
        );
    }
}

#[test]
fn unsupported_boolean_fields_and_implicit_boolean_text_are_rejected_in_cold_code() {
    for source in [
        r#"fn main() = bind(r: ui.focus(node_id: "a"), body: field(value: r, name: "selected"))"#,
        r#"fn main() = bind(r: ui.set_selection(node_id: "a", state: "selected"), body: field(value: r, name: "required"))"#,
        r#"fn main() = bind(r: ui.assert_form_field_required(node_id: "form", field: "name", state: "required"),
            body: choose(when: true, then: false, otherwise: field(value: r, name: "selected")))"#,
        r#"fn main() = bind(r: ui.set_selection(node_id: "a", state: "selected"),
            body: eq(left: field(value: r, name: "selected"), right: "selected"))"#,
        r#"fn main() = bind(r: ui.set_selection(node_id: "a", state: "selected"),
            body: ui.focus(node_id: field(value: r, name: "selected")))"#,
        r#"fn main() = bind(r: ui.set_selection(node_id: "a", state: "selected"),
            body: field(value: r, name: "state"))"#,
    ] {
        assert!(lower(&parse(source)).is_err(), "{source}");
    }
}
