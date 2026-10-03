use leselang_hir::computation::{Computation, ScalarType};
use leselang_hir::{Effect, Type, authorize, canonical_source, lower};
use leselang_host_contract::CapabilitySet;
use leselang_syntax::parse;

#[test]
fn typed_group_and_atomic_aliases_survive_multiple_post_group_captures() {
    for group in [
        r#"seq(a: ui.focus(node_id: "a"), b: runtime.list())"#,
        r#"all(a: ui.focus(node_id: "a"), b: runtime.list())"#,
    ] {
        let source = format!(
            r#"fn main() = bind(g: {group}, body:
            bind(r: ui.focus(node_id: field(value: member(value: g, name: "a"), name: "node_id")), body:
                bind(alias: g, body: bind(previous: r, body:
                    bind(s: ui.focus(node_id: field(value: previous, name: "node_id")), body:
                        eq(left: field(value: s, name: "node_id"), right: field(value: member(value: alias, name: "a"), name: "node_id")))))))"#
        );
        let program = lower(&parse(&source)).unwrap();
        assert_eq!(
            program.function.result_type,
            Type::Scalar(ScalarType::Boolean)
        );
        assert_eq!(
            lower(&parse(&canonical_source(&program.function.effect).unwrap())).unwrap(),
            program
        );
        let Effect::Compute { expression } = &program.function.effect else {
            panic!()
        };
        let Computation::Bind { body, .. } = expression.as_ref() else {
            panic!()
        };
        assert!(body.is_result_chain());
        assert!(!body.is_atomic_capture());
        assert_eq!(body.atomic_chain_bound(), Some(2));
    }
}

#[test]
fn longest_cold_path_reserves_graph_slots_and_all_capabilities() {
    for count in [1, 62, 63, 64] {
        let source = format!(
            r#"fn main() = bind(g: repeat(times: {count}, body: runtime.list()), body:
            choose(when: true, then: bind(r: ui.focus(node_id: "a"), body: true),
                otherwise: bind(r: debugger.cancel(session_id: "s"), body: bind(s: ui.focus(node_id: "b"), body: false))))"#
        );
        let program = lower(&parse(&source));
        assert_eq!(program.is_ok(), count <= 62);
        if let Ok(program) = program {
            assert_eq!(
                program.function.required_capabilities,
                ["debugger.control", "runtime.read", "ui.presentation"]
            );
            assert!(
                authorize(
                    &program,
                    &CapabilitySet::new(["runtime.read", "ui.presentation"])
                )
                .is_err()
            );
        }
    }
    for count in [2, 62, 63] {
        let members = (0..count)
            .map(|index| format!("m{index}: runtime.list()"))
            .collect::<Vec<_>>()
            .join(", ");
        let source = format!(
            "fn main() = bind(g: all({members}), body: bind(r: runtime.list(), body: bind(s: runtime.list(), body: true)))"
        );
        assert_eq!(lower(&parse(&source)).is_ok(), count <= 62);
    }
}

#[test]
fn group_chains_keep_effectful_operands_guards_loops_and_nested_groups_fenced() {
    for tail in [
        r#"bind(r: ui.focus(node_id: "a"), body: choose(when: eq(left: field(value: ui.focus(node_id: "b"), name: "node_id"), right: "b"), then: true, otherwise: bind(s: ui.focus(node_id: "b"), body: true)))"#,
        r#"bind(r: ui.focus(node_id: field(value: ui.focus(node_id: "a"), name: "node_id")), body: true)"#,
        r#"bind(r: runtime.list(), body: bind(s: all(a: runtime.list(), b: runtime.list()), body: true))"#,
        r#"bind(r: runtime.list(), body: loop(i: 0, while: false, next: bind(s: runtime.list(), body: 1), limit: 0))"#,
    ] {
        let source = format!("fn main() = bind(g: seq(a: runtime.list()), body: {tail})");
        assert!(lower(&parse(&source)).is_err(), "{tail}");
    }
}
