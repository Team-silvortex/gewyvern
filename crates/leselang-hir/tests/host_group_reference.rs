use leselang_hir::computation::Computation;
use leselang_hir::{Effect, Type, authorize, canonical_source, lower};
use leselang_host_contract::CapabilitySet;
use leselang_runtime_core::ScalarType;
use leselang_syntax::parse;

fn roundtrip(source: &str) -> leselang_hir::HirProgram {
    let program = lower(&parse(source)).unwrap();
    let wire = serde_json::to_vec(&program).unwrap();
    let canonical = lower(&parse(&canonical_source(&program.function.effect).unwrap())).unwrap();
    assert_eq!(serde_json::to_vec(&canonical).unwrap(), wire);
    let Effect::Compute { expression } = &program.function.effect else {
        panic!()
    };
    assert_eq!(
        expression.validate_in_scope(&[]).unwrap(),
        program.function.result_type
    );
    program
}

#[test]
fn mixed_flat_sequence_parallel_groups_keep_product_types_wire_and_capabilities() {
    for mode in ["seq", "all"] {
        let program = roundtrip(&format!(
            r#"fn main() = bind(g: {mode}(native: ui.focus(node_id: "a"), computed: ui.focus(node_id: concat(left: "b", right: ""))), body: field(value: member(value: g, name: "native"), name: "node_id"))"#
        ));
        assert_eq!(
            program.function.result_type,
            Type::Scalar(ScalarType::String)
        );
        let Effect::Compute { expression } = &program.function.effect else {
            panic!()
        };
        let Computation::Bind { value, .. } = expression.as_ref() else {
            panic!()
        };
        let Computation::Group { branches, .. } = value.as_ref() else {
            panic!()
        };
        assert!(matches!(branches[0].value, Computation::Host { .. }));
        assert!(matches!(branches[1].value, Computation::Call { .. }));
        assert!(authorize(&program, &CapabilitySet::default()).is_err());
        authorize(&program, &CapabilitySet::new(["ui.presentation"])).unwrap();
    }
}

#[test]
fn conditional_host_call_preparation_and_surrounding_native_captures_share_closed_groups() {
    for source in [
        r#"fn main() = bind(g: seq(move: choose(when: false, then: ui.focus(node_id: "a"), otherwise: ui.focus(node_id: concat(left: "b", right: "")))), body: field(value: member(value: g, name: "move"), name: "node_id"))"#,
        r#"fn main() = bind(g: all(native: ui.focus(node_id: "a"), computed: ui.focus(node_id: concat(left: "b", right: ""))), body: bind(r: runtime.list(), body: field(value: r, name: "count")))"#,
        r#"fn focus(node: string) = ui.focus(node_id: node)
fn main() = bind(g: seq(native: ui.focus(node_id: "a"), helper: focus(node: "b")), body: field(value: member(value: g, name: "helper"), name: "node_id"))"#,
    ] {
        roundtrip(source);
    }
}

#[test]
fn cold_group_failures_keep_native_source_diagnostics_and_do_not_become_execution_grants() {
    for source in [
        r#"seq(native: ui.focus(node_id: "a"), computed: ui.focus(node_id: 7))"#,
        r#"seq(move: choose(when: false, then: runtime.list(), otherwise: ui.focus(node_id: concat(left: "a", right: ""))))"#,
        r#"seq(move: bind(r: runtime.list(), body: ui.focus(node_id: concat(left: "a", right: ""))))"#,
        r#"seq(native: ui.focus(node_id: "a"), computed: ui.focus(node_id: missing))"#,
        r#"bind(r: runtime.list(), body: bind(g: all(native: ui.focus(node_id: "a"), computed: ui.focus(node_id: concat(left: "b", right: ""))), body: field(value: r, name: "count")))"#,
    ] {
        assert!(
            lower(&parse(&format!("fn main() = {source}"))).is_err(),
            "{source}"
        );
    }
    let program = roundtrip(
        r#"fn main() = bind(g: all(read: runtime.list(), focus: ui.focus(node_id: concat(left: "a", right: ""))), body: field(value: member(value: g, name: "read"), name: "count"))"#,
    );
    assert!(authorize(&program, &CapabilitySet::new(["ui.presentation"])).is_err());
    assert!(authorize(&program, &CapabilitySet::new(["runtime.read"])).is_err());
    authorize(
        &program,
        &CapabilitySet::new(["runtime.read", "ui.presentation"]),
    )
    .unwrap();
}

#[test]
fn forged_group_result_operation_and_native_payload_are_rejected_before_shared_type_exports() {
    let program = roundtrip(
        r#"fn main() = bind(g: seq(native: ui.focus(node_id: "a"), computed: ui.focus(node_id: concat(left: "b", right: ""))), body: field(value: member(value: g, name: "native"), name: "node_id"))"#,
    );
    let Effect::Compute { expression } = &program.function.effect else {
        panic!()
    };
    for variant in 0..3 {
        let mut forged = expression.as_ref().clone();
        let Computation::Bind { value, .. } = &mut forged else {
            panic!()
        };
        let Computation::Group { branches, .. } = value.as_mut() else {
            panic!()
        };
        match variant {
            0 => branches[0].result_type = Type::RuntimeList,
            1 => {
                let Computation::Host { effect } = &mut branches[0].value else {
                    panic!()
                };
                *effect.as_mut() = Effect::RuntimeList {
                    filter: Default::default(),
                };
            }
            2 => {
                let Computation::Host { effect } = &mut branches[0].value else {
                    panic!()
                };
                *effect.as_mut() = Effect::UiFocus { node_id: "".into() };
            }
            _ => unreachable!(),
        }
        assert!(forged.validate_in_scope(&[]).is_err());
    }
}
