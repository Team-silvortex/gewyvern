#![cfg(feature = "leserpent-profile")]

use leselang_hir as facade;
use leselang_syntax::parse;
use leserpent_hir as owner;
use std::any::TypeId;

#[test]
fn released_product_types_are_the_adapter_types_not_mirror_definitions() {
    assert_eq!(
        TypeId::of::<facade::HirProgram>(),
        TypeId::of::<owner::HirProgram>()
    );
    assert_eq!(
        TypeId::of::<facade::Effect>(),
        TypeId::of::<owner::Effect>()
    );
    assert_eq!(TypeId::of::<facade::Type>(), TypeId::of::<owner::Type>());
    assert_eq!(
        TypeId::of::<facade::Diagnostic>(),
        TypeId::of::<owner::Diagnostic>()
    );
    assert_eq!(
        TypeId::of::<facade::host_call::HostOperation>(),
        TypeId::of::<owner::host_call::HostOperation>()
    );
    assert_eq!(
        TypeId::of::<facade::result_field::ResultField>(),
        TypeId::of::<owner::result_field::ResultField>()
    );
}

#[test]
fn old_and_direct_entry_points_share_original_program_and_computation_borrows() {
    let program: owner::HirProgram = facade::lower(&parse(
        "fn main() = bind(x: 1, body: add(left: x, right: 2))",
    ))
    .unwrap();
    let compatible: &facade::HirProgram = &program;
    assert!(std::ptr::eq(compatible, &program));
    let owner::Effect::Compute { expression } = &program.function.effect else {
        panic!("expected computation")
    };
    let compatible: &facade::computation::Computation = expression;
    let native: &leselang_hir_core::ir::Computation<
        owner::result_field::ResultField,
        owner::host_call::HostOperation,
        owner::Effect,
        owner::Type,
    > = compatible;
    assert!(std::ptr::eq(native, expression.as_ref()));
    native.validate_structure().unwrap();
    assert_eq!(
        native.validate_in_scope(&[]),
        Ok(program.function.result_type)
    );
}

#[test]
fn canonical_wire_and_capability_checks_are_interchangeable_across_entry_points() {
    for source in [
        "fn main() = runtime.list()",
        "fn main() = runtime.inspect(runtime_id: \"runtime-a\")",
        "fn main() = ui.focus(node_id: \"runtime-card\")",
        "fn main() = all(first: runtime.list(), second: runtime.list())",
    ] {
        let direct = owner::lower(&parse(source)).unwrap();
        let compatible = facade::lower(&parse(source)).unwrap();
        assert_eq!(direct, compatible);
        let wire = serde_json::to_string(&direct).unwrap();
        let decoded: facade::HirProgram = serde_json::from_str(&wire).unwrap();
        assert_eq!(decoded, direct);
        assert_eq!(
            facade::canonical_source(&direct.function.effect),
            owner::canonical_source(&decoded.function.effect)
        );
        assert_eq!(
            facade::authorize(&direct, &facade::CapabilitySet::default()),
            owner::authorize(&decoded, &owner::CapabilitySet::default())
        );
        let capabilities = owner::CapabilitySet::new(direct.function.required_capabilities.clone());
        owner::authorize(&compatible, &capabilities).unwrap();
        facade::authorize(&direct, &capabilities).unwrap();
    }
}

#[test]
fn cold_branch_and_shape_diagnostics_keep_codes_order_and_spans() {
    for source in [
        "fn main() = runtime.list(role: \"edge\", role: none, mystery: \"x\")",
        "fn main() = and(left: false, right: missing)",
        "fn main() = recover(value: 1, fallback: missing)",
        "fn main() = fold(n: 0, items: strings(), item: \"entry\", next: missing, limit: 0)",
    ] {
        let tree = parse(source);
        assert_eq!(
            facade::lower(&tree).unwrap_err(),
            owner::lower(&tree).unwrap_err()
        );
    }
}
