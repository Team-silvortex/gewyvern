use leselang_hir::call_typing::{CallTypeLimits, check_call_arguments};
use leselang_hir::computation::{Computation, GroupLocalType};
use leselang_hir::host_call::HostOperation;
use leselang_hir::pure_typing::{PureTypeEnvironment, TypeInferenceLimits};
use leselang_hir::{CanonicalSourceError, Effect, Type, canonical_source, lower};
use leselang_runtime_core::{
    NamedParameter, OperationSchema, ScalarType, ScalarTypeSet, ScalarValue,
};
use leselang_syntax::parse;

#[test]
fn computed_reference_calls_revalidate_without_changing_result_or_wire_bytes() {
    for source in [
        r#"ui.focus(node_id: concat(left: "a", right: "-b"))"#,
        r#"ui.navigate_focus(direction: "first", node_id: concat(left: "a", right: "-b"))"#,
        r#"ui.assert_text(node_id: concat(left: "a", right: "-b"), expected: value_or(left: optional_string(value: none), right: ""))"#,
    ] {
        let program = lower(&parse(&format!("fn main() = {source}"))).unwrap();
        let Effect::Compute { expression } = &program.function.effect else {
            panic!("expected computation")
        };
        assert!(matches!(expression.as_ref(), Computation::Call { .. }));
        let wire = serde_json::to_vec(expression).unwrap();
        assert_eq!(
            expression.validate_in_scope(&[]).unwrap(),
            program.function.result_type
        );
        assert_eq!(serde_json::to_vec(expression).unwrap(), wire);
        let canonical = canonical_source(&program.function.effect).unwrap();
        assert_eq!(lower(&parse(&canonical)).unwrap(), program);
    }
}

#[test]
fn restored_reference_result_and_group_fields_feed_real_atomic_call_typing() {
    let program = lower(&parse(r#"fn main() = bind(r: ui.focus(node_id: "a"), body: ui.focus(node_id: concat(left: field(value: r, name: "node_id"), right: "-b")))"#)).unwrap();
    let Effect::Compute { expression } = &program.function.effect else {
        panic!("expected computation")
    };
    let Computation::Bind { body, .. } = expression.as_ref() else {
        panic!("expected bind")
    };
    assert_eq!(
        body.validate_in_scope(&[("r".into(), Type::UiFocus)])
            .unwrap(),
        Type::UiFocus
    );

    let program = lower(&parse(r#"fn main() = bind(g: seq(a: ui.focus(node_id: "a")), body: ui.focus(node_id: concat(left: field(value: member(value: g, name: "a"), name: "node_id"), right: "-b")))"#)).unwrap();
    let Effect::Compute { expression } = &program.function.effect else {
        panic!("expected computation")
    };
    let Computation::Bind { body, .. } = expression.as_ref() else {
        panic!("expected group bind")
    };
    let group = GroupLocalType {
        name: "g".into(),
        members: vec![("a".into(), HostOperation::UiFocus)],
    };
    assert_eq!(
        body.validate_in_group_scope(&[], &[group]).unwrap(),
        Type::UiFocus
    );
}

struct NoHost;
impl PureTypeEnvironment<leselang_hir::result_field::ResultField, HostOperation> for NoHost {
    type Result = ();
    fn field_type(
        &self,
        _: &(),
        _: &leselang_hir::result_field::ResultField,
    ) -> Option<ScalarType> {
        None
    }
    fn member_result(&self, _: &(), _: &str, _: &HostOperation) -> Option<()> {
        None
    }
}

#[test]
fn generic_signature_success_does_not_replace_reference_canonical_call_gate() {
    let expression = Computation::Call {
        operation: HostOperation::UiFocus,
        arguments: vec![leselang_hir::computation::ComputedArgument {
            name: "node_id".into(),
            value: Computation::Literal {
                value: ScalarValue::String("a".into()),
            },
        }],
    };
    let Computation::Call { arguments, .. } = &expression else {
        unreachable!()
    };
    let parameters = [NamedParameter::required(
        "node_id",
        ScalarTypeSet::only(ScalarType::String),
    )];
    let schema = OperationSchema {
        key: HostOperation::UiFocus,
        parameters: &parameters,
        result: Type::UiFocus,
        required_capability: (),
    };
    assert_eq!(
        check_call_arguments(
            arguments,
            &schema,
            &[],
            &NoHost,
            CallTypeLimits {
                pure: TypeInferenceLimits {
                    max_nodes: 2,
                    max_depth: 1,
                    max_bindings: 0
                },
                max_arguments: 1
            }
        )
        .unwrap(),
        &Type::UiFocus
    );
    // Literal-only reference source lowers to Host, never a forged Call wrapper.
    assert!(matches!(
        expression.validate_in_scope(&[]),
        Err(CanonicalSourceError::RoundTripMismatch)
    ));
}

#[test]
fn reference_reordered_arguments_and_invalid_domains_keep_original_rejections() {
    let program = lower(&parse(r#"fn main() = ui.navigate_focus(node_id: concat(left: "a", right: ""), direction: "first")"#)).unwrap();
    let Effect::Compute { expression } = program.function.effect else {
        panic!("expected computation")
    };
    let Computation::Call {
        operation,
        arguments,
    } = *expression
    else {
        panic!("expected call")
    };
    let mut reversed = arguments;
    reversed.reverse();
    let forged = Computation::Call {
        operation,
        arguments: reversed,
    };
    assert!(matches!(
        forged.validate_in_scope(&[]),
        Err(CanonicalSourceError::RoundTripMismatch)
    ));
    let source = r#"fn main() = ui.navigate_focus(node_id: concat(left: "a", right: ""), direction: "private-invalid")"#;
    let errors = lower(&parse(source)).unwrap_err();
    assert_eq!(errors[0].code, "LSH1407");
    assert_eq!(
        errors[0].message,
        "invalid ui.navigate_focus argument 'direction'"
    );
    let span = errors[0].span.unwrap();
    assert_eq!(
        &source[span.start..span.end],
        r#"direction: "private-invalid""#
    );
}
