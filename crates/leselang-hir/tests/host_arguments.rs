use leselang_hir::computation::{Computation, ComputedArgument, ScalarValue};
use leselang_hir::host_call::HostOperation;
use leselang_hir::{Effect, Type, authorize, canonical_source, lower};
use leselang_host_contract::CapabilitySet;
use leselang_syntax::parse;

#[test]
fn computed_host_arguments_have_fixed_operations_types_and_canonical_order() {
    for source in [
        r#"ui.focus(node_id: concat(left: "runtime-", right: "a"))"#,
        r#"bind(node: "runtime-a", body: ui.focus(node_id: node))"#,
        r#"bind(empty: none, body: runtime.list(role: empty))"#,
        r#"bind(node: "runtime-a", body: ui.navigate_focus(direction: choose(when: true, then: "next", otherwise: "last"), node_id: node))"#,
        r#"bind(empty: none, body: ui.wait_form_field_placeholder(expected: empty, field: "target", node_id: "a"))"#,
        r#"choose(when: true, then: ui.focus(node_id: concat(left: "a", right: "b")), otherwise: ui.focus(node_id: "c"))"#,
    ] {
        let program = lower(&parse(&format!("fn main() = {source}"))).unwrap();
        let canonical = canonical_source(&program.function.effect).unwrap();
        assert_eq!(lower(&parse(&canonical)).unwrap(), program);
        authorize(
            &program,
            &CapabilitySet::new(["runtime.read", "ui.presentation"]),
        )
        .unwrap();
    }
    let first = lower(&parse(
        r#"fn main() = ui.assert_text(expected: concat(left: "a", right: "b"), node_id: "a")"#,
    ))
    .unwrap();
    let second = lower(&parse(
        r#"fn main() = ui.assert_text(node_id: "a", expected: concat(right: "b", left: "a"))"#,
    ))
    .unwrap();
    assert_eq!(first, second);
}

#[test]
fn host_argument_preflight_rejects_authority_type_shape_and_static_domain_errors() {
    for source in [
        r#"ui.focus(node_id: missing)"#,
        r#"bind(node: true, body: ui.focus(node_id: node))"#,
        r#"bind(node: none, body: ui.focus(node_id: node))"#,
        r#"ui.assert_child_count(node_id: "a", count: add(left: 1, right: 1))"#,
        r#"ui.focus(node_id: runtime.list())"#,
        r#"ui.focus(node_id: concat(left: "a", right: "b"), node_id: "c")"#,
        r#"ui.focus(other: concat(left: "a", right: "b"))"#,
        r#"ui.assert_text(expected: concat(left: "a", right: "b"))"#,
        r#"ui.unknown(node_id: concat(left: "a", right: "b"))"#,
        r#"ui.navigate_focus(node_id: concat(left: "a", right: "b"), direction: "unknown")"#,
        r#"choose(when: true, then: ui.focus(node_id: "a"), otherwise: ui.focus(node_id: add(left: 1, right: 1)))"#,
        r#"choose(when: true, then: ui.navigate_focus(node_id: "a", direction: "next"), otherwise: ui.navigate_focus(node_id: concat(left: "a", right: "b"), direction: "unknown"))"#,
    ] {
        assert!(
            lower(&parse(&format!("fn main() = {source}"))).is_err(),
            "{source}"
        );
    }
    let mut program = lower(&parse(
        r#"fn main() = ui.focus(node_id: concat(left: "a", right: "b"))"#,
    ))
    .unwrap();
    assert_eq!(
        authorize(&program, &CapabilitySet::default())
            .unwrap_err()
            .code,
        "LSH2001"
    );
    program.function.required_capabilities.clear();
    assert_eq!(
        authorize(&program, &CapabilitySet::new(["ui.presentation"]))
            .unwrap_err()
            .code,
        "LSH2002"
    );
}

#[test]
fn forged_calls_cannot_smuggle_operations_locals_arguments_or_result_metadata() {
    let mut program = lower(&parse(
        r#"fn main() = ui.focus(node_id: concat(left: "a", right: "b"))"#,
    ))
    .unwrap();
    program.function.result_type = Type::RuntimeDeploy;
    assert_eq!(
        authorize(&program, &CapabilitySet::new(["ui.presentation"]))
            .unwrap_err()
            .code,
        "LSH1405"
    );
    for arguments in [
        vec![ComputedArgument {
            name: "node_id".into(),
            value: Computation::Local {
                name: "missing".into(),
            },
        }],
        vec![ComputedArgument {
            name: "node_id".into(),
            value: Computation::Host {
                effect: Box::new(Effect::RuntimeList {
                    filter: Default::default(),
                }),
            },
        }],
        vec![ComputedArgument {
            name: "node_id".into(),
            value: Computation::Literal {
                value: ScalarValue::String("x".repeat(4097)),
            },
        }],
        vec![ComputedArgument {
            name: "unknown".into(),
            value: Computation::Literal {
                value: ScalarValue::None,
            },
        }],
        vec![
            ComputedArgument {
                name: "node_id".into(),
                value: Computation::Literal {
                    value: ScalarValue::None
                }
            };
            1025
        ],
    ] {
        assert!(
            canonical_source(&Effect::Compute {
                expression: Box::new(Computation::Call {
                    operation: HostOperation::UiFocus,
                    arguments
                })
            })
            .is_err()
        );
    }
}
