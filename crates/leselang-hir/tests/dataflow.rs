use leselang_hir::computation::{Computation, ScalarType};
use leselang_hir::{Effect, Type, authorize, canonical_source, lower};
use leselang_host_contract::CapabilitySet;
use leselang_syntax::parse;

#[test]
fn nested_atomic_bindings_support_prior_results_and_canonical_roundtrips() {
    for expression in [
        r#"bind(r: runtime.list(), body: bind(other: runtime.list(), body: 1))"#,
        r#"bind(r: runtime.list(), body: bind(s: ui.focus(node_id: "a"), body: ui.focus(node_id: "b")))"#,
        r#"bind(r: ui.navigate_focus(node_id: "a", direction: "next"), body:
            bind(alias: r, body: bind(s: ui.focus(node_id: field(value: alias, name: "focused_node_id")), body:
                eq(left: field(value: r, name: "focused_node_id"), right: field(value: s, name: "node_id")))))"#,
        r#"bind(r: runtime.list(), body: choose(when: eq(left: field(value: r, name: "count"), right: 0),
            then: bind(s: ui.focus(node_id: "a"), body: true),
            otherwise: bind(s: ui.focus(node_id: "b"), body: false)))"#,
    ] {
        let program = lower(&parse(&format!("fn main() = {expression}"))).unwrap();
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
fn chains_preserve_lexical_types_capabilities_and_effect_boundaries() {
    for expression in [
        r#"bind(r: runtime.list(), body: bind(s: ui.focus(node_id: "a"), body: field(value: r, name: "node_id")))"#,
        r#"bind(r: runtime.list(), body: bind(r: runtime.list(), body: true))"#,
        r#"bind(r: runtime.list(), body: bind(s: ui.focus(node_id: field(value: later, name: "node_id")), body: 1))"#,
        r#"bind(r: runtime.list(), body: bind(s: runtime.list(), body: s))"#,
        r#"bind(r: runtime.list(), body: bind(s: runtime.list(), body: seq(a: runtime.list())))"#,
        r#"bind(r: runtime.list(), body: loop(n: 0, while: false, next: bind(s: runtime.list(), body: 1), limit: 0))"#,
        r#"seq(a: bind(r: runtime.list(), body: bind(s: runtime.list(), body: 1)))"#,
    ] {
        assert!(
            lower(&parse(&format!("fn main() = {expression}"))).is_err(),
            "{expression}"
        );
    }
    let program = lower(&parse(
        r#"fn main() = bind(r: runtime.list(), body:
        choose(when: true, then: bind(a: runtime.list(), body: true),
            otherwise: bind(b: runtime.refresh(runtime_id: "x"), body: true)))"#,
    ))
    .unwrap();
    assert!(authorize(&program, &CapabilitySet::new(["runtime.read"])).is_err());
}

#[test]
fn residual_scope_validation_rejects_forgery_and_keeps_depth_bounds() {
    let expression = Computation::Local {
        name: "saved".into(),
    };
    assert_eq!(
        expression
            .validate_in_scope(&[("saved".into(), Type::Scalar(ScalarType::Boolean))])
            .unwrap(),
        Type::Scalar(ScalarType::Boolean)
    );
    assert!(expression.validate_in_scope(&[]).is_err());
    assert!(
        expression
            .validate_in_scope(&[
                ("saved".into(), Type::UiFocus),
                ("saved".into(), Type::UiFocus)
            ])
            .is_err()
    );
    assert!(
        expression
            .validate_in_scope(&[("bad-name".into(), Type::UiFocus)])
            .is_err()
    );
    let forged = Computation::Bind {
        name: "saved".into(),
        value: Box::new(Computation::Host {
            effect: Box::new(Effect::RuntimeList {
                filter: Default::default(),
            }),
        }),
        body: Box::new(expression),
    };
    assert!(
        forged
            .validate_in_scope(&[("saved".into(), Type::UiFocus)])
            .is_err()
    );
    let mut source = "true".to_string();
    for i in 0..20 {
        source = format!("bind(r{i}: runtime.list(), body: {source})");
    }
    assert!(lower(&parse(&format!("fn main() = {source}"))).is_err());
}
