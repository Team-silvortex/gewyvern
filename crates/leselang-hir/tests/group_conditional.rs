use leselang_hir::computation::{Computation, ScalarType};
use leselang_hir::{Effect, Type, authorize, canonical_source, lower};
use leselang_host_contract::CapabilitySet;
use leselang_syntax::parse;

#[test]
fn group_results_and_atomic_results_can_select_typed_scalar_early_exits() {
    for group in ["seq", "all"] {
        for tail in [
            r#"choose(when: eq(left: field(value: member(value: g, name: "a"), name: "count"), right: 0), then: true, otherwise: bind(r: ui.focus(node_id: "a"), body: true))"#,
            r#"bind(r: ui.focus(node_id: "a"), body: choose(when: eq(left: field(value: r, name: "node_id"), right: "a"), then: true, otherwise: bind(s: runtime.list(), body: eq(left: field(value: s, name: "count"), right: 0))))"#,
            r#"bind(alias: g, body: choose(when: true, then: 0, otherwise: bind(r: ui.focus(node_id: "a"), body: bind(previous: r, body: choose(when: false, then: field(value: member(value: alias, name: "a"), name: "count"), otherwise: bind(s: ui.focus(node_id: field(value: previous, name: "node_id")), body: 1))))))"#,
        ] {
            let source = format!(
                "fn main() = bind(g: {group}(a: runtime.list(), b: runtime.list()), body: {tail})"
            );
            let program = lower(&parse(&source)).unwrap();
            assert!(matches!(program.function.result_type, Type::Scalar(_)));
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
            assert!(body.is_result_flow());
            assert!(!body.is_result_chain());
            assert_eq!(body.atomic_chain_bound(), None);
            assert_eq!(
                body.atomic_flow_bound(),
                Some(if tail.contains("bind(s:") { 2 } else { 1 })
            );
            assert_eq!(
                body.can_return_without_suspending(),
                !tail.starts_with("bind(r:")
            );
        }
    }
}

#[test]
fn conditional_flows_reserve_the_longest_cold_path_within_sixty_four_slots() {
    for bound in [1, 2, 3] {
        let mut tail = "true".to_string();
        for index in 0..bound {
            tail = format!(
                "choose(when: true, then: true, otherwise: bind(r{index}: runtime.list(), body: {tail}))"
            );
        }
        for count in [64 - bound, 65 - bound] {
            for group in ["seq", "all"] {
                let members = (0..count)
                    .map(|index| format!("m{index}: runtime.list()"))
                    .collect::<Vec<_>>()
                    .join(", ");
                let source = format!("fn main() = bind(g: {group}({members}), body: {tail})");
                assert_eq!(
                    lower(&parse(&source)).is_ok(),
                    count + bound <= 64,
                    "{source}"
                );
            }
        }
    }
}

#[test]
fn early_exit_does_not_hide_cold_permissions_or_invalid_types_and_shapes() {
    for group in ["seq", "all"] {
        let program = lower(&parse(&format!(
            r#"fn main() = bind(g: {group}(a: runtime.list(), b: runtime.list()), body: choose(when: true, then: true, otherwise: bind(r: debugger.cancel(session_id: "s"), body: bind(t: ui.focus(node_id: "a"), body: false))))"#
        ))).unwrap();
        assert_eq!(
            program.function.result_type,
            Type::Scalar(ScalarType::Boolean)
        );
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
        for tail in [
            r#"choose(when: true, then: true, otherwise: bind(r: runtime.list(), body: 1))"#,
            r#"choose(when: true, then: true, otherwise: runtime.list())"#,
            r#"choose(when: true, then: true, otherwise: bind(r: runtime.list(), body: field(value: member(value: g, name: "missing"), name: "count")))"#,
            r#"choose(when: true, then: true, otherwise: bind(r: ui.focus(node_id: field(value: ui.focus(node_id: "a"), name: "node_id")), body: true))"#,
            r#"choose(when: eq(left: field(value: runtime.list(), name: "count"), right: 0), then: true, otherwise: bind(r: runtime.list(), body: true))"#,
            r#"choose(when: true, then: true, otherwise: bind(r: runtime.list(), body: bind(s: seq(c: runtime.list()), body: true)))"#,
            r#"choose(when: true, then: true, otherwise: bind(g: runtime.list(), body: true))"#,
            r#"choose(when: true, then: 0, otherwise: loop(i: 0, while: false, next: bind(r: runtime.list(), body: 1), limit: 0))"#,
        ] {
            let source = format!(
                "fn main() = bind(g: {group}(a: runtime.list(), b: runtime.list()), body: {tail})"
            );
            assert!(lower(&parse(&source)).is_err(), "{source}");
        }
    }
}

#[test]
fn pure_group_choices_remain_pure_and_reserve_no_atomic_slots() {
    let program = lower(&parse("fn main() = bind(g: seq(a: runtime.list()), body: choose(when: true, then: 1, otherwise: 2))")).unwrap();
    let Effect::Compute { expression } = program.function.effect else {
        panic!()
    };
    let Computation::Bind { body, .. } = *expression else {
        panic!()
    };
    assert!(body.is_pure());
    assert_eq!(body.atomic_chain_bound(), Some(0));
    assert_eq!(body.atomic_flow_bound(), Some(0));
}
