use leselang_hir::computation::{Computation, ScalarType};
use leselang_hir::host_call::HostOperation;
use leselang_hir::{Effect, Type, authorize, canonical_source, lower};
use leselang_host_contract::CapabilitySet;
use leselang_syntax::parse;

#[test]
fn named_members_are_typed_and_canonical_for_sequences_parallel_groups_and_repeats() {
    for (expression, ty) in [
        (
            r#"bind(g: seq(read: runtime.list(), focus: ui.focus(node_id: "a")), body: field(value: member(value: g, name: "read"), name: "count"))"#,
            ScalarType::Integer,
        ),
        (
            r#"bind(g: all(read: runtime.list(), focus: ui.focus(node_id: "a")), body: field(value: member(name: "focus", value: g), name: "node_id"))"#,
            ScalarType::String,
        ),
        (
            r#"bind(g: repeat(times: 2, body: runtime.list()), body: eq(left: field(value: member(value: g, name: "iteration_1"), name: "count"), right: field(value: member(value: g, name: "iteration_2"), name: "count")))"#,
            ScalarType::Boolean,
        ),
        (
            r#"bind(prefix: "node-", body: bind(g: seq(first: ui.focus(node_id: concat(left: prefix, right: "a"))), body: bind(alias: g, body: bind(r: member(value: alias, name: "first"), body: field(value: r, name: "node_id")))))"#,
            ScalarType::String,
        ),
        (
            r#"bind(g: seq(nested: seq(read: runtime.list())), body: field(value: member(value: g, name: "nested__read"), name: "revision"))"#,
            ScalarType::Integer,
        ),
        (
            r#"bind(g: seq(read: runtime.list()), body: none)"#,
            ScalarType::None,
        ),
    ] {
        let program = lower(&parse(&format!("fn main() = {expression}"))).unwrap();
        assert_eq!(
            program.function.result_type,
            Type::Scalar(ty),
            "{expression}"
        );
        assert_eq!(
            lower(&parse(&canonical_source(&program.function.effect).unwrap())).unwrap(),
            program
        );
    }
}

#[test]
fn group_members_cannot_escape_static_names_types_or_pure_scalar_bodies() {
    for expression in [
        r#"member(value: missing, name: "a")"#,
        r#"bind(g: 1, body: member(value: g, name: "a"))"#,
        r#"bind(g: runtime.list(), body: member(value: g, name: "a"))"#,
        r#"bind(g: seq(a: runtime.list()), body: field(value: member(value: g, name: "missing"), name: "count"))"#,
        r#"bind(g: seq(a: runtime.list()), body: field(value: member(value: g, name: "a"), name: "node_id"))"#,
        r#"bind(g: seq(a: runtime.list()), body: field(value: member(value: g, name: to_string(value: "a")), name: "count"))"#,
        r#"bind(g: seq(a: runtime.list()), body: field(value: member(value: choose(when: true, then: g, otherwise: g), name: "a"), name: "count"))"#,
        r#"bind(g: seq(a: runtime.list()), body: member(value: g, name: "a"))"#,
        r#"bind(g: seq(a: runtime.list()), body: g)"#,
        r#"bind(g: seq(a: runtime.list()), body: runtime.list())"#,
        r#"bind(g: seq(a: runtime.list()), body: bind(r: runtime.list(), body: true))"#,
        r#"bind(r: runtime.list(), body: bind(g: seq(a: runtime.list()), body: true))"#,
        r#"bind(g: choose(when: true, then: seq(a: runtime.list()), otherwise: seq(b: runtime.list())), body: true)"#,
        r#"bind(g: all(a: seq(read: runtime.list()), b: runtime.list()), body: true)"#,
        r#"bind(g: seq(a: ui.focus(node_id: field(value: member(value: g, name: "a"), name: "node_id"))), body: true)"#,
        r#"bind(g: seq(a: runtime.list()), body: choose(when: true, then: 1, otherwise: field(value: member(value: g, name: "missing"), name: "count")))"#,
        r#"bind(g: seq(a: runtime.list()), body: member(value: g, name: "a", extra: 1))"#,
    ] {
        let errors = lower(&parse(&format!("fn main() = {expression}"))).unwrap_err();
        assert!(
            errors.iter().all(|error| error.span.is_some()),
            "{expression}"
        );
    }
}

#[test]
fn all_member_capabilities_are_required_even_for_unused_or_cold_results() {
    let program = lower(&parse(r#"fn main() = bind(g: all(read: runtime.list(), focus: ui.focus(node_id: "a")), body: true)"#)).unwrap();
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

#[test]
fn forged_member_operation_and_scope_are_rejected_on_canonical_revalidation() {
    let program = lower(&parse(r#"fn main() = bind(g: seq(a: ui.focus(node_id: "a")), body: field(value: member(value: g, name: "a"), name: "node_id"))"#)).unwrap();
    for change in 0..4 {
        let mut effect = program.function.effect.clone();
        let Effect::Compute { expression } = &mut effect else {
            panic!()
        };
        let Computation::Bind { body, .. } = expression.as_mut() else {
            panic!()
        };
        let Computation::Field { value, .. } = body.as_mut() else {
            panic!()
        };
        let Computation::Member {
            group,
            name,
            operation,
        } = value.as_mut()
        else {
            panic!()
        };
        match change {
            0 => *operation = HostOperation::UiActivate,
            1 => *group = "missing".into(),
            2 => *name = "missing".into(),
            _ => *name = "x".repeat(65),
        }
        assert!(canonical_source(&effect).is_err());
    }
}
