use leselang_hir::computation::{Computation, ComputedBranch};
use leselang_hir::{Effect, HirBranch, Type, authorize, canonical_source, lower};
use leselang_host_contract::CapabilitySet;
use leselang_runtime_core::{ScalarType, ScalarValue};
use leselang_syntax::parse;

fn checked(source: &str, ty: Type) {
    let program = lower(&parse(&format!("fn main() = {source}"))).unwrap();
    assert_eq!(program.function.result_type, ty);
    let wire = serde_json::to_vec(&program).unwrap();
    let roundtrip = lower(&parse(&canonical_source(&program.function.effect).unwrap())).unwrap();
    assert_eq!(serde_json::to_vec(&roundtrip).unwrap(), wire);
    let Effect::Compute { expression } = &program.function.effect else {
        panic!("expected the existing computation profile")
    };
    assert_eq!(expression.validate_in_scope(&[]).unwrap(), ty);
    assert!(authorize(&program, &CapabilitySet::default()).is_err());
    authorize(
        &program,
        &CapabilitySet::new(["runtime.read", "ui.presentation"]),
    )
    .unwrap();
}

#[test]
fn flat_native_group_captures_and_result_driven_successors_keep_wire_and_authority() {
    for source in [
        r#"bind(g: seq(read: runtime.list()), body: field(value: member(value: g, name: "read"), name: "count"))"#,
        r#"bind(g: all(read: runtime.list(), focus: ui.focus(node_id: "a")), body: field(value: member(value: g, name: "read"), name: "count"))"#,
        r#"bind(g: seq(read: runtime.list()), body: bind(r: ui.focus(node_id: concat(left: "a", right: "")), body: field(value: member(value: g, name: "read"), name: "count")))"#,
    ] {
        checked(source, Type::Scalar(ScalarType::Integer));
    }
}

#[test]
fn native_and_computed_group_choices_share_one_closed_ordered_signature() {
    for kind in ["seq", "all"] {
        let source = format!(
            r#"bind(g: choose(when: false, then: {kind}(focus: ui.focus(node_id: "a"), read: runtime.list()), otherwise: {kind}(focus: ui.focus(node_id: concat(left: "b", right: "")), read: runtime.list())), body: field(value: member(value: g, name: "focus"), name: "node_id"))"#
        );
        checked(&source, Type::Scalar(ScalarType::String));
        let program = lower(&parse(&format!("fn main() = {source}"))).unwrap();
        let Effect::Compute { expression } = program.function.effect else {
            panic!()
        };
        let mut native = false;
        let mut computed = false;
        let mut pending = vec![expression.as_ref()];
        while let Some(node) = pending.pop() {
            native |= matches!(node, Computation::Host { effect } if matches!(effect.as_ref(), Effect::Sequence { .. } | Effect::All { .. }));
            computed |= matches!(node, Computation::Group { .. });
            pending.extend(node.children());
        }
        assert!(native && computed);
    }
}

#[test]
fn native_group_aliases_pure_selection_and_helpers_keep_closed_exports() {
    for source in [
        r#"bind(g: choose(when: true, then: seq(read: runtime.list(role: "edge")), otherwise: seq(read: runtime.list(role: "core"))), body: bind(alias: g, body: field(value: member(value: alias, name: "read"), name: "count")))"#,
        r#"bind(g: bind(role: "edge", body: seq(read: runtime.list(role: role))), body: field(value: member(value: g, name: "read"), name: "count"))"#,
    ] {
        checked(source, Type::Scalar(ScalarType::Integer));
    }
    let program = lower(&parse(
        r#"fn inventory() = seq(read: runtime.list())
fn main() = bind(g: inventory(), body: field(value: member(value: g, name: "read"), name: "count"))"#,
    ))
    .unwrap();
    assert_eq!(
        program.function.result_type,
        Type::Scalar(ScalarType::Integer)
    );
    assert_eq!(
        lower(&parse(&canonical_source(&program.function.effect).unwrap())).unwrap(),
        program
    );
}

#[test]
fn mode_order_names_and_operation_mismatches_do_not_union_native_and_computed_exports() {
    for other in [
        r#"all(focus: ui.focus(node_id: concat(left: "b", right: "")), read: runtime.list())"#,
        r#"seq(read: runtime.list(), focus: ui.focus(node_id: concat(left: "b", right: "")))"#,
        r#"seq(other: ui.focus(node_id: concat(left: "b", right: "")), read: runtime.list())"#,
        r#"seq(focus: ui.activate(node_id: concat(left: "b", right: "")), read: runtime.list())"#,
    ] {
        let source = format!(
            r#"fn main() = bind(g: choose(when: true, then: seq(focus: ui.focus(node_id: "a"), read: runtime.list()), otherwise: {other}), body: field(value: member(value: g, name: "focus"), name: "node_id"))"#
        );
        assert!(lower(&parse(&source)).is_err(), "{other}");
    }
}

#[test]
fn graph_typing_does_not_allow_nested_members_raw_receipts_or_effectful_preparation() {
    for source in [
        r#"bind(g: seq(read: runtime.list()), body: g)"#,
        r#"bind(g: seq(read: runtime.list()), body: member(value: g, name: "read"))"#,
        r#"bind(g: seq(read: runtime.list()), body: field(value: member(value: g, name: "private"), name: "count"))"#,
        r#"bind(g: seq(nested: seq(read: runtime.list())), body: field(value: member(value: g, name: "nested"), name: "count"))"#,
        r#"bind(g: seq(read: bind(r: runtime.list(), body: ui.focus(node_id: "a"))), body: true)"#,
        r#"bind(r: runtime.list(), body: bind(g: seq(read: runtime.list()), body: true))"#,
    ] {
        assert!(
            lower(&parse(&format!("fn main() = {source}"))).is_err(),
            "{source}"
        );
    }
}

fn native_capture(branches: Vec<HirBranch>) -> Computation {
    Computation::Bind {
        name: "g".into(),
        value: Box::new(Computation::Host {
            effect: Box::new(Effect::Sequence { steps: branches }),
        }),
        body: Box::new(Computation::Literal {
            value: ScalarValue::Boolean(true),
        }),
    }
}

#[test]
fn cold_forged_native_graph_metadata_and_payloads_fail_canonical_admission() {
    let branch = || HirBranch {
        name: "focus".into(),
        effect: Effect::UiFocus {
            node_id: "a".into(),
        },
        result_type: Type::UiFocus,
    };
    let mut wrong_type = branch();
    wrong_type.result_type = Type::RuntimeList;
    let mut invalid_name = branch();
    invalid_name.name = "bad name".into();
    let mut invalid_payload = branch();
    invalid_payload.effect = Effect::UiFocus { node_id: "".into() };
    for branches in [
        vec![],
        vec![wrong_type],
        vec![invalid_name],
        vec![invalid_payload],
        vec![branch(), branch()],
    ] {
        let value = Computation::Choose {
            when: Box::new(Computation::Literal {
                value: ScalarValue::Boolean(false),
            }),
            then: Box::new(native_capture(branches)),
            otherwise: Box::new(Computation::Literal {
                value: ScalarValue::Boolean(true),
            }),
        };
        assert!(value.validate_in_scope(&[]).is_err());
    }
}

#[test]
fn an_opaque_group_cannot_masquerade_as_an_atomic_computed_member() {
    let group = Computation::Group {
        group_kind: leselang_hir::ir::GroupKind::Sequence,
        branches: vec![ComputedBranch {
            name: "nested".into(),
            value: Computation::Host {
                effect: Box::new(Effect::Sequence {
                    steps: vec![HirBranch {
                        name: "read".into(),
                        effect: Effect::RuntimeList {
                            filter: Default::default(),
                        },
                        result_type: Type::RuntimeList,
                    }],
                }),
            },
            result_type: Type::Structured,
        }],
    };
    assert!(group.validate_in_scope(&[]).is_err());
}
