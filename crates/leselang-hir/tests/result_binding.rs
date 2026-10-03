use leselang_hir::computation::{Computation, ScalarType};
use leselang_hir::result_field::ResultField;
use leselang_hir::{Effect, Type, authorize, canonical_source, lower};
use leselang_host_contract::CapabilitySet;
use leselang_syntax::parse;

#[test]
fn atomic_results_have_typed_read_only_projections_and_canonical_roundtrips() {
    for (expression, ty) in [
        (
            r#"bind(r: runtime.list(), body: field(value: r, name: "revision"))"#,
            ScalarType::Integer,
        ),
        (
            r#"bind(r: runtime.history(runtime_id: "a"), body: field(name: "count", value: r))"#,
            ScalarType::Integer,
        ),
        (
            r#"bind(r: ui.navigate_focus(node_id: "a", direction: "next"), body: field(value: r, name: "focused_node_id"))"#,
            ScalarType::String,
        ),
        (
            r#"bind(r: ui.wait_form_field_max_length(node_id: "a", field: "name", max_length: "128"), body: field(value: r, name: "max_length"))"#,
            ScalarType::Integer,
        ),
        (
            r#"bind(expected: "a", body: bind(r: ui.focus(node_id: expected), body: eq(left: field(value: r, name: "node_id"), right: expected)))"#,
            ScalarType::Boolean,
        ),
        (
            r#"bind(r: runtime.list(), body: bind(alias: r, body: add(left: field(value: alias, name: "count"), right: 1)))"#,
            ScalarType::Integer,
        ),
        (
            r#"choose(when: false, then: bind(r: runtime.list(), body: field(value: r, name: "count")), otherwise: 0)"#,
            ScalarType::Integer,
        ),
        (
            r#"bind(r: ui.focus(node_id: "a"), body: choose(when: true, then: none, otherwise: none))"#,
            ScalarType::None,
        ),
    ] {
        let program = lower(&parse(&format!("fn main() = {expression}"))).unwrap();
        assert_eq!(program.function.result_type, Type::Scalar(ty));
        authorize(
            &program,
            &CapabilitySet::new(["runtime.read", "ui.presentation"]),
        )
        .unwrap();
        assert_eq!(
            lower(&parse(&canonical_source(&program.function.effect).unwrap())).unwrap(),
            program
        );
    }
}

#[test]
fn field_names_types_purity_and_result_binding_boundaries_are_checked_statically() {
    for expression in [
        r#"bind(r: runtime.list(), body: field(value: r, name: "node_id"))"#,
        r#"bind(r: runtime.list(), body: field(value: r, name: "runtimes"))"#,
        r#"bind(r: ui.focus(node_id: "a"), body: field(value: r, name: "focused_node_id"))"#,
        r#"bind(r: ui.focus(node_id: "a"), body: field(value: r, name: "__proto__"))"#,
        r#"bind(r: ui.focus(node_id: "a"), body: field(value: r, name: concat(left: "node", right: "_id")))"#,
        r#"field(value: runtime.list(), name: "revision")"#,
        r#"field(value: 1, name: "count")"#,
        r#"bind(r: runtime.list(), body: field(value: missing, name: "count"))"#,
        r#"bind(r: runtime.list(), body: field(value: r, name: "count", extra: 0))"#,
        r#"bind(r: runtime.list(), body: add(left: field(value: r, name: "count"), right: "1"))"#,
        r#"bind(r: runtime.list(), body: r)"#,
        r#"bind(r: runtime.list(), body: bind(r: 1, body: r))"#,
        r#"bind(r: runtime.list(), body: choose(when: true, then: 1, otherwise: field(value: r, name: "missing")))"#,
        r#"bind(r: seq(read: runtime.list()), body: bind(next: runtime.list(), body: bind(last: seq(read: runtime.list()), body: true)))"#,
        r#"bind(r: all(a: runtime.list(), b: runtime.list()), body: all(c: runtime.list(), d: runtime.list()))"#,
        r#"bind(r: choose(when: true, then: runtime.list(), otherwise: bind(hidden: runtime.list(), body: runtime.list())), body: 1)"#,
        r#"seq(step: bind(r: runtime.list(), body: 1))"#,
        r#"all(a: bind(r: runtime.list(), body: 1), b: runtime.list())"#,
        r#"repeat(times: 2, body: bind(r: runtime.list(), body: 1))"#,
        r#"add(left: bind(r: runtime.list(), body: 1), right: 1)"#,
        r#"len(value: bind(r: runtime.list(), body: "a"))"#,
        r#"choose(when: bind(r: runtime.list(), body: true), then: 1, otherwise: 0)"#,
        r#"ui.focus(node_id: bind(r: runtime.list(), body: "a"))"#,
        r#"bind(n: bind(r: runtime.list(), body: 1), body: n)"#,
    ] {
        let errors = lower(&parse(&format!("fn main() = {expression}"))).unwrap_err();
        assert!(
            errors.iter().all(|error| error.span.is_some()),
            "{expression}"
        );
    }
}

#[test]
fn result_binding_preserves_cold_branch_authority_and_rejects_forged_field_hir() {
    let program = lower(&parse(
        r#"fn main() = choose(when: true,
        then: bind(r: runtime.list(), body: field(value: r, name: "count")),
        otherwise: bind(r: runtime.refresh(runtime_id: "a"), body: 0))"#,
    ))
    .unwrap();
    assert_eq!(
        program.function.required_capabilities,
        ["runtime.read", "runtime.refresh"]
    );
    assert_eq!(
        authorize(&program, &CapabilitySet::new(["runtime.read"]))
            .unwrap_err()
            .code,
        "LSH2001"
    );
    let mut forged = program.clone();
    forged.function.required_capabilities = vec!["runtime.read".into()];
    assert_eq!(
        authorize(
            &forged,
            &CapabilitySet::new(["runtime.read", "runtime.refresh"])
        )
        .unwrap_err()
        .code,
        "LSH2002"
    );
    forged.function.effect = Effect::Compute {
        expression: Box::new(Computation::Field {
            value: Box::new(Computation::Local {
                name: "unknown".into(),
            }),
            field: ResultField::NodeId,
        }),
    };
    assert!(canonical_source(&forged.function.effect).is_err());
}
