use leselang_hir::computation::{Computation, ScalarType};
use leselang_hir::host_call::HostOperation;
use leselang_hir::{Effect, Type, authorize, canonical_source, lower};
use leselang_host_contract::CapabilitySet;
use leselang_syntax::parse;

fn compile(source: &str) -> leselang_hir::HirProgram {
    let program = lower(&parse(source)).unwrap();
    assert_eq!(
        lower(&parse(&canonical_source(&program.function.effect).unwrap())).unwrap(),
        program
    );
    program
}

#[test]
fn pure_prepared_atomic_values_export_the_existing_typed_result_fields() {
    for (value, field, ty) in [
        (
            r#"bind(target: concat(left: "node-", right: "a"), body: ui.focus(node_id: target))"#,
            "node_id",
            ScalarType::String,
        ),
        (
            r#"choose(when: true, then: runtime.list(), otherwise: runtime.list(role: "edge"))"#,
            "count",
            ScalarType::Integer,
        ),
        (
            r#"choose(when: false, then: ui.assert_selection(node_id: "a", state: "selected"), otherwise: ui.assert_selection(node_id: "b", state: "unselected"))"#,
            "selected",
            ScalarType::Boolean,
        ),
        (
            r#"bind(target: "form", body: choose(when: false, then: ui.assert_form_field_placeholder(node_id: target, field: "name", expected: none), otherwise: ui.assert_form_field_placeholder(node_id: target, field: "name", expected: "")))"#,
            "optional_expected",
            ScalarType::OptionalString,
        ),
    ] {
        let program = compile(&format!(
            r#"fn main() = bind(result: {value}, body: field(value: result, name: "{field}"))"#
        ));
        assert_eq!(program.function.result_type, Type::Scalar(ty));
        authorize(
            &program,
            &CapabilitySet::new(["ui.presentation", "runtime.read"]),
        )
        .unwrap();
        assert!(authorize(&program, &CapabilitySet::default()).is_err());
        let Effect::Compute { expression } = &program.function.effect else {
            panic!()
        };
        assert!(expression.is_atomic_capture());
        assert!(expression.is_result_chain());
        assert_eq!(expression.atomic_flow_bound(), Some(1));
    }
}

#[test]
fn selection_between_atomic_helper_calls_keeps_arguments_pure_and_hygienic() {
    let program = compile(
        r#"fn focus(node: string) = bind(target: concat(left: "node-", right: node), body: ui.focus(node_id: target))
        fn main() = bind(target: "outer", body: bind(result: choose(when: true, then: focus(node: "a"), otherwise: focus(node: "b")), body: concat(left: target, right: field(value: result, name: "node_id"))))"#,
    );
    assert_eq!(
        program.function.result_type,
        Type::Scalar(ScalarType::String)
    );
    for source in [
        r#"fn focus(node: string) = ui.focus(node_id: node)
            fn main() = bind(result: choose(when: true, then: focus(node: bind(hidden: ui.focus(node_id: "a"), body: "a")), otherwise: focus(node: "b")), body: true)"#,
        r#"fn main() = bind(result: bind(target: "a", body: ui.focus(node_id: target)), body: target)"#,
    ] {
        assert!(lower(&parse(source)).is_err());
    }
}

#[test]
fn prepared_captures_compose_with_atomic_chains_and_conditional_scalar_exits() {
    let program = compile(
        r#"fn main() = bind(first: ui.assert_text(node_id: "status", expected: "ready"), body:
        bind(selected: bind(target: concat(left: "node-", right: field(value: first, name: "expected")), body:
            choose(when: starts_with(left: target, right: "node-"), then: ui.focus(node_id: target), otherwise: ui.focus(node_id: "fallback"))), body:
                choose(when: eq(left: field(value: selected, name: "node_id"), right: "fallback"), then: false, otherwise:
                    bind(written: ui.set_form_value(node_id: "form", field: "target", value: field(value: selected, name: "node_id")), body: true))))"#,
    );
    let Effect::Compute { expression } = program.function.effect else {
        panic!()
    };
    assert!(expression.is_result_flow());
    assert!(!expression.is_result_chain());
    assert!(!expression.is_atomic_capture());
    assert_eq!(expression.atomic_flow_bound(), Some(3));
}

#[test]
fn prepared_captures_after_groups_reserve_exactly_one_atomic_slot() {
    for kind in ["seq", "all"] {
        for count in [2, 63, 64] {
            let members = (0..count)
                .map(|index| format!(r#"step{index}: ui.focus(node_id: "a")"#))
                .collect::<Vec<_>>()
                .join(", ");
            let source = format!(
                r#"fn main() = bind(group: {kind}({members}), body: bind(result: choose(when: true, then: ui.focus(node_id: field(value: member(value: group, name: "step0"), name: "node_id")), otherwise: ui.focus(node_id: "b")), body: true))"#
            );
            if count == 64 {
                assert!(
                    lower(&parse(&source))
                        .unwrap_err()
                        .iter()
                        .any(|error| error.code == "LSH1412")
                );
            } else {
                let program = compile(&source);
                let Effect::Compute { expression } = program.function.effect else {
                    panic!()
                };
                let Computation::Bind { body, .. } = expression.as_ref() else {
                    panic!()
                };
                assert!(body.is_atomic_capture());
                assert_eq!(body.atomic_flow_bound(), Some(1));
            }
        }
    }
}

#[test]
fn cold_multistep_results_and_hidden_captures_do_not_become_atomic_values() {
    for value in [
        r#"bind(hidden: ui.focus(node_id: "a"), body: ui.focus(node_id: "b"))"#,
        r#"choose(when: true, then: ui.focus(node_id: "a"), otherwise: bind(hidden: ui.focus(node_id: "b"), body: ui.focus(node_id: "c")))"#,
        r#"choose(when: true, then: bind(hidden: ui.focus(node_id: "a"), body: 1), otherwise: bind(hidden: ui.focus(node_id: "b"), body: 1))"#,
        r#"bind(target: bind(hidden: ui.focus(node_id: "a"), body: "b"), body: ui.focus(node_id: target))"#,
        r#"recover(value: ui.focus(node_id: "a"), fallback: ui.focus(node_id: "b"))"#,
    ] {
        assert!(
            lower(&parse(&format!(
                "fn main() = bind(result: {value}, body: true)"
            )))
            .is_err(),
            "{value}"
        );
    }
    for source in [
        r#"fn main() = bind(result: choose(when: true, then: ui.focus(node_id: "a"), otherwise: ui.focus(node_id: "bad node")), body: true)"#,
        r#"fn main() = seq(first: bind(result: choose(when: true, then: ui.focus(node_id: "a"), otherwise: ui.focus(node_id: "b")), body: true))"#,
        r#"fn main() = add(left: bind(result: choose(when: true, then: runtime.list(), otherwise: runtime.list()), body: 1), right: 1)"#,
    ] {
        assert!(lower(&parse(source)).is_err(), "{source}");
    }
}

#[test]
fn forged_cold_operation_signatures_fail_full_hir_revalidation() {
    let mut program = compile(
        r#"fn main() = bind(result: choose(when: true, then: ui.focus(node_id: "a"), otherwise: ui.focus(node_id: "b")), body: field(value: result, name: "node_id"))"#,
    );
    let Effect::Compute { expression } = &mut program.function.effect else {
        panic!()
    };
    let Computation::Bind { value, .. } = expression.as_mut() else {
        panic!()
    };
    let Computation::Choose { otherwise, .. } = value.as_mut() else {
        panic!()
    };
    let Computation::Host { effect } = otherwise.as_mut() else {
        panic!()
    };
    **effect = Effect::UiActivate {
        node_id: "b".into(),
    };
    assert_eq!(value.prepared_atomic_operation(), None);
    assert!(!expression.is_result_flow());
    assert!(canonical_source(&program.function.effect).is_err());
    assert!(authorize(&program, &CapabilitySet::new(["ui.presentation"])).is_err());
    assert_ne!(
        HostOperation::UiActivate.result_type(),
        HostOperation::UiFocus.result_type()
    );
}
