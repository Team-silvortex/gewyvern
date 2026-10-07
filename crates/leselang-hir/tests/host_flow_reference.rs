use leselang_hir::computation::{Computation, GroupLocalType};
use leselang_hir::host_call::HostOperation;
use leselang_hir::{Effect, Type, authorize, canonical_source, lower};
use leselang_host_contract::CapabilitySet;
use leselang_runtime_core::{ScalarType, ScalarValue};
use leselang_syntax::parse;

#[test]
fn mixed_native_host_and_computed_call_chains_keep_native_types_wire_and_authority() {
    for source in [
        r#"bind(r: ui.focus(node_id: "a"), body: bind(s: ui.focus(node_id: field(value: r, name: "node_id")), body: field(value: s, name: "node_id")))"#,
        r#"bind(r: choose(when: false, then: ui.focus(node_id: "a"), otherwise: ui.focus(node_id: concat(left: "b", right: ""))), body: field(value: r, name: "node_id"))"#,
        r#"choose(when: false, then: bind(r: ui.focus(node_id: "a"), body: true), otherwise: bind(r: ui.focus(node_id: concat(left: "b", right: "")), body: false))"#,
    ] {
        let program = lower(&parse(&format!("fn main() = {source}"))).unwrap();
        let Effect::Compute { expression } = &program.function.effect else {
            panic!()
        };
        let wire = serde_json::to_vec(&program).unwrap();
        assert_eq!(
            expression.validate_in_scope(&[]).unwrap(),
            program.function.result_type
        );
        let roundtrip =
            lower(&parse(&canonical_source(&program.function.effect).unwrap())).unwrap();
        assert_eq!(serde_json::to_vec(&roundtrip).unwrap(), wire);
        assert!(authorize(&program, &CapabilitySet::default()).is_err());
        authorize(&program, &CapabilitySet::new(["ui.presentation"])).unwrap();
        let mut host = false;
        let mut call = false;
        let mut pending = vec![expression.as_ref()];
        while let Some(node) = pending.pop() {
            host |= matches!(node, Computation::Host { .. });
            call |= matches!(node, Computation::Call { .. });
            pending.extend(node.children());
        }
        assert!(host && call);
    }
}

#[test]
fn external_closed_group_exports_stay_exact_inside_a_mixed_native_host_chain() {
    let source = r#"fn main() = bind(g: seq(a: ui.focus(node_id: "a")), body: bind(r: runtime.list(), body: bind(s: ui.focus(node_id: field(value: member(value: g, name: "a"), name: "node_id")), body: field(value: r, name: "count"))))"#;
    let program = lower(&parse(source)).unwrap();
    let Effect::Compute { expression } = &program.function.effect else {
        panic!()
    };
    let Computation::Bind { body, .. } = expression.as_ref() else {
        panic!()
    };
    let group = GroupLocalType {
        name: "g".into(),
        members: vec![("a".into(), HostOperation::UiFocus)],
    };
    assert_eq!(
        body.validate_in_group_scope(&[], &[group]).unwrap(),
        Type::Scalar(ScalarType::Integer)
    );
    let forged = GroupLocalType {
        name: "g".into(),
        members: vec![("a".into(), HostOperation::RuntimeList)],
    };
    assert!(body.validate_in_group_scope(&[], &[forged]).is_err());
    assert!(body.validate_in_scope(&[]).is_err());
}

#[test]
fn native_graph_host_wrappers_and_new_groups_keep_their_existing_source_policy() {
    for source in [
        r#"bind(g: seq(a: runtime.list()), body: bind(r: ui.focus(node_id: concat(left: "a", right: "")), body: field(value: member(value: g, name: "a"), name: "count")))"#,
        r#"choose(when: true, then: seq(a: runtime.list()), otherwise: seq(a: runtime.list()))"#,
    ] {
        let program = lower(&parse(&format!("fn main() = {source}"))).unwrap();
        assert_eq!(
            lower(&parse(&canonical_source(&program.function.effect).unwrap())).unwrap(),
            program
        );
    }
}

#[test]
fn native_raw_receipt_and_effectful_operand_restrictions_do_not_become_typing_permissions() {
    for source in [
        r#"bind(r: runtime.list(), body: r)"#,
        r#"field(value: runtime.list(), name: "count")"#,
        r#"choose(when: runtime.list(), then: true, otherwise: false)"#,
        r#"recover(value: 7, fallback: bind(r: runtime.list(), body: 8))"#,
        r#"loop(n: 0, while: false, next: bind(r: runtime.list(), body: 1), limit: 0)"#,
    ] {
        assert!(
            lower(&parse(&format!("fn main() = {source}"))).is_err(),
            "{source}"
        );
    }
}

#[test]
fn forged_native_payload_and_literal_call_nodes_are_still_rejected_by_canonical_admission() {
    let invalid_host = Computation::Choose {
        when: Box::new(Computation::Literal {
            value: ScalarValue::Boolean(false),
        }),
        then: Box::new(Computation::Host {
            effect: Box::new(Effect::UiFocus { node_id: "".into() }),
        }),
        otherwise: Box::new(Computation::Host {
            effect: Box::new(Effect::UiFocus {
                node_id: "target".into(),
            }),
        }),
    };
    assert!(invalid_host.validate_in_scope(&[]).is_err());
    let literal_call = Computation::Bind {
        name: "r".into(),
        value: Box::new(Computation::Host {
            effect: Box::new(Effect::RuntimeList {
                filter: Default::default(),
            }),
        }),
        body: Box::new(Computation::Call {
            operation: HostOperation::UiFocus,
            arguments: vec![leselang_hir::computation::ComputedArgument {
                name: "node_id".into(),
                value: Computation::Literal {
                    value: ScalarValue::String("target".into()),
                },
            }],
        }),
    };
    assert!(literal_call.validate_in_scope(&[]).is_err());
}
