use leselang_hir::computation::ScalarType;
use leselang_hir::result_field::ResultField;
use leselang_hir::{
    CAPABILITY_UI_PRESENTATION, Type, UiFormInputKind, UiSemanticActionKind, UiSemanticNodeKind,
    canonical_source, lower,
};
use leselang_syntax::parse;

#[test]
fn canonical_kind_tokens_share_source_and_projection_spellings() {
    for token in [
        "column",
        "heading",
        "text",
        "runtime_card",
        "runtime_workspace",
        "section",
        "history_entry",
        "log_entry",
        "debugger_workspace",
        "debugger_frame",
        "action",
    ] {
        let kind = UiSemanticNodeKind::from_token(token).unwrap();
        assert_eq!(kind.as_str(), token);
        assert!(UiSemanticNodeKind::from_token(&token.to_uppercase()).is_none());
    }
    for token in [
        "runtime_inspect",
        "runtime_refresh",
        "runtime_capabilities_refresh",
        "runtime_deploy",
        "debugger_cancel",
    ] {
        let kind = UiSemanticActionKind::from_token(token).unwrap();
        assert_eq!(kind.as_str(), token);
        assert!(UiSemanticActionKind::from_token(&format!("{token} ")).is_none());
    }
    for token in ["path_token", "trimmed_text"] {
        let kind = UiFormInputKind::from_token(token).unwrap();
        assert_eq!(kind.as_str(), token);
    }
    for token in [
        "",
        "button",
        "Heading",
        " heading",
        "runtime.refresh",
        "free_text",
        "none",
    ] {
        assert!(UiSemanticNodeKind::from_token(token).is_none());
        assert!(UiSemanticActionKind::from_token(token).is_none());
        assert!(UiFormInputKind::from_token(token).is_none());
    }
}

#[test]
fn kind_projection_is_string_typed_for_exactly_the_six_kind_operations() {
    for (operation, arguments) in [
        ("assert_node_kind", r#"kind: "heading""#),
        ("wait_node_kind", r#"kind: "section""#),
        ("assert_action_kind", r#"kind: "runtime_refresh""#),
        ("wait_action_kind", r#"kind: "debugger_cancel""#),
        (
            "assert_form_field_input_kind",
            r#"field: "name", kind: "trimmed_text""#,
        ),
        (
            "wait_form_field_input_kind",
            r#"field: "name", kind: "path_token""#,
        ),
    ] {
        let source = format!(
            r#"fn identity(token: string) = token
            fn main() = bind(r: ui.{operation}(node_id: "a", {arguments}), body: identity(token: field(value: r, name: "kind")))"#
        );
        let program = lower(&parse(&source)).unwrap();
        assert_eq!(
            program.function.result_type,
            Type::Scalar(ScalarType::String)
        );
        assert_eq!(
            program.function.required_capabilities,
            [CAPABILITY_UI_PRESENTATION]
        );
        assert_eq!(
            lower(&parse(&canonical_source(&program.function.effect).unwrap())).unwrap(),
            program
        );
    }
    for ty in [
        Type::UiAssertNodeKind,
        Type::UiWaitNodeKind,
        Type::UiAssertActionKind,
        Type::UiWaitActionKind,
        Type::UiAssertFormFieldInputKind,
        Type::UiWaitFormFieldInputKind,
    ] {
        assert_eq!(ResultField::Kind.result_type(ty), Some(ScalarType::String));
    }
    for ty in [
        Type::UiFocus,
        Type::UiSetFormValue,
        Type::UiAssertFormFieldPlaceholder,
        Type::UiWaitFormFieldRequired,
        Type::UiAssertText,
        Type::RuntimeInspect,
    ] {
        assert_eq!(ResultField::Kind.result_type(ty), None);
    }
}

#[test]
fn kind_tokens_flow_through_aliases_helpers_group_members_and_computed_arguments() {
    for source in [
        r#"fn same(kind: string) = eq(left: kind, right: "heading")
        fn main() = bind(r: ui.assert_node_kind(node_id: "a", kind: "heading"), body: bind(alias: r, body: choose(when: same(kind: field(value: alias, name: "kind")), then: ui.wait_node_kind(node_id: "b", kind: field(value: r, name: "kind")), otherwise: ui.wait_node_kind(node_id: "c", kind: "text"))))"#,
        r#"fn main() = bind(g: all(a: ui.assert_action_kind(node_id: "a", kind: "runtime_deploy"), b: ui.assert_form_field_input_kind(node_id: "form", field: "name", kind: "path_token")), body: bind(r: ui.wait_form_field_input_kind(node_id: "form", field: field(value: member(value: g, name: "b"), name: "field"), kind: field(value: member(value: g, name: "b"), name: "kind")), body: field(value: member(value: g, name: "a"), name: "kind")))"#,
    ] {
        let program = lower(&parse(source)).unwrap();
        assert_eq!(
            lower(&parse(&canonical_source(&program.function.effect).unwrap())).unwrap(),
            program
        );
    }
}

#[test]
fn raw_enum_names_unsupported_fields_and_implicit_coercions_stay_rejected_on_cold_paths() {
    for (operation, arguments, field) in [
        ("focus", "", "kind"),
        ("assert_node_kind", r#", kind: "heading""#, "expected_kind"),
        (
            "assert_action_kind",
            r#", kind: "runtime_refresh""#,
            "expected",
        ),
        (
            "assert_form_field_input_kind",
            r#", field: "name", kind: "path_token""#,
            "input_kind",
        ),
        (
            "assert_form_field_placeholder",
            r#", field: "name", expected: none"#,
            "kind",
        ),
    ] {
        let source = format!(
            r#"fn main() = bind(r: ui.{operation}(node_id: "a"{arguments}), body: choose(when: true, then: "", otherwise: field(value: r, name: "{field}")))"#
        );
        assert!(lower(&parse(&source)).is_err(), "{source}");
    }
    for body in [
        r#"add(left: field(value: r, name: "kind"), right: 1)"#,
        r#"choose(when: field(value: r, name: "kind"), then: true, otherwise: false)"#,
    ] {
        assert!(lower(&parse(&format!(r#"fn main() = bind(r: ui.assert_node_kind(node_id: "a", kind: "heading"), body: {body})"#))).is_err());
    }
}

#[test]
fn v4_extends_frozen_vocabularies_without_changing_old_field_order() {
    assert_eq!(ResultField::V4[..ResultField::V3.len()], ResultField::V3);
    assert_eq!(ResultField::V3[..ResultField::V2.len()], ResultField::V2);
    assert_eq!(ResultField::V2[..ResultField::V1.len()], ResultField::V1);
    assert_eq!(ResultField::ALL, ResultField::V5);
    assert_eq!(ResultField::V4.last(), Some(&ResultField::Kind));
    assert_eq!(ResultField::Kind.name(), "kind");
}
