use leselang_hir::computation::ScalarType;
use leselang_hir::{Type, authorize, canonical_source, lower};
use leselang_host_contract::CapabilitySet;
use leselang_syntax::parse;

#[test]
fn parallel_members_prepare_one_tail_or_capture_with_closed_typed_aliases() {
    for (expression, ty) in [
        (
            r#"bind(g: all(a: runtime.list(), b: runtime.list()), body: runtime.list(role: to_string(value: field(value: member(value: g, name: "b"), name: "count"))))"#,
            Type::RuntimeList,
        ),
        (
            r#"bind(prefix: "node-", body: bind(g: all(a: ui.focus(node_id: concat(left: prefix, right: "a")), b: ui.assert_selection(node_id: "toggle", state: "selected")), body: bind(alias: g, body: bind(state: member(value: alias, name: "b"), body: bind(r: ui.focus(node_id: field(value: member(value: g, name: "a"), name: "node_id")), body: and(left: field(value: state, name: "selected"), right: eq(left: field(value: r, name: "node_id"), right: field(value: member(value: alias, name: "a"), name: "node_id"))))))))"#,
            Type::Scalar(ScalarType::Boolean),
        ),
        (
            r#"bind(g: all(a: runtime.list(), b: runtime.list()), body: choose(when: true, then: bind(r: ui.focus(node_id: "a"), body: field(value: member(value: g, name: "a"), name: "count")), otherwise: bind(r: runtime.list(), body: field(value: r, name: "count"))))"#,
            Type::Scalar(ScalarType::Integer),
        ),
    ] {
        let source = format!("fn main() = {expression}");
        let program = lower(&parse(&source)).unwrap();
        assert_eq!(program.function.result_type, ty);
        assert_eq!(
            lower(&parse(&canonical_source(&program.function.effect).unwrap())).unwrap(),
            program
        );
    }
}

#[test]
fn parallel_tails_preflight_cold_capabilities_and_reject_dynamic_group_shapes() {
    let program = lower(&parse(r#"fn main() = bind(g: all(a: runtime.list(), b: runtime.list()), body: choose(when: true, then: bind(r: ui.focus(node_id: "a"), body: true), otherwise: bind(r: debugger.cancel(session_id: "s"), body: false)))"#)).unwrap();
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
    for source in [
        r#"fn main() = bind(g: all(a: runtime.list()), body: runtime.list())"#,
        r#"fn main() = bind(g: all(a: runtime.list(), b: runtime.list()), body: seq(c: runtime.list()))"#,
        r#"fn main() = bind(g: all(a: runtime.list(), b: runtime.list()), body: choose(when: true, then: true, otherwise: bind(r: runtime.list(), body: 0)))"#,
        r#"fn main() = bind(g: all(a: runtime.list(), b: runtime.list()), body: bind(r: runtime.list(), body: bind(s: seq(c: runtime.list()), body: true)))"#,
        r#"fn main() = bind(g: all(a: runtime.list(), b: runtime.list()), body: ui.focus(node_id: field(value: member(value: g, name: "missing"), name: "node_id")))"#,
    ] {
        assert!(lower(&parse(source)).is_err(), "{source}");
    }
}

#[test]
fn parallel_prefix_reserves_one_slot_without_reducing_pure_all_limits() {
    for count in [2, 63, 64] {
        let members = (0..count)
            .map(|index| format!("m{index}: runtime.list()"))
            .collect::<Vec<_>>()
            .join(", ");
        for tail in ["runtime.list()", "bind(r: runtime.list(), body: true)"] {
            let source = format!("fn main() = bind(g: all({members}), body: {tail})");
            assert_eq!(
                lower(&parse(&source)).is_ok(),
                count < 64,
                "{count}: {tail}"
            );
        }
        assert!(
            lower(&parse(&format!(
                "fn main() = bind(g: all({members}), body: true)"
            )))
            .is_ok()
        );
    }
}
