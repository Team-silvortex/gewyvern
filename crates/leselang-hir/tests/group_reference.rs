use leselang_hir::computation::{Computation, GroupKind};
use leselang_hir::{Effect, Type, authorize, canonical_source, lower};
use leselang_host_contract::CapabilitySet;
use leselang_runtime_core::ScalarType;
use leselang_syntax::parse;

#[test]
fn all_call_groups_support_capture_closed_aliases_and_a_typed_successor_without_wire_changes() {
    for kind in ["seq", "all"] {
        let source = format!(
            r#"fn main() = bind(g: {kind}(first: ui.focus(node_id: concat(left: "a", right: "")), second: ui.focus(node_id: concat(left: "b", right: ""))), body: bind(alias: g, body: bind(r: ui.focus(node_id: field(value: member(value: alias, name: "second"), name: "node_id")), body: field(value: r, name: "node_id"))))"#
        );
        let program = lower(&parse(&source)).unwrap();
        let Effect::Compute { expression } = &program.function.effect else {
            panic!()
        };
        let mut pending = vec![expression.as_ref()];
        while let Some(node) = pending.pop() {
            assert!(!matches!(node, Computation::Host { .. }));
            pending.extend(node.children());
        }
        assert_eq!(
            expression.validate_in_scope(&[]).unwrap(),
            Type::Scalar(ScalarType::String)
        );
        let wire = serde_json::to_vec(expression).unwrap();
        assert_eq!(
            lower(&parse(&canonical_source(&program.function.effect).unwrap())).unwrap(),
            program
        );
        assert_eq!(serde_json::to_vec(expression).unwrap(), wire);
        authorize(&program, &CapabilitySet::new(["ui.presentation"])).unwrap();
        assert!(authorize(&program, &CapabilitySet::default()).is_err());
    }
}

#[test]
fn selected_prepared_groups_preserve_exact_order_operation_and_mode_exports() {
    for kind in ["seq", "all"] {
        let source = format!(
            r#"fn main() = bind(node: "a", body: bind(g: choose(when: false, then: {kind}(first: ui.focus(node_id: node), second: ui.focus(node_id: node)), otherwise: {kind}(first: bind(target: "b", body: ui.focus(node_id: target)), second: ui.focus(node_id: node))), body: field(value: member(value: g, name: "first"), name: "node_id")))"#
        );
        let program = lower(&parse(&source)).unwrap();
        let Effect::Compute { expression } = &program.function.effect else {
            panic!()
        };
        assert_eq!(
            expression.validate_in_scope(&[]).unwrap(),
            Type::Scalar(ScalarType::String)
        );
        assert_eq!(
            lower(&parse(&canonical_source(&program.function.effect).unwrap())).unwrap(),
            program
        );
    }
    for source in [
        r#"bind(g: choose(when: true, then: seq(a: ui.focus(node_id: concat(left: "a", right: ""))), otherwise: seq(b: ui.focus(node_id: concat(left: "b", right: "")))), body: true)"#,
        r#"bind(g: choose(when: true, then: seq(a: ui.focus(node_id: concat(left: "a", right: "")), b: ui.focus(node_id: "b")), otherwise: all(a: ui.focus(node_id: "a"), b: ui.focus(node_id: "b"))), body: true)"#,
    ] {
        assert!(lower(&parse(&format!("fn main() = {source}"))).is_err());
    }
}

#[test]
fn forged_original_branch_declarations_names_and_nested_groups_still_fail_before_execution() {
    let program = lower(&parse(r#"fn main() = seq(a: ui.focus(node_id: concat(left: "a", right: "")), b: ui.focus(node_id: concat(left: "b", right: "")))"#)).unwrap();
    let Effect::Compute { expression } = &program.function.effect else {
        panic!()
    };
    assert_eq!(expression.validate_in_scope(&[]).unwrap(), Type::Structured);
    for case in 0..4 {
        let mut forged = expression.as_ref().clone();
        let Computation::Group {
            group_kind,
            branches,
        } = &mut forged
        else {
            panic!()
        };
        match case {
            0 => branches[1].result_type = Type::RuntimeList,
            1 => branches[1].name = branches[0].name.clone(),
            2 => {
                *group_kind = GroupKind::Parallel;
                branches.truncate(1);
            }
            _ => branches[0].value = expression.as_ref().clone(),
        }
        assert!(forged.validate_in_scope(&[]).is_err());
    }
}

#[test]
fn literal_mixed_host_and_old_group_source_limits_keep_their_adapter_path() {
    for source in [
        r#"bind(g: seq(a: ui.focus(node_id: "a"), b: ui.focus(node_id: concat(left: "b", right: ""))), body: field(value: member(value: g, name: "a"), name: "node_id"))"#,
        r#"bind(g: choose(when: false, then: seq(a: ui.focus(node_id: "a")), otherwise: seq(a: ui.focus(node_id: concat(left: "b", right: "")))), body: field(value: member(value: g, name: "a"), name: "node_id"))"#,
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
    for source in [
        r#"bind(g: seq(a: ui.focus(node_id: concat(left: "a", right: "")), b: ui.focus(node_id: field(value: member(value: g, name: "a"), name: "node_id"))), body: true)"#,
        r#"seq(a: bind(r: ui.focus(node_id: "a"), body: ui.focus(node_id: "b")))"#,
        r#"bind(r: ui.focus(node_id: concat(left: "a", right: "")), body: bind(g: seq(a: ui.focus(node_id: "b")), body: true))"#,
    ] {
        assert!(lower(&parse(&format!("fn main() = {source}"))).is_err());
    }
}
