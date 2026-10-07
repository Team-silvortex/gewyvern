use leselang_hir::computation::{Computation, ScalarType};
use leselang_hir::result_field::ResultField;
use leselang_hir::{Effect, Type, authorize, canonical_source, lower};
use leselang_host_contract::CapabilitySet;
use leselang_syntax::{Expression, Span, parse};

fn at(expression: &Expression) -> Span {
    match expression {
        Expression::Call { span, .. }
        | Expression::Reference { span, .. }
        | Expression::Integer { span, .. }
        | Expression::String { span, .. }
        | Expression::Boolean { span, .. }
        | Expression::None { span } => *span,
    }
}

#[test]
fn field_metadata_errors_preserve_child_before_name_before_native_export_order() {
    for (source, code, message, selected) in [
        (
            "field(value: missing, name: 1)",
            "LSH1403",
            "undefined local 'missing'",
            "value",
        ),
        (
            "field(value: missing, name: \"unknown\")",
            "LSH1403",
            "undefined local 'missing'",
            "value",
        ),
        (
            "field(value: 1, name: missing)",
            "LSH1409",
            "field name must be a string literal",
            "name",
        ),
        (
            "field(value: runtime.list(), name: 1)",
            "LSH1409",
            "field name must be a string literal",
            "name",
        ),
        (
            "field(value: runtime.list(), name: \"unknown\")",
            "LSH1409",
            "unknown result field",
            "root",
        ),
        (
            "field(value: 1, name: \"unknown\")",
            "LSH1409",
            "unknown result field",
            "root",
        ),
        (
            "field(value: runtime.list(), name: \"count\")",
            "LSH1409",
            "field is not exported by this bound result type",
            "root",
        ),
        (
            "field(value: 1, name: \"count\")",
            "LSH1409",
            "field is not exported by this bound result type",
            "root",
        ),
    ] {
        let ast = parse(&format!("fn main() = {source}"));
        let body = &ast.function.as_ref().unwrap().body;
        let Expression::Call { arguments, .. } = body else {
            panic!()
        };
        let expected = if selected == "root" {
            at(body)
        } else {
            at(&arguments
                .iter()
                .find(|argument| argument.name == selected)
                .unwrap()
                .value)
        };
        let errors = lower(&ast).unwrap_err();
        assert_eq!(errors.len(), 1, "{source}: {errors:?}");
        assert_eq!(errors[0].code, code, "{source}");
        assert_eq!(errors[0].message, message, "{source}");
        assert_eq!(errors[0].span, Some(expected), "{source}");
    }
}

#[test]
fn field_headers_reject_before_lowering_an_invalid_child() {
    for source in [
        "field(value: missing)",
        "field(value: missing, name: 1, extra: missing)",
        "field(value: missing, name: 1, name: 2)",
    ] {
        let ast = parse(&format!("fn main() = {source}"));
        let errors = lower(&ast).unwrap_err();
        assert_eq!(errors[0].code, "LSH1401");
        assert_eq!(
            errors[0].message,
            "expected exactly the named arguments: value, name"
        );
        assert_eq!(
            errors[0].span,
            Some(at(&ast.function.as_ref().unwrap().body))
        );
    }
}

#[test]
fn native_field_slots_and_result_types_round_trip_without_new_wire_nodes() {
    for (read, name, field, ty) in [
        (
            "runtime.list()",
            "count",
            ResultField::Count,
            ScalarType::Integer,
        ),
        (
            "ui.assert_text(node_id: \"status\", expected: \"ok\")",
            "expected",
            ResultField::Expected,
            ScalarType::String,
        ),
        (
            "ui.assert_selection(node_id: \"toggle\", state: \"selected\")",
            "selected",
            ResultField::Selected,
            ScalarType::Boolean,
        ),
        (
            "ui.assert_form_field_placeholder(node_id: \"form\", field: \"name\", expected: none)",
            "optional_expected",
            ResultField::OptionalExpected,
            ScalarType::OptionalString,
        ),
    ] {
        let program = lower(&parse(&format!(
            "fn main() = bind(row: {read}, body: field(name: \"{name}\", value: row))"
        )))
        .unwrap();
        assert_eq!(program.function.result_type, Type::Scalar(ty));
        let Effect::Compute { expression } = &program.function.effect else {
            panic!()
        };
        let Computation::Bind { body, .. } = expression.as_ref() else {
            panic!()
        };
        let Computation::Field {
            value,
            field: selected,
        } = body.as_ref()
        else {
            panic!()
        };
        assert_eq!(*selected, field);
        assert!(matches!(value.as_ref(), Computation::Local { name } if name == "row"));
        assert_eq!(body.children().count(), 1);
        let canonical = canonical_source(&program.function.effect).unwrap();
        assert_eq!(
            serde_json::to_vec(&program).unwrap(),
            serde_json::to_vec(&lower(&parse(&canonical)).unwrap()).unwrap()
        );
        assert!(authorize(&program, &CapabilitySet::default()).is_err());
        authorize(
            &program,
            &CapabilitySet::new(
                program
                    .function
                    .required_capabilities
                    .iter()
                    .map(String::as_str),
            ),
        )
        .unwrap();
    }
}

#[test]
fn complex_pure_input_and_helper_expansion_use_the_same_field_constructor() {
    for input in [
        "choose(when: true, then: row, otherwise: row)",
        "bind(alias: row, body: alias)",
    ] {
        let source = format!(
            "fn id(n: integer) = n\nfn main() = bind(row: runtime.list(), body: id(n: field(value: {input}, name: \"count\")))"
        );
        let program = lower(&parse(&source)).unwrap();
        assert_eq!(
            program.function.result_type,
            Type::Scalar(ScalarType::Integer)
        );
        assert_eq!(program.function.required_capabilities, ["runtime.read"]);
        let canonical = canonical_source(&program.function.effect).unwrap();
        assert_eq!(
            serde_json::to_vec(&program).unwrap(),
            serde_json::to_vec(&lower(&parse(&canonical)).unwrap()).unwrap()
        );
    }
}

#[test]
fn lookalike_native_records_do_not_supply_foreign_field_exports() {
    for source in [
        "bind(row: runtime.inspect(runtime_id: \"a\"), body: field(value: row, name: \"count\"))",
        "bind(row: runtime.list(), body: field(value: row, name: \"selected\"))",
        "bind(row: seq(a: runtime.list()), body: field(value: row, name: \"count\"))",
    ] {
        let errors = lower(&parse(&format!("fn main() = {source}"))).unwrap_err();
        assert_eq!(errors[0].code, "LSH1409", "{source}: {errors:?}");
        assert_eq!(
            errors[0].message,
            "field is not exported by this bound result type"
        );
    }
}

#[test]
fn field_does_not_hide_effects_in_cold_pure_input_branches() {
    for (input, code, message) in [
        (
            "choose(when: true, then: row, otherwise: runtime.list())",
            "LSH1409",
            "field is not exported by this bound result type",
        ),
        (
            "bind(inner: runtime.list(), body: row)",
            "LSH1408",
            "a captured host result requires a pure scalar body or a bounded atomic result chain",
        ),
    ] {
        let source = format!(
            "fn main() = bind(row: runtime.list(), body: field(value: {input}, name: \"count\"))"
        );
        let errors = lower(&parse(&source)).unwrap_err();
        assert_eq!(errors[0].code, code, "{source}: {errors:?}");
        assert_eq!(errors[0].message, message);
    }
}

#[test]
fn helper_local_result_names_never_escape_their_original_lexical_prefix() {
    let source = "fn count() = bind(row: runtime.list(), body: field(value: row, name: \"count\"))\nfn main() = field(value: row, name: \"count\")";
    let errors = lower(&parse(source)).unwrap_err();
    assert_eq!(errors[0].code, "LSH1403");
    assert_eq!(errors[0].message, "undefined local 'row'");
}
