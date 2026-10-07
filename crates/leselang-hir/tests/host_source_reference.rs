use leselang_hir::computation::Computation;
use leselang_hir::host_call::HostOperation;
use leselang_hir::{Effect, Type, authorize, canonical_source, lower};
use leselang_host_contract::CapabilitySet;
use leselang_syntax::{Span, parse};

fn round_trip(source: &str) -> leselang_hir::HirProgram {
    let program = lower(&parse(source)).unwrap();
    let canonical = canonical_source(&program.function.effect).unwrap();
    assert_eq!(
        serde_json::to_vec(&program).unwrap(),
        serde_json::to_vec(&lower(&parse(&canonical)).unwrap()).unwrap()
    );
    program
}

#[test]
fn reference_literal_host_leaves_keep_exact_native_operations_results_and_wire() {
    for (source, operation, ty) in [
        (
            "runtime.list()",
            HostOperation::RuntimeList,
            Type::RuntimeList,
        ),
        (
            "runtime.inspect(runtime_id: \"a\")",
            HostOperation::RuntimeInspect,
            Type::RuntimeInspect,
        ),
        (
            "ui.focus(node_id: \"target\")",
            HostOperation::UiFocus,
            Type::UiFocus,
        ),
    ] {
        let program = round_trip(&format!(
            "fn main() = choose(when: true, then: {source}, otherwise: {source})"
        ));
        assert_eq!(program.function.result_type, ty);
        let Effect::Compute { expression } = &program.function.effect else {
            panic!()
        };
        let Computation::Choose {
            then: then_branch,
            otherwise: else_branch,
            ..
        } = expression.as_ref()
        else {
            panic!()
        };
        for node in [then_branch.as_ref(), else_branch.as_ref()] {
            let Computation::Host { effect } = node else {
                panic!()
            };
            assert_eq!(HostOperation::for_effect(effect.as_ref()), Some(operation));
            assert_eq!(node.children().count(), 0);
        }
        assert!(authorize(&program, &CapabilitySet::default()).is_err());
        authorize(
            &program,
            &CapabilitySet::new(
                program
                    .function
                    .required_capabilities
                    .iter()
                    .map(String::as_str),
            ),
        )
        .unwrap();
    }
}

#[test]
fn helper_native_host_unwrapping_preserves_the_direct_atomic_product_bytes() {
    let helper = round_trip("fn read() = runtime.list()\nfn main() = read()");
    let direct = round_trip("fn main() = runtime.list()");
    assert_eq!(
        serde_json::to_vec(&helper).unwrap(),
        serde_json::to_vec(&direct).unwrap()
    );
    assert!(matches!(helper.function.effect, Effect::RuntimeList { .. }));
}

#[test]
fn native_leaf_errors_keep_original_messages_priority_and_shifted_source_spans() {
    for native in [
        "runtime.inspect(runtime_id: 7)",
        "runtime.list(extra: \"x\")",
        "ui.focus(node_id: \"\")",
        "unknown.call()",
    ] {
        let direct = format!("fn main() = {native}");
        let wrapped =
            format!("fn main() = choose(when: true, then: {native}, otherwise: runtime.list())");
        let expected = lower(&parse(&direct)).unwrap_err();
        let actual = lower(&parse(&wrapped)).unwrap_err();
        let shift = wrapped.find(native).unwrap() - direct.find(native).unwrap();
        assert_eq!(expected.len(), actual.len(), "{native}");
        for (expected, actual) in expected.iter().zip(actual) {
            assert_eq!(actual.code, expected.code, "{native}");
            assert_eq!(actual.message, expected.message, "{native}");
            assert_eq!(
                actual.span,
                expected.span.map(|span| Span {
                    start: span.start + shift,
                    end: span.end + shift
                }),
                "{native}"
            );
        }
    }
}

#[test]
fn literal_host_bridge_does_not_replace_the_computed_argument_call_path() {
    let program = round_trip(
        "fn main() = choose(when: true, then: ui.focus(node_id: \"target\"), otherwise: bind(node: \"target\", body: ui.focus(node_id: node)))",
    );
    let Effect::Compute { expression } = program.function.effect else {
        panic!()
    };
    let Computation::Choose {
        then: then_branch,
        otherwise: else_branch,
        ..
    } = expression.as_ref()
    else {
        panic!()
    };
    assert!(matches!(then_branch.as_ref(), Computation::Host { .. }));
    let Computation::Bind { body, .. } = else_branch.as_ref() else {
        panic!()
    };
    assert!(matches!(
        body.as_ref(),
        Computation::Call {
            operation: HostOperation::UiFocus,
            ..
        }
    ));
}

#[test]
fn native_host_leaves_still_supply_closed_group_members_and_projected_result_bindings() {
    for source in [
        "fn main() = bind(row: runtime.list(), body: field(value: row, name: \"count\"))",
        "fn main() = bind(rows: seq(a: runtime.list(), b: runtime.list()), body: field(value: member(value: rows, name: \"a\"), name: \"count\"))",
        "fn rows() = all(a: runtime.list(), b: runtime.list())\nfn main() = bind(rows: rows(), body: field(value: member(value: rows, name: \"b\"), name: \"revision\"))",
    ] {
        let program = round_trip(source);
        assert_eq!(program.function.required_capabilities, ["runtime.read"]);
        assert_eq!(
            program.function.result_type,
            Type::Scalar(leselang_runtime_core::ScalarType::Integer)
        );
    }
}

#[test]
fn cold_unselected_native_effects_remain_validated_before_any_execution() {
    let errors = lower(&parse("fn main() = choose(when: false, then: runtime.inspect(runtime_id: 7), otherwise: runtime.list())")).unwrap_err();
    assert!(errors.iter().all(|error| error.code != "LSH1999"));
    let expected = lower(&parse("fn main() = runtime.inspect(runtime_id: 7)")).unwrap_err();
    assert_eq!(
        errors
            .iter()
            .map(|error| (&error.code, &error.message))
            .collect::<Vec<_>>(),
        expected
            .iter()
            .map(|error| (&error.code, &error.message))
            .collect::<Vec<_>>()
    );
}
