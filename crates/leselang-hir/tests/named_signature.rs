use leselang_hir::computation::{OptionalStringValue, ScalarValue};
use leselang_hir::host_call::HostOperation;
use leselang_hir::{Effect, lower};
use leselang_syntax::parse;

#[test]
fn computed_names_fail_before_undefined_or_invalid_argument_values() {
    for (arguments, message) in [
        (
            "private_name: missing, expected: missing",
            "invalid named arguments for ui.assert_text",
        ),
        (
            "node_id: missing, node_id: missing",
            "invalid named arguments for ui.assert_text",
        ),
        (
            "node_id: missing",
            "invalid named arguments for ui.assert_text",
        ),
        (
            "node_id: missing, expected: missing, private_name: missing",
            "too many host arguments",
        ),
    ] {
        let source = format!("fn main() = ui.assert_text({arguments})");
        let errors = lower(&parse(&source)).unwrap_err();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, "LSH1407");
        assert_eq!(errors[0].message, message);
        let span = errors[0].span.unwrap();
        assert_eq!(
            &source[span.start..span.end],
            format!("ui.assert_text({arguments})")
        );
        assert!(
            !serde_json::to_string(&errors)
                .unwrap()
                .contains("private_name")
        );
    }
}

#[test]
fn resolve_name_errors_keep_exact_payload_free_legacy_diagnostics() {
    for (arguments, message) in [
        (vec![], "invalid named arguments for ui.focus"),
        (
            vec![("private_name".into(), ScalarValue::Boolean(true))],
            "invalid named arguments for ui.focus",
        ),
        (
            vec![
                ("node_id".into(), ScalarValue::Boolean(true)),
                ("node_id".into(), ScalarValue::None),
            ],
            "too many host arguments",
        ),
    ] {
        let errors = HostOperation::UiFocus.resolve(&arguments).unwrap_err();
        assert_eq!(
            serde_json::to_value(&errors).unwrap(),
            serde_json::json!([{"code": "LSH1407", "message": message, "span": null}])
        );
    }
}

#[test]
fn required_name_presence_is_distinct_from_explicit_optional_absence_and_empty_text() {
    let base = [
        ("node_id".into(), ScalarValue::String("target".into())),
        ("field".into(), ScalarValue::String("caption".into())),
    ];
    let operation = HostOperation::UiAssertFormFieldPlaceholder;
    assert!(operation.resolve(&base).is_err());
    for (value, expected) in [
        (ScalarValue::None, None),
        (ScalarValue::OptionalString(OptionalStringValue(None)), None),
        (ScalarValue::String(String::new()), Some(String::new())),
    ] {
        let mut arguments = base.to_vec();
        arguments.insert(0, ("expected".into(), value));
        let effect = operation.resolve(&arguments).unwrap();
        assert_eq!(
            effect,
            Effect::UiAssertFormFieldPlaceholder {
                node_id: "target".into(),
                field: "caption".into(),
                expected,
            }
        );
    }
    assert!(
        HostOperation::UiFocus
            .resolve(&[("node_id".into(), ScalarValue::None)])
            .is_err()
    );
    assert!(HostOperation::RuntimeList.resolve(&[]).is_ok());
}
