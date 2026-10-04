use leselang_hir::computation::Computation;
use leselang_hir::host_call::HostOperation;
use leselang_hir::{CanonicalSourceError, Effect, Type, authorize, canonical_source, lower};
use leselang_host_contract::CapabilitySet;
use leselang_runtime_core::ScalarValue;
use leselang_syntax::parse;

#[test]
fn call_only_preparation_preserves_canonical_source_type_wire_and_authority() {
    for source in [
        r#"bind(target: "a", body: ui.focus(node_id: target))"#,
        r#"bind(target: concat(left: "node-", right: "a"), body: choose(when: true, then: ui.focus(node_id: target), otherwise: ui.focus(node_id: concat(left: "node-", right: "b"))))"#,
        r#"choose(when: true, then: bind(target: "a", body: ui.focus(node_id: target)), otherwise: bind(target: "b", body: ui.focus(node_id: target)))"#,
        r#"bind(target: recover(value: to_string(value: div(left: 1, right: 0)), fallback: "a"), body: ui.focus(node_id: target))"#,
    ] {
        let program = lower(&parse(&format!("fn main() = {source}"))).unwrap();
        let Effect::Compute { expression } = &program.function.effect else {
            panic!("expected computation")
        };
        assert_eq!(
            expression.prepared_atomic_operation(),
            Some(HostOperation::UiFocus)
        );
        assert_eq!(expression.validate_in_scope(&[]).unwrap(), Type::UiFocus);
        let wire = serde_json::to_vec(expression).unwrap();
        let canonical = canonical_source(&program.function.effect).unwrap();
        assert_eq!(lower(&parse(&canonical)).unwrap(), program);
        assert_eq!(serde_json::to_vec(expression).unwrap(), wire);
        authorize(&program, &CapabilitySet::new(["ui.presentation"])).unwrap();
        assert!(authorize(&program, &CapabilitySet::default()).is_err());
    }
}

#[test]
fn external_receipt_aliases_feed_prepared_calls_without_new_captures() {
    let source = r#"fn main() = bind(r: ui.focus(node_id: "a"), body: bind(alias: r, body: bind(target: field(value: alias, name: "node_id"), body: choose(when: true, then: ui.focus(node_id: target), otherwise: ui.focus(node_id: target)))))"#;
    let program = lower(&parse(source)).unwrap();
    let Effect::Compute { expression } = &program.function.effect else {
        panic!("expected computation")
    };
    let Computation::Bind { body, .. } = expression.as_ref() else {
        panic!("expected capture")
    };
    assert_eq!(
        body.prepared_atomic_operation(),
        Some(HostOperation::UiFocus)
    );
    assert_eq!(
        body.validate_in_scope(&[("r".into(), Type::UiFocus)])
            .unwrap(),
        Type::UiFocus
    );
    assert_eq!(expression.atomic_flow_bound(), Some(2));
}

#[test]
fn reference_literal_host_mixed_terminal_groups_and_captures_keep_existing_paths() {
    for source in [
        r#"choose(when: false, then: ui.focus(node_id: "a"), otherwise: ui.focus(node_id: concat(left: "b", right: "")))"#,
        r#"seq(first: bind(target: "a", body: ui.focus(node_id: target)), second: ui.focus(node_id: "b"))"#,
        r#"bind(r: bind(target: "a", body: ui.focus(node_id: target)), body: field(value: r, name: "node_id"))"#,
        r#"bind(target: "a", body: choose(when: true, then: ui.focus(node_id: target), otherwise: bind(hidden: ui.focus(node_id: "b"), body: ui.focus(node_id: target))))"#,
    ] {
        let program = lower(&parse(&format!("fn main() = {source}"))).unwrap();
        let Effect::Compute { expression } = &program.function.effect else {
            panic!("expected computation")
        };
        assert_eq!(
            expression.validate_in_scope(&[]).unwrap(),
            program.function.result_type
        );
        let canonical = canonical_source(&program.function.effect).unwrap();
        assert_eq!(lower(&parse(&canonical)).unwrap(), program);
    }
}

#[test]
fn generic_preparation_does_not_promote_noncanonical_reference_trees_or_cold_faults() {
    let source = r#"fn main() = bind(target: "a", body: ui.focus(node_id: target))"#;
    let program = lower(&parse(source)).unwrap();
    let Effect::Compute { expression } = program.function.effect else {
        panic!("expected computation")
    };
    let Computation::Bind { name, value, body } = *expression else {
        panic!("expected bind")
    };
    let Computation::Call {
        operation,
        mut arguments,
    } = *body
    else {
        panic!("expected call")
    };
    arguments[0].value = Computation::Literal {
        value: ScalarValue::String("a".into()),
    };
    let forged = Computation::Bind {
        name,
        value,
        body: Box::new(Computation::Call {
            operation,
            arguments,
        }),
    };
    assert_eq!(
        forged.prepared_atomic_operation(),
        Some(HostOperation::UiFocus)
    );
    assert!(matches!(
        forged.validate_in_scope(&[]),
        Err(CanonicalSourceError::RoundTripMismatch)
    ));
    for source in [
        r#"fn main() = choose(when: true, then: bind(target: "a", body: ui.focus(node_id: target)), otherwise: bind(target: "b", body: ui.focus(node_id: 1)))"#,
        r#"fn main() = bind(target: "a", body: choose(when: bind(hidden: ui.focus(node_id: "b"), body: true), then: ui.focus(node_id: target), otherwise: ui.focus(node_id: target)))"#,
    ] {
        assert!(
            lower(&parse(source)).is_err(),
            "unexpected acceptance: {source}"
        );
    }
}
