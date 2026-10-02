use leselang_hir::computation::ScalarType;
use leselang_hir::{Type, authorize, canonical_source, lower};
use leselang_host_contract::CapabilitySet;
use leselang_syntax::parse;

#[test]
fn conditional_exits_remain_typed_and_canonical_through_nested_captures() {
    for expression in [
        r#"bind(r: runtime.list(), body: choose(when: true, then: 1, otherwise: bind(s: runtime.list(), body: 1)))"#,
        r#"bind(r: runtime.list(), body: choose(when: false, then: bind(s: runtime.list(), body: 1), otherwise: field(value: r, name: "count")))"#,
        r#"bind(r: runtime.list(), body: bind(n: field(value: r, name: "count"), body:
            choose(when: eq(left: n, right: 0), then: loop(i: 0, while: lt(left: i, right: 2), next: add(left: i, right: 1), limit: 2),
                otherwise: bind(s: runtime.list(), body: choose(when: true, then: n,
                    otherwise: bind(t: runtime.list(), body: field(value: t, name: "count")))))))"#,
    ] {
        let program = lower(&parse(&format!("fn main() = {expression}"))).unwrap();
        assert_eq!(
            program.function.result_type,
            Type::Scalar(ScalarType::Integer)
        );
        authorize(&program, &CapabilitySet::new(["runtime.read"])).unwrap();
        assert_eq!(
            lower(&parse(&canonical_source(&program.function.effect).unwrap())).unwrap(),
            program
        );
    }
}

#[test]
fn cold_conditional_branches_still_require_types_capabilities_and_atomic_boundaries() {
    for expression in [
        r#"bind(r: runtime.list(), body: choose(when: true, then: 1, otherwise: bind(s: runtime.list(), body: false)))"#,
        r#"bind(r: ui.focus(node_id: "a"), body: choose(when: true, then: r, otherwise: ui.focus(node_id: "b")))"#,
        r#"bind(r: runtime.list(), body: choose(when: true, then: 1, otherwise: bind(s: runtime.list(), body: field(value: missing, name: "count"))))"#,
        r#"bind(r: runtime.list(), body: choose(when: true, then: 1, otherwise: bind(r: runtime.list(), body: 1)))"#,
        r#"bind(r: runtime.list(), body: choose(when: true, then: 1, otherwise: bind(s: seq(a: runtime.list()), body: 1)))"#,
        r#"bind(r: runtime.list(), body: add(left: 1, right: choose(when: true, then: 1, otherwise: bind(s: runtime.list(), body: 1))))"#,
        r#"bind(r: runtime.list(), body: loop(i: 0, while: false, next: choose(when: true, then: 1, otherwise: bind(s: runtime.list(), body: 1)), limit: 0))"#,
    ] {
        assert!(
            lower(&parse(&format!("fn main() = {expression}"))).is_err(),
            "{expression}"
        );
    }
    let program = lower(&parse(r#"fn main() = bind(r: runtime.list(), body:
        choose(when: true, then: 1, otherwise: bind(s: runtime.refresh(runtime_id: "a"), body: 1)))"#)).unwrap();
    assert!(authorize(&program, &CapabilitySet::new(["runtime.read"])).is_err());
}
