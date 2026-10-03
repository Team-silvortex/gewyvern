use leselang_hir::{Type, authorize, canonical_source, lower};
use leselang_host_contract::CapabilitySet;
use leselang_syntax::parse;

#[test]
fn sequential_members_can_prepare_one_atomic_tail_with_aliases_helpers_and_decisions() {
    for source in [
        r#"fn main() = bind(g: seq(read: runtime.list()), body: ui.focus(node_id: to_string(value: field(value: member(value: g, name: "read"), name: "count"))))"#,
        r#"fn main() = bind(g: repeat(times: 2, body: ui.focus(node_id: "a")), body: ui.focus(node_id: field(value: member(value: g, name: "iteration_2"), name: "node_id")))"#,
        r#"fn main() = bind(prefix: "target-", body: bind(g: seq(first: ui.focus(node_id: concat(left: prefix, right: "a"))),
            body: bind(alias: g, body: ui.focus(node_id: field(value: member(value: alias, name: "first"), name: "node_id")))))"#,
        r#"fn pick(selected: boolean) = choose(when: selected, then: "run", otherwise: "skip")
            fn main() = bind(g: seq(check: ui.assert_selection(node_id: "a", state: "selected")),
                body: ui.focus(node_id: pick(selected: field(value: member(value: g, name: "check"), name: "selected"))))"#,
        r#"fn main() = bind(g: seq(a: ui.focus(node_id: "a")), body: choose(when: true,
            then: ui.focus(node_id: "b"), otherwise: ui.focus(node_id: concat(left: "bad", right: " node"))))"#,
    ] {
        let program = lower(&parse(source)).unwrap();
        assert_eq!(program.function.result_type, Type::UiFocus);
        assert_eq!(
            lower(&parse(&canonical_source(&program.function.effect).unwrap())).unwrap(),
            program
        );
    }
}

#[test]
fn group_tail_rejects_nested_reentry_and_mixed_return_shapes() {
    for source in [
        r#"fn main() = bind(g: all(a: ui.focus(node_id: "a"), b: ui.focus(node_id: "b")), body: all(c: ui.focus(node_id: "c"), d: ui.focus(node_id: "d")))"#,
        r#"fn main() = bind(g: seq(a: ui.focus(node_id: "a")), body: seq(next: ui.focus(node_id: "b")))"#,
        r#"fn main() = bind(g: seq(a: ui.focus(node_id: "a")), body: choose(when: true, then: true, otherwise: ui.focus(node_id: "b")))"#,
        r#"fn main() = bind(g: seq(a: ui.focus(node_id: "a")), body: ui.focus(node_id: field(value: member(value: g, name: "missing"), name: "node_id")))"#,
    ] {
        assert!(lower(&parse(source)).is_err(), "{source}");
    }
}

#[test]
fn tail_reserves_one_graph_slot_without_reducing_pure_group_limits() {
    for count in [1, 63, 64] {
        let source = format!(
            r#"fn main() = bind(g: repeat(times: {count}, body: ui.focus(node_id: "a")), body: ui.focus(node_id: "b"))"#
        );
        assert_eq!(lower(&parse(&source)).is_ok(), count <= 63);
        let pure = format!(
            r#"fn main() = bind(g: repeat(times: {count}, body: ui.focus(node_id: "a")), body: true)"#
        );
        assert!(lower(&parse(&pure)).is_ok());
    }
}

#[test]
fn complete_prefix_and_cold_tail_capabilities_are_preflighted_together() {
    let source = r#"fn main() = bind(g: seq(read: runtime.list()), body: choose(when: true,
        then: ui.focus(node_id: "a"), otherwise: ui.focus(node_id: "b")))"#;
    let program = lower(&parse(source)).unwrap();
    assert_eq!(
        program.function.required_capabilities,
        ["runtime.read", "ui.presentation"]
    );
    assert!(authorize(&program, &CapabilitySet::new(["runtime.read"])).is_err());
    assert!(authorize(&program, &CapabilitySet::new(["ui.presentation"])).is_err());
    authorize(
        &program,
        &CapabilitySet::new(["runtime.read", "ui.presentation"]),
    )
    .unwrap();
}
