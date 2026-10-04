use leselang_hir::computation::{Computation, GroupLocalType};
use leselang_hir::host_call::HostOperation;
use leselang_hir::{CanonicalSourceError, Effect, Type, authorize, canonical_source, lower};
use leselang_host_contract::CapabilitySet;
use leselang_runtime_core::{ScalarType, ScalarValue};
use leselang_syntax::parse;

#[test]
fn call_result_chains_keep_source_wire_types_and_capability_boundaries() {
    for source in [
        r#"bind(r: ui.focus(node_id: concat(left: "a", right: "")), body: bind(s: ui.focus(node_id: field(value: r, name: "node_id")), body: eq(left: field(value: r, name: "node_id"), right: field(value: s, name: "node_id"))))"#,
        r#"bind(r: choose(when: false, then: ui.focus(node_id: concat(left: "a", right: "")), otherwise: ui.focus(node_id: concat(left: "b", right: ""))), body: field(value: r, name: "node_id"))"#,
        r#"bind(target: "a", body: choose(when: true, then: bind(r: ui.focus(node_id: target), body: true), otherwise: bind(r: ui.focus(node_id: target), body: false)))"#,
    ] {
        let program = lower(&parse(&format!("fn main() = {source}"))).unwrap();
        let Effect::Compute { expression } = &program.function.effect else {
            panic!()
        };
        let mut pending = vec![expression.as_ref()];
        while let Some(node) = pending.pop() {
            assert!(!matches!(
                node,
                Computation::Host { .. } | Computation::Group { .. }
            ));
            pending.extend(node.children());
        }
        assert_eq!(
            expression.validate_in_scope(&[]).unwrap(),
            program.function.result_type
        );
        let wire = serde_json::to_vec(expression).unwrap();
        assert_eq!(
            lower(&parse(&canonical_source(&program.function.effect).unwrap())).unwrap(),
            program
        );
        assert_eq!(serde_json::to_vec(expression).unwrap(), wire);
        assert!(authorize(&program, &CapabilitySet::default()).is_err());
        authorize(&program, &CapabilitySet::new(["ui.presentation"])).unwrap();
    }
}

#[test]
fn external_group_exports_remain_closed_when_used_by_a_new_capture() {
    let source = r#"fn main() = bind(g: seq(first: ui.focus(node_id: "a")), body: bind(r: ui.focus(node_id: field(value: member(value: g, name: "first"), name: "node_id")), body: field(value: r, name: "node_id")))"#;
    let program = lower(&parse(source)).unwrap();
    let Effect::Compute { expression } = &program.function.effect else {
        panic!()
    };
    let Computation::Bind { body, .. } = expression.as_ref() else {
        panic!()
    };
    let group = GroupLocalType {
        name: "g".into(),
        members: vec![("first".into(), HostOperation::UiFocus)],
    };
    assert_eq!(
        body.validate_in_group_scope(&[], std::slice::from_ref(&group))
            .unwrap(),
        Type::Scalar(ScalarType::String)
    );
    let forged = GroupLocalType {
        name: "g".into(),
        members: vec![("first".into(), HostOperation::RuntimeList)],
    };
    assert!(body.validate_in_group_scope(&[], &[forged]).is_err());
    assert!(body.validate_in_scope(&[]).is_err());
}

#[test]
fn old_source_flow_restrictions_still_reject_raw_receipts_and_effectful_operands() {
    for source in [
        r#"bind(r: ui.focus(node_id: concat(left: "a", right: "")), body: r)"#,
        r#"field(value: ui.focus(node_id: concat(left: "a", right: "")), name: "node_id")"#,
        r#"loop(n: 0, while: false, next: bind(r: ui.focus(node_id: concat(left: "a", right: "")), body: 1), limit: 0)"#,
        r#"bind(r: ui.focus(node_id: concat(left: "a", right: "")), body: bind(r: ui.focus(node_id: "b"), body: true))"#,
    ] {
        assert!(
            lower(&parse(&format!("fn main() = {source}"))).is_err(),
            "{source}"
        );
    }
    let forged = Computation::Bind {
        name: "r".into(),
        value: Box::new(Computation::Call {
            operation: HostOperation::UiFocus,
            arguments: vec![leselang_hir::computation::ComputedArgument {
                name: "node_id".into(),
                value: Computation::Literal {
                    value: ScalarValue::String("a".into()),
                },
            }],
        }),
        body: Box::new(Computation::Literal {
            value: ScalarValue::Boolean(true),
        }),
    };
    assert!(matches!(
        forged.validate_in_scope(&[]),
        Err(CanonicalSourceError::RoundTripMismatch)
    ));
}

#[test]
fn mixed_opaque_and_new_group_flows_keep_their_existing_adapter_path() {
    for source in [
        r#"bind(r: runtime.list(), body: bind(s: ui.focus(node_id: concat(left: "a", right: "")), body: true))"#,
        r#"bind(g: seq(a: ui.focus(node_id: concat(left: "a", right: ""))), body: field(value: member(value: g, name: "a"), name: "node_id"))"#,
        r#"choose(when: false, then: bind(r: ui.focus(node_id: "a"), body: true), otherwise: bind(r: ui.focus(node_id: concat(left: "b", right: "")), body: false))"#,
    ] {
        let program = lower(&parse(&format!("fn main() = {source}"))).unwrap();
        let Effect::Compute { expression } = &program.function.effect else {
            panic!()
        };
        assert_eq!(
            expression.validate_in_scope(&[]).unwrap(),
            program.function.result_type
        );
        assert_eq!(
            lower(&parse(&canonical_source(&program.function.effect).unwrap())).unwrap(),
            program
        );
    }
}
