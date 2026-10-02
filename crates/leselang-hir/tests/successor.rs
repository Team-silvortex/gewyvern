use leselang_hir::{authorize, canonical_source, lower};
use leselang_host_contract::CapabilitySet;
use leselang_syntax::parse;

#[test]
fn one_atomic_successor_supports_result_projections_pure_locals_and_lazy_selection() {
    for expression in [
        r#"bind(r: ui.navigate_focus(node_id: "a", direction: "next"), body: ui.focus(node_id: field(value: r, name: "focused_node_id")))"#,
        r#"bind(prefix: "target-", body: bind(r: ui.focus(node_id: "a"), body: bind(id: concat(left: prefix, right: field(value: r, name: "node_id")), body: ui.focus(node_id: id))))"#,
        r#"bind(r: runtime.list(), body: choose(when: eq(left: field(value: r, name: "count"), right: 0), then: ui.focus(node_id: "empty"), otherwise: ui.focus(node_id: "populated")))"#,
        r#"bind(r: runtime.list(), body: ui.focus(node_id: choose(when: eq(left: loop(n: field(value: r, name: "count"), while: lt(left: n, right: 2), next: add(left: n, right: 1), limit: 2), right: 2), then: "a", otherwise: "b")))"#,
    ] {
        let program = lower(&parse(&format!("fn main() = {expression}"))).unwrap();
        authorize(
            &program,
            &CapabilitySet::new(["ui.presentation", "runtime.read"]),
        )
        .unwrap();
        assert_eq!(
            lower(&parse(&canonical_source(&program.function.effect).unwrap())).unwrap(),
            program
        );
    }
}

#[test]
fn successors_do_not_enable_dynamic_groups_or_effectful_operands() {
    for expression in [
        r#"bind(r: runtime.list(), body: seq(a: ui.focus(node_id: "a")))"#,
        r#"bind(r: runtime.list(), body: all(a: ui.focus(node_id: "a"), b: ui.focus(node_id: "b")))"#,
        r#"bind(r: runtime.list(), body: repeat(times: 2, body: ui.focus(node_id: "a")))"#,
        r#"bind(r: runtime.list(), body: choose(when: false, then: bind(s: runtime.list(), body: 1), otherwise: ui.focus(node_id: "b")))"#,
        r#"bind(r: runtime.list(), body: choose(when: true, then: ui.focus(node_id: "a"), otherwise: 1))"#,
        r#"bind(r: runtime.list(), body: ui.focus(node_id: bind(s: runtime.list(), body: "a")))"#,
        r#"bind(r: runtime.list(), body: loop(n: 0, while: true, next: ui.focus(node_id: "a"), limit: 1))"#,
        r#"bind(r: runtime.list(), body: ui.focus(node_id: field(value: missing, name: "node_id")))"#,
    ] {
        assert!(
            lower(&parse(&format!("fn main() = {expression}"))).is_err(),
            "{expression}"
        );
    }
}

#[test]
fn successor_authority_includes_the_unselected_branch_before_the_first_effect() {
    let program = lower(&parse(
        r#"fn main() = bind(r: runtime.list(), body:
        choose(when: true, then: runtime.list(), otherwise: runtime.list(role: "sensor")))"#,
    ))
    .unwrap();
    assert_eq!(program.function.required_capabilities, ["runtime.read"]);
    let program = lower(&parse(r#"fn main() = bind(r: runtime.list(), body:
        choose(when: true, then: runtime.refresh(runtime_id: "a"), otherwise: runtime.refresh(runtime_id: "b")))"#)).unwrap();
    assert_eq!(
        program.function.required_capabilities,
        ["runtime.read", "runtime.refresh"]
    );
    assert!(authorize(&program, &CapabilitySet::new(["runtime.read"])).is_err());
}
