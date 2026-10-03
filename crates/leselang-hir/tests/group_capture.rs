use leselang_hir::computation::{Computation, GroupLocalType, ScalarType};
use leselang_hir::host_call::HostOperation;
use leselang_hir::{Effect, Type, authorize, canonical_source, lower};
use leselang_host_contract::CapabilitySet;
use leselang_syntax::parse;

#[test]
fn sequential_groups_allow_one_atomic_capture_and_a_typed_pure_body() {
    for source in [
        r#"fn main() = bind(g: seq(a: ui.focus(node_id: "a")), body: bind(r: ui.focus(node_id: field(value: member(value: g, name: "a"), name: "node_id")), body: eq(left: field(value: r, name: "node_id"), right: field(value: member(value: g, name: "a"), name: "node_id"))))"#,
        r#"fn main() = bind(g: repeat(times: 2, body: ui.focus(node_id: "a")), body: bind(alias: g, body: bind(r: ui.focus(node_id: "a"), body: eq(left: field(value: r, name: "node_id"), right: field(value: member(value: alias, name: "iteration_2"), name: "node_id")))))"#,
        r#"fn main() = bind(g: seq(a: ui.focus(node_id: concat(left: "", right: "a"))), body: choose(when: true, then: bind(r: ui.focus(node_id: "b"), body: true), otherwise: bind(r: runtime.list(), body: false)))"#,
        r#"fn yes(selected: boolean) = selected
            fn main() = bind(g: seq(a: ui.assert_selection(node_id: "a", state: "selected")), body: bind(r: ui.focus(node_id: "b"), body: yes(selected: field(value: member(value: g, name: "a"), name: "selected"))))"#,
    ] {
        let program = lower(&parse(source)).unwrap();
        assert_eq!(
            program.function.result_type,
            Type::Scalar(ScalarType::Boolean)
        );
        assert_eq!(
            lower(&parse(&canonical_source(&program.function.effect).unwrap())).unwrap(),
            program
        );
    }
}

#[test]
fn group_captures_reject_nested_reentry_raw_tails_and_mixed_result_types() {
    for source in [
        r#"fn main() = bind(g: seq(a: ui.focus(node_id: "a")), body: bind(r: ui.focus(node_id: "b"), body: ui.focus(node_id: "c")))"#,
        r#"fn main() = bind(g: seq(a: ui.focus(node_id: "a")), body: bind(r: ui.focus(node_id: "b"), body: choose(when: true, then: true, otherwise: bind(next: ui.focus(node_id: "c"), body: 1))))"#,
        r#"fn main() = bind(g: seq(a: ui.focus(node_id: "a")), body: choose(when: true, then: true, otherwise: bind(r: ui.focus(node_id: "b"), body: "false")))"#,
        r#"fn main() = bind(g: seq(a: ui.focus(node_id: "a")), body: choose(when: true, then: ui.focus(node_id: "b"), otherwise: bind(r: ui.focus(node_id: "b"), body: field(value: r, name: "node_id"))))"#,
        r#"fn main() = bind(g: seq(a: ui.focus(node_id: "a")), body: bind(r: seq(b: ui.focus(node_id: "b")), body: true))"#,
        r#"fn main() = bind(g: seq(a: ui.focus(node_id: "a")), body: bind(r: ui.focus(node_id: "b"), body: field(value: member(value: g, name: "missing"), name: "node_id")))"#,
    ] {
        assert!(lower(&parse(source)).is_err(), "{source}");
    }
}

#[test]
fn capture_preflight_includes_all_prefix_and_cold_branch_capabilities() {
    let program = lower(&parse(r#"fn main() = bind(g: seq(a: ui.focus(node_id: "a")), body: choose(when: true, then: bind(r: ui.focus(node_id: "b"), body: true), otherwise: bind(r: runtime.list(), body: false)))"#)).unwrap();
    assert_eq!(
        program.function.required_capabilities,
        ["runtime.read", "ui.presentation"]
    );
    assert!(authorize(&program, &CapabilitySet::new(["ui.presentation"])).is_err());
    authorize(
        &program,
        &CapabilitySet::new(["ui.presentation", "runtime.read"]),
    )
    .unwrap();
}

#[test]
fn captured_successor_reserves_one_of_the_sixty_four_graph_slots() {
    for count in [1, 63, 64] {
        let source = format!(
            r#"fn main() = bind(g: repeat(times: {count}, body: ui.focus(node_id: "a")), body: bind(r: ui.focus(node_id: "b"), body: true))"#
        );
        assert_eq!(lower(&parse(&source)).is_ok(), count <= 63);
    }
}

#[test]
fn external_group_scope_is_closed_bounded_and_canonical() {
    let program = lower(&parse(r#"fn main() = bind(g: seq(a: ui.focus(node_id: "a")), body: bind(r: ui.focus(node_id: "b"), body: field(value: member(value: g, name: "a"), name: "node_id")))"#)).unwrap();
    let Effect::Compute { expression } = program.function.effect else {
        panic!()
    };
    let Computation::Bind { body, .. } = *expression else {
        panic!()
    };
    let Computation::Bind { body, .. } = *body else {
        panic!()
    };
    let group = GroupLocalType {
        name: "g".into(),
        members: vec![("a".into(), HostOperation::UiFocus)],
    };
    assert_eq!(
        body.validate_in_group_scope(&[], std::slice::from_ref(&group))
            .unwrap(),
        Type::Scalar(ScalarType::String)
    );
    for forged in [
        GroupLocalType {
            name: "g".into(),
            members: vec![],
        },
        GroupLocalType {
            name: "g".into(),
            members: vec![("a".into(), HostOperation::RuntimeList)],
        },
        GroupLocalType {
            name: "g".into(),
            members: vec![("a".into(), HostOperation::UiFocus); 64],
        },
        GroupLocalType {
            name: "g".into(),
            members: vec![("a".into(), HostOperation::UiFocus); 2],
        },
    ] {
        assert!(body.validate_in_group_scope(&[], &[forged]).is_err());
    }
    assert!(
        body.validate_in_group_scope(&[("g".into(), Type::RuntimeList)], &[group])
            .is_err()
    );
}
