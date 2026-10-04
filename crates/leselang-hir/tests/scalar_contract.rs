use leselang_hir::computation::{OptionalStringValue, ScalarValue, StringListValue};
use leselang_hir::host_call::HostOperation;
use leselang_hir::lower;
use leselang_syntax::parse;

#[test]
fn actual_none_optional_absence_and_plain_text_keep_distinct_host_contracts() {
    let values = [
        ScalarValue::Integer(0),
        ScalarValue::Boolean(false),
        ScalarValue::String("ready".into()),
        ScalarValue::None,
        ScalarValue::OptionalString(OptionalStringValue(None)),
        ScalarValue::StringList(StringListValue(vec![])),
    ];
    for (index, value) in values.into_iter().enumerate() {
        let nullable_text = HostOperation::UiAssertFormFieldPlaceholder.resolve(&[
            ("node_id".into(), ScalarValue::String("target".into())),
            ("field".into(), ScalarValue::String("caption".into())),
            ("expected".into(), value.clone()),
        ]);
        let ordinary_text = HostOperation::UiAssertText.resolve(&[
            ("node_id".into(), ScalarValue::String("target".into())),
            ("expected".into(), value.clone()),
        ]);
        let filter = HostOperation::RuntimeList.resolve(&[("role".into(), value)]);
        assert_eq!(nullable_text.is_ok(), matches!(index, 2..=4));
        assert_eq!(ordinary_text.is_ok(), index == 2);
        assert_eq!(filter.is_ok(), matches!(index, 2 | 3));
    }
}

#[test]
fn type_acceptance_does_not_replace_static_cold_domain_or_language_size_checks() {
    for source in [
        r#"bind(x: none, body: ui.focus(node_id: x))"#,
        r#"bind(x: optional_string(value: none), body: ui.focus(node_id: x))"#,
        r#"bind(x: strings(), body: ui.assert_text(node_id: "target", expected: x))"#,
        r#"choose(when: true, then: ui.focus(node_id: "target"), otherwise: ui.focus(node_id: 7))"#,
        r#"ui.navigate_focus(node_id: concat(left: "tar", right: "get"), direction: "not-a-direction")"#,
    ] {
        assert!(
            lower(&parse(&format!("fn main() = {source}"))).is_err(),
            "{source}"
        );
    }
    let too_large = "private".repeat(586);
    for value in [
        ScalarValue::String(too_large.clone()),
        ScalarValue::OptionalString(OptionalStringValue(Some(too_large))),
    ] {
        let errors = HostOperation::UiAssertFormFieldPlaceholder
            .resolve(&[
                ("node_id".into(), ScalarValue::String("target".into())),
                ("field".into(), ScalarValue::String("caption".into())),
                ("expected".into(), value),
            ])
            .unwrap_err();
        assert_eq!(errors[0].code, "LSH1407");
        assert!(!serde_json::to_string(&errors).unwrap().contains("private"));
    }
    assert!(
        HostOperation::UiFocus
            .resolve(&[("node_id".into(), ScalarValue::String("bad node".into()))])
            .is_err()
    );
}
