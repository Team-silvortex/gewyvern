use leselang_product_hir_boundary::{
    CapabilitySet, Effect, HirProgram, Type, authorize, canonical_source, lower,
};
use leselang_runtime_core::ScalarType;
use leselang_syntax::parse;

#[test]
fn foreign_product_types_reuse_the_language_ir_without_an_orphan_impl() {
    let program = lower(&parse(
        "fn main() = bind(x: 1, body: add(left: x, right: 2))",
    ))
    .unwrap();
    assert_eq!(
        program.function.result_type,
        Type::Scalar(ScalarType::Integer)
    );
    let Effect::Compute { expression } = &program.function.effect else {
        panic!("expected scalar computation");
    };
    let native: &leselang_hir::ir::Computation<_, _, Effect, Type> = expression;
    assert!(native.is_pure());
    native.validate_structure().unwrap();
    assert_eq!(
        native.validate_in_scope(&[]),
        Ok(Type::Scalar(ScalarType::Integer))
    );
    assert!(native.required_capabilities().is_empty());
}

#[test]
fn foreign_product_lowering_preserves_canonical_and_wire_round_trips() {
    for source in [
        "fn main() = runtime.list()",
        "fn main() = runtime.inspect(runtime_id: \"runtime-a\")",
        "fn main() = ui.focus(node_id: \"runtime-card\")",
        "fn main() = bind(x: 1, body: add(left: x, right: 2))",
        "fn main() = loop(n: 0, while: false, next: n, limit: 0)",
        "fn main() = fold(n: 0, items: strings(), item: \"entry\", next: n, limit: 0)",
        "fn main() = all(first: runtime.list(), second: runtime.list())",
        "fn main() = seq(first: runtime.list(), second: runtime.list())",
        "fn plus(x: integer) = add(left: x, right: 1)\nfn main() = plus(x: 2)",
    ] {
        let program = lower(&parse(source)).unwrap_or_else(|e| panic!("{source}: {e:?}"));
        let canonical = canonical_source(&program.function.effect).unwrap();
        assert_eq!(lower(&parse(&canonical)).unwrap(), program, "{source}");
        let wire = serde_json::to_string(&program).unwrap();
        assert_eq!(serde_json::from_str::<HirProgram>(&wire).unwrap(), program);
    }
}

#[test]
fn foreign_product_keeps_authority_checks_in_the_host_profile() {
    let mut program = lower(&parse("fn main() = runtime.list()")).unwrap();
    assert_eq!(
        authorize(&program, &CapabilitySet::default())
            .unwrap_err()
            .code,
        "LSH2001"
    );
    authorize(
        &program,
        &CapabilitySet::new([leselang_host_contract::CAPABILITY_RUNTIME_READ]),
    )
    .unwrap();
    program.function.required_capabilities.clear();
    assert_eq!(
        authorize(&program, &CapabilitySet::default())
            .unwrap_err()
            .code,
        "LSH2002"
    );
}

#[test]
fn foreign_product_still_checks_cold_operands_and_form_specific_local_names() {
    for source in [
        "fn main() = and(left: false, right: missing)",
        "fn main() = recover(value: 1, fallback: missing)",
        "fn main() = loop(n: 0, while: false, next: missing, limit: 0)",
        "fn main() = fold(n: 0, items: strings(), item: \"entry\", next: missing, limit: 0)",
        "fn main() = loop(next: 0, while: false, next: next, limit: 0)",
        "fn main() = bind(x: 1, body: bind(x: 2, body: x))",
    ] {
        assert!(lower(&parse(source)).is_err(), "{source}");
    }
}

#[test]
fn upstream_public_vm_reuses_native_core_identity_and_cancellation_without_product_features() {
    use leselang_hir::{
        effect_evaluation::EffectEvaluationLimits,
        effect_session::{EffectSessionEnd, EffectSessionStatus},
        effect_stack::EffectStackLimits,
        effect_stack_session::StackedControlSession,
        pure_evaluation::PureEvaluationLimits,
    };
    use leselang_runtime_core::{Fuel, LocalExecutionScope};
    use leselang_vm::native::{LocalVm, Vm};

    let limits = EffectStackLimits {
        effects: EffectEvaluationLimits {
            pure: PureEvaluationLimits {
                max_nodes: 16,
                max_depth: 8,
                max_bindings: 4,
            },
            max_arguments: 4,
            max_branches: 4,
        },
        max_frames: 4,
    };
    let node = ();
    let vm: Vm<'_, '_, (), (), (), u8, (), ()> = Vm::new(&node, Vec::new(), Fuel::new(17), limits);
    let mut hir_session: StackedControlSession<'_, '_, (), (), (), u8, (), ()> = vm;
    assert_eq!(hir_session.status(), EffectSessionStatus::Ready);
    assert!(hir_session.cancel());
    assert!(!hir_session.cancel());
    assert_eq!(
        hir_session.status(),
        EffectSessionStatus::Terminal(EffectSessionEnd::Cancelled)
    );
    assert_eq!(hir_session.fuel_remaining(), 17);

    let scope = LocalExecutionScope::new();
    let lease = scope.lease();
    let mut scoped: LocalVm<'_, '_, (), (), (), u8, (), ()> =
        LocalVm::new(scope, &node, Vec::new(), Fuel::new(23), limits);
    assert!(lease.is_open());
    assert!(scoped.cancel());
    assert!(!lease.is_open());
    assert_eq!(scoped.fuel_remaining(), 23);
    assert_eq!(
        scoped.status(),
        EffectSessionStatus::Terminal(EffectSessionEnd::Cancelled)
    );
    assert!(!scoped.cancel());
}
